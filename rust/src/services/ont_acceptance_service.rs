use crate::{
    domain::ont_acceptance::{
        self, LocalPrecheckStatus, OneStatus, OneTestResult, OntTestRequest, OpticalMeasurement,
        OpticalPrecheckProfile,
    },
    mcp::auth::ControlPlaneAdminContext,
    services::{
        control_plane_admin_service as operations,
        one_provider::{ManualOneProvider, OneProvider},
    },
};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};

pub async fn requirements(
    pool: &PgPool,
    wr: &str,
    ont_id: &str,
    operator: &str,
) -> Result<Value, &'static str> {
    let mut tx = pool.begin().await.map_err(|_| "AAA unavailable")?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|_| "read-only transaction unavailable")?;
    let serial = validate_association(&mut tx, wr, ont_id, operator, false).await?;
    let profiles:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('profile_id',p.profile_id,'rx_min_dbm',p.rx_min_dbm,'rx_max_dbm',p.rx_max_dbm,'revision',p.revision) FROM optical_precheck_profiles p JOIN onts o ON o.vendor=p.vendor AND o.model=p.model WHERE o.ont_id=$1 AND p.operator_name=$2 ORDER BY p.profile_id")
        .bind(ont_id).bind(operator).fetch_all(&mut *tx).await.map_err(|_|"precheck profiles unavailable")?;
    tx.rollback()
        .await
        .map_err(|_| "read-only rollback failed")?;
    Ok(
        json!({"work_order_id":wr,"ont_id":ont_id,"ont_serial":serial,"test_provider":"ONE","provider_mode":"MANUAL_RECORDED",
        "positive_closure_requires":["ONE_PASS","codice_collaudo"],"precheck_is_final":false,"local_profiles":profiles}),
    )
}

async fn validate_association(
    tx: &mut Transaction<'_, Postgres>,
    wr: &str,
    ont_id: &str,
    operator: &str,
    lock: bool,
) -> Result<String, &'static str> {
    for id in [wr, ont_id] {
        crate::domain::physical_asset::identifier(id)?;
    }
    let query = if lock {
        "SELECT a.activation_id,a.ont_serial,w.operator_name,w.customer_id,o.serial_number,o.customer_id AS ont_customer,o.status FROM activation_requests a JOIN delivery_work_details w ON w.activation_id=a.activation_id JOIN onts o ON o.ont_id=$2 WHERE a.wr=$1 FOR UPDATE OF a,w,o"
    } else {
        "SELECT a.activation_id,a.ont_serial,w.operator_name,w.customer_id,o.serial_number,o.customer_id AS ont_customer,o.status FROM activation_requests a JOIN delivery_work_details w ON w.activation_id=a.activation_id JOIN onts o ON o.ont_id=$2 WHERE a.wr=$1"
    };
    let rows = sqlx::query(query)
        .bind(wr)
        .bind(ont_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(|_| "WR/ONT association lookup failed")?;
    if rows.len() != 1 {
        return Err("WR/ONT association unavailable or ambiguous");
    }
    let row = &rows[0];
    let serial: String = row
        .try_get("serial_number")
        .map_err(|_| "ONT serial unavailable")?;
    if row
        .try_get::<Option<String>, _>("ont_serial")
        .map_err(|_| "WR serial unavailable")?
        .as_deref()
        != Some(&serial)
        || row
            .try_get::<String, _>("operator_name")
            .map_err(|_| "operator unavailable")?
            != operator
        || row
            .try_get::<String, _>("status")
            .map_err(|_| "ONT status unavailable")?
            != "ACTIVE"
        || row
            .try_get::<Option<String>, _>("customer_id")
            .map_err(|_| "WR customer unavailable")?
            != row
                .try_get::<Option<String>, _>("ont_customer")
                .map_err(|_| "ONT customer unavailable")?
    {
        return Err("ONT is not associated with this WR/operator/customer");
    }
    let installed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM onts o JOIN ports p ON p.port_id=o.current_port_id WHERE o.ont_id=$1 AND o.status='ACTIVE' AND p.resource_state='OCCUPIED')")
        .bind(ont_id).fetch_one(&mut **tx).await.map_err(|_|"ONT installation lookup failed")?;
    if !installed {
        return Err("ONT is not installed on an occupied GPON port");
    }
    Ok(serial)
}

pub async fn start_test(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &OntTestRequest,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let tx = operations::begin_transaction(pool).await?;
    let (tx, result) = start_transaction(tx, actor, operation_id, input).await?;
    tx.commit().await.map_err(|_| "ONE start commit failed")?;
    Ok(result)
}

pub async fn start_transaction<'tx>(
    mut tx: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &OntTestRequest,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    ont_acceptance::validate_request(input)?;
    let payload = serde_json::to_value(input).map_err(|_| "test request invalid")?;
    let digest = operations::input_digest(&payload)?;
    if let Some(result) = operations::begin_operation(
        &mut tx,
        actor,
        operation_id,
        "start_ont_acceptance_test",
        &digest,
    )
    .await?
    {
        return Ok((tx, result));
    }
    let serial = validate_association(
        &mut tx,
        &input.work_order_id,
        &input.ont_id,
        &input.operator,
        true,
    )
    .await?;
    if let Some(tech) = &input.technician_id {
        let matches:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activation_requests a JOIN technicians t ON t.technician_id=a.technician_id WHERE a.wr=$1 AND a.technician_id=$2 AND t.status='ACTIVE')")
            .bind(&input.work_order_id).bind(tech).fetch_one(&mut *tx).await.map_err(|_|"test technician lookup failed")?;
        if !matches {
            return Err("test technician is not assigned to this WR");
        }
    }
    let bounds = if let Some(profile) = &input.optical_profile_id {
        let row=sqlx::query("SELECT p.rx_min_dbm,p.rx_max_dbm FROM optical_precheck_profiles p JOIN onts o ON o.vendor=p.vendor AND o.model=p.model WHERE p.profile_id=$1 AND p.operator_name=$2 AND o.ont_id=$3 FOR SHARE OF p")
            .bind(profile).bind(&input.operator).bind(&input.ont_id).fetch_optional(&mut *tx).await.map_err(|_|"precheck profile lookup failed")?.ok_or("profile does not match operator/apparatus")?;
        Some((
            row.try_get::<f64, _>("rx_min_dbm")
                .map_err(|_| "profile invalid")?,
            row.try_get::<f64, _>("rx_max_dbm")
                .map_err(|_| "profile invalid")?,
        ))
    } else {
        None
    };
    let local = ont_acceptance::precheck(input.measurement.as_ref().map(|m| m.rx_dbm), bounds);
    let local_status = local_string(&local);
    sqlx::query("INSERT INTO ont_acceptance_tests(test_id,activation_id,work_order_id,ont_id,ont_serial,operator_name,test_type,technician_id,optical_profile_id,precheck_bounds,measurement,rx_dbm,tx_dbm,attenuation_db,local_precheck_status,notes,created_by,updated_by) SELECT $1,a.activation_id,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$16 FROM activation_requests a WHERE a.wr=$2")
        .bind(&input.test_id).bind(&input.work_order_id).bind(&input.ont_id).bind(&serial).bind(&input.operator).bind(&input.test_type)
        .bind(&input.technician_id).bind(&input.optical_profile_id).bind(bounds.map(|(min,max)|json!({"min":min,"max":max})))
        .bind(input.measurement.as_ref().map(|m|json!(m))).bind(input.measurement.as_ref().map(|m|m.rx_dbm))
        .bind(input.measurement.as_ref().and_then(|m|m.tx_dbm)).bind(input.measurement.as_ref().and_then(|m|m.attenuation_db))
        .bind(local_status).bind(&input.notes).bind(actor.principal_id).execute(&mut *tx).await.map_err(|_|"ONE test start failed")?;
    let result = json!({"test_id":input.test_id,"work_order_id":input.work_order_id,"ont_id":input.ont_id,"ont_serial":serial,"one_status":"ONE_PENDING","local_precheck_status":local_status,"provider_mode":ManualOneProvider.mode(),"positive_closure_allowed":false});
    operations::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "start_ont_acceptance_test",
        "ONT_TEST",
        &input.test_id,
        &digest,
        json!({"test_id":input.test_id,"wr":input.work_order_id,"ont_id":input.ont_id}),
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((tx, result))
}

pub async fn record_result(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &OneTestResult,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let tx = operations::begin_transaction(pool).await?;
    let (tx, result) = record_result_transaction(tx, actor, operation_id, input).await?;
    tx.commit().await.map_err(|_| "ONE result commit failed")?;
    Ok(result)
}

pub async fn record_result_transaction<'tx>(
    mut tx: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &OneTestResult,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    ManualOneProvider.validate_recorded_result(input)?;
    let payload = json!(input);
    let digest = operations::input_digest(&payload)?;
    if let Some(result) = operations::begin_operation(
        &mut tx,
        actor,
        operation_id,
        "record_one_test_result",
        &digest,
    )
    .await?
    {
        return Ok((tx, result));
    }
    validate_result_against_test(&mut tx, input, true).await?;
    let row = sqlx::query("SELECT precheck_bounds FROM ont_acceptance_tests WHERE test_id=$1")
        .bind(&input.test_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| "test bounds unavailable")?;
    let bounds: Option<Value> = row
        .try_get("precheck_bounds")
        .map_err(|_| "test bounds invalid")?;
    let bounds = bounds
        .as_ref()
        .and_then(|b| Some((b.get("min")?.as_f64()?, b.get("max")?.as_f64()?)));
    let status = one_string(&input.one_status);
    if let Some(measurement) = &input.measurement {
        let local = local_string(&ont_acceptance::precheck(Some(measurement.rx_dbm), bounds));
        sqlx::query("UPDATE ont_acceptance_tests SET measurement=$2,rx_dbm=$3,tx_dbm=$4,attenuation_db=$5,local_precheck_status=$6 WHERE test_id=$1")
            .bind(&input.test_id).bind(json!(measurement)).bind(measurement.rx_dbm).bind(measurement.tx_dbm).bind(measurement.attenuation_db).bind(local)
            .execute(&mut *tx).await.map_err(|_|"test measurement update failed")?;
    }
    let changed=sqlx::query("UPDATE ont_acceptance_tests SET one_status=$2,one_result_code=$3,one_test_code=$4,completed_at=$5::text::timestamptz,failure_reason=$6,notes=$7,raw_reference=$8,updated_by=$9,updated_at=now() WHERE test_id=$1 AND one_status='ONE_PENDING'")
        .bind(&input.test_id).bind(status).bind(&input.one_result_code).bind(&input.one_test_code).bind(&input.completed_at)
        .bind(&input.failure_reason).bind(&input.notes).bind(&input.raw_reference).bind(actor.principal_id)
        .execute(&mut *tx).await.map_err(|_|"ONE result persistence failed")?;
    if changed.rows_affected() != 1 {
        return Err("ONE result state changed during recording");
    }
    let result = json!({"test_id":input.test_id,"work_order_id":input.work_order_id,"one_status":status,"one_test_code":input.one_test_code,"provider_mode":"MANUAL_RECORDED","positive_closure_allowed":input.one_status==OneStatus::OnePass});
    operations::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "record_one_test_result",
        "ONT_TEST",
        &input.test_id,
        &digest,
        json!({"test_id":input.test_id,"wr":input.work_order_id,"one_status":status}),
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((tx, result))
}

pub(crate) async fn validate_result_against_test(
    tx: &mut Transaction<'_, Postgres>,
    input: &OneTestResult,
    lock: bool,
) -> Result<(), &'static str> {
    ont_acceptance::validate_result(input)?;
    let serial = validate_association(
        tx,
        &input.work_order_id,
        &input.ont_id,
        &input.operator,
        lock,
    )
    .await?;
    if serial != input.ont_serial {
        return Err("ONE result serial differs from installed ONT");
    }
    let query = if lock {
        "SELECT work_order_id,ont_id,ont_serial,operator_name,one_status,($2::text::timestamptz>=started_at AND $2::text::timestamptz<=now()+interval '5 minutes') AS time_valid FROM ont_acceptance_tests WHERE test_id=$1 FOR UPDATE"
    } else {
        "SELECT work_order_id,ont_id,ont_serial,operator_name,one_status,($2::text::timestamptz>=started_at AND $2::text::timestamptz<=now()+interval '5 minutes') AS time_valid FROM ont_acceptance_tests WHERE test_id=$1"
    };
    let row = sqlx::query(query)
        .bind(&input.test_id)
        .bind(&input.completed_at)
        .fetch_optional(&mut **tx)
        .await
        .map_err(|_| "ONE test lookup failed")?
        .ok_or("ONE test was not started")?;
    if row
        .try_get::<String, _>("work_order_id")
        .map_err(|_| "test WR invalid")?
        != input.work_order_id
        || row
            .try_get::<String, _>("ont_id")
            .map_err(|_| "test ONT invalid")?
            != input.ont_id
        || row
            .try_get::<String, _>("ont_serial")
            .map_err(|_| "test serial invalid")?
            != input.ont_serial
        || row
            .try_get::<String, _>("operator_name")
            .map_err(|_| "test operator invalid")?
            != input.operator
        || row
            .try_get::<String, _>("one_status")
            .map_err(|_| "test status invalid")?
            != "ONE_PENDING"
        || !row
            .try_get::<bool, _>("time_valid")
            .map_err(|_| "test time invalid")?
    {
        return Err("ONE result inconsistent with started test or already finalized");
    }
    if let Some(code) = &input.one_test_code {
        let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ont_acceptance_tests WHERE one_test_code=$1 AND test_id<>$2)")
            .bind(code).bind(&input.test_id).fetch_one(&mut **tx).await.map_err(|_|"ONE code uniqueness check failed")?;
        if duplicate {
            return Err("ONE codice_collaudo already belongs to another test");
        }
    }
    Ok(())
}

pub async fn dry_run_result(pool: &PgPool, input: &OneTestResult) -> Result<Value, &'static str> {
    let mut tx = pool.begin().await.map_err(|_| "AAA unavailable")?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *tx)
        .await
        .map_err(|_| "read-only unavailable")?;
    let validation = validate_result_against_test(&mut tx, input, false).await;
    tx.rollback().await.map_err(|_| "dry-run rollback failed")?;
    Ok(match validation {
        Ok(()) => json!({"status":"PASS","persisted":false}),
        Err(reason) => json!({"status":"DENY","reason":reason,"persisted":false}),
    })
}

pub async fn get_test(pool: &PgPool, test_id: &str) -> Result<Option<Value>, &'static str> {
    crate::domain::physical_asset::identifier(test_id)?;
    sqlx::query_scalar("SELECT to_jsonb(t) FROM ont_acceptance_tests t WHERE test_id=$1")
        .bind(test_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| "ONE test unavailable")
}
pub async fn list_tests(pool: &PgPool, wr: &str) -> Result<Vec<Value>, &'static str> {
    crate::domain::physical_asset::identifier(wr)?;
    sqlx::query_scalar("SELECT to_jsonb(t) FROM ont_acceptance_tests t WHERE work_order_id=$1 ORDER BY started_at DESC,test_id LIMIT 100").bind(wr).fetch_all(pool).await.map_err(|_|"ONE tests unavailable")
}

pub(crate) async fn require_closure_test(
    tx: &mut Transaction<'_, Postgres>,
    wr: &str,
    ont_serial: &str,
) -> Result<Value, &'static str> {
    let current:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activation_requests a JOIN delivery_work_details w ON w.activation_id=a.activation_id JOIN onts o ON o.serial_number=a.ont_serial AND o.customer_id=w.customer_id JOIN ports p ON p.port_id=o.current_port_id JOIN ont_modem_links l ON l.ont_id=o.ont_id AND l.status='ACTIVE' JOIN modems m ON m.modem_id=l.modem_id AND m.status='ACTIVE' WHERE a.wr=$1 AND a.ont_serial=$2 AND o.status='ACTIVE' AND p.resource_state='OCCUPIED')")
        .bind(wr).bind(ont_serial).fetch_one(&mut **tx).await.map_err(|_|"ONE closure installation check failed")?;
    if !current {
        return Err("WR/ONT customer association or installed modem is missing");
    }
    let row=sqlx::query("SELECT test_id,one_status,one_test_code,rx_dbm FROM ont_acceptance_tests WHERE work_order_id=$1 AND ont_serial=$2 ORDER BY started_at DESC,test_id DESC LIMIT 1 FOR SHARE")
        .bind(wr).bind(ont_serial).fetch_optional(&mut **tx).await.map_err(|_|"ONE closure validation unavailable")?.ok_or("positive closure requires a recorded ONE test")?;
    if row
        .try_get::<String, _>("one_status")
        .map_err(|_| "ONE status unavailable")?
        != "ONE_PASS"
    {
        return Err("latest ONE test is not ONE_PASS");
    }
    let code: String = row
        .try_get("one_test_code")
        .map_err(|_| "ONE_PASS lacks codice_collaudo")?;
    ont_acceptance::validate_test_code(&code)?;
    let mut fields = json!({"one_test_status":"ONE_PASS","one_test_code":code,"one_test_id":row.try_get::<String,_>("test_id").map_err(|_|"ONE test ID unavailable")?});
    if let Some(rx) = row
        .try_get::<Option<f64>, _>("rx_dbm")
        .map_err(|_| "RX unavailable")?
    {
        fields["optical_rx_dbm"] = json!(rx);
    }
    Ok(fields)
}

pub async fn validate_closure(pool: &PgPool, wr: &str) -> Result<Value, &'static str> {
    let mut tx = pool.begin().await.map_err(|_| "AAA unavailable")?;
    let serial: Option<String> =
        sqlx::query_scalar("SELECT ont_serial FROM activation_requests WHERE wr=$1")
            .bind(wr)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| "WR unavailable")?
            .flatten();
    let validation = if let Some(serial) = serial {
        require_closure_test(&mut tx, wr, &serial).await
    } else {
        Err("WR has no installed ONT serial")
    };
    tx.rollback()
        .await
        .map_err(|_| "closure dry-run rollback failed")?;
    Ok(match validation {
        Ok(fields) => json!({"status":"PASS","persisted":false,"extra_fields_patch":fields}),
        Err(reason) => json!({"status":"DENY","persisted":false,"reason":reason}),
    })
}

pub async fn close_work(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    wr: &str,
    external_reference: &str,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let tx = operations::begin_transaction(pool).await?;
    let (tx, result) = close_transaction(tx, actor, operation_id, wr, external_reference).await?;
    tx.commit().await.map_err(|_| "WR closure commit failed")?;
    Ok(result)
}
pub async fn close_transaction<'tx>(
    mut tx: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    wr: &str,
    external_reference: &str,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    crate::domain::physical_asset::identifier(wr)?;
    crate::domain::physical_asset::identifier(external_reference)?;
    let payload = json!({"wr":wr,"external_reference":external_reference});
    let digest = operations::input_digest(&payload)?;
    if let Some(result) = operations::begin_operation(
        &mut tx,
        actor,
        operation_id,
        "close_work_with_test_result",
        &digest,
    )
    .await?
    {
        return Ok((tx, result));
    }
    let rows=sqlx::query("SELECT a.activation_id,a.ont_serial,w.operator_name,w.customer_id,w.work_status,w.note_chiusura,w.extra_fields FROM activation_requests a JOIN delivery_work_details w ON w.activation_id=a.activation_id WHERE a.wr=$1 FOR UPDATE OF a,w")
        .bind(wr).fetch_all(&mut *tx).await.map_err(|_|"WR closure lock failed")?;
    if rows.len() != 1 {
        return Err("WR closure unavailable or ambiguous");
    }
    let row = &rows[0];
    let activation: String = row.try_get("activation_id").map_err(|_| "WR ID invalid")?;
    let work_status: String = row
        .try_get("work_status")
        .map_err(|_| "WR status unavailable")?;
    if !matches!(
        work_status.as_str(),
        "ASSIGNED" | "SCHEDULED" | "IN_PROGRESS"
    ) {
        return Err("WR is not in an eligible phase for positive closure");
    }
    let serial: String = row
        .try_get("ont_serial")
        .map_err(|_| "WR has no installed ONT")?;
    let installed:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM onts o JOIN ports p ON p.port_id=o.current_port_id JOIN ont_modem_links l ON l.ont_id=o.ont_id AND l.status='ACTIVE' JOIN modems m ON m.modem_id=l.modem_id AND m.status='ACTIVE' WHERE o.serial_number=$1 AND o.customer_id=$2 AND o.status='ACTIVE' AND p.resource_state='OCCUPIED')")
        .bind(&serial).bind(row.try_get::<Option<String>,_>("customer_id").map_err(|_|"WR customer unavailable")?).fetch_one(&mut *tx).await.map_err(|_|"installed line check failed")?;
    if !installed {
        return Err("closure requires installed active customer ONT/modem");
    }
    let fields = require_closure_test(&mut tx, wr, &serial).await?;
    let code = fields["one_test_code"].as_str().ok_or("ONE code missing")?;
    let line = format!("Collaudo ONE: PASS - Codice: {code}");
    let existing: Option<String> = row
        .try_get("note_chiusura")
        .map_err(|_| "closure notes unavailable")?;
    let note = match existing {
        Some(note) if !note.trim().is_empty() => format!("{note}\n{line}"),
        _ => line.clone(),
    };
    sqlx::query("UPDATE delivery_work_details SET work_status='COMPLETED',note_chiusura=$2,extra_fields=extra_fields||$3::jsonb,revision=revision+1,updated_by=$4,updated_at=now() WHERE activation_id=$1")
        .bind(&activation).bind(&note).bind(&fields).bind(actor.principal_id).execute(&mut *tx).await.map_err(|_|"WR closure update failed")?;
    let patch = json!({"work_order_id":wr,"external_reference":external_reference,"note_chiusura_append":line,"extra_fields_patch":fields,
        "merge_policy":"APPEND_NOTE_AND_MERGE_FIELDS","test_provider":"ONE"});
    let result = json!({"status":"COMPLETED","work_order_id":wr,"one_test_code":code,"delivery_sync_status":"PENDING","delivery_patch":patch});
    operations::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "close_work_with_test_result",
        "WORK",
        &activation,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    sqlx::query("INSERT INTO delivery_closure_outbox(operation_id,work_order_id,external_reference,test_id,patch) VALUES ($1,$2,$3,$4,$5)")
        .bind(operation_id).bind(wr).bind(external_reference).bind(fields["one_test_id"].as_str()).bind(patch).execute(&mut *tx).await.map_err(|_|"delivery closure patch enqueue failed")?;
    Ok((tx, result))
}

pub async fn record_measurement(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    test_id: &str,
    measurement: &OpticalMeasurement,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    ont_acceptance::validate_measurement(measurement)?;
    let mut tx = operations::begin_transaction(pool).await?;
    let payload = json!({"test_id":test_id,"measurement":measurement});
    let digest = operations::input_digest(&payload)?;
    if let Some(result) = operations::begin_operation(
        &mut tx,
        actor,
        operation_id,
        "record_optical_measurement",
        &digest,
    )
    .await?
    {
        tx.rollback()
            .await
            .map_err(|_| "measurement replay rollback failed")?;
        return Ok(result);
    }
    let row=sqlx::query("SELECT work_order_id,ont_id,operator_name,precheck_bounds FROM ont_acceptance_tests WHERE test_id=$1 AND one_status='ONE_PENDING' FOR UPDATE").bind(test_id).fetch_optional(&mut *tx).await.map_err(|_|"test lookup failed")?.ok_or("measurement requires a pending ONE test")?;
    validate_association(
        &mut tx,
        &row.try_get::<String, _>("work_order_id")
            .map_err(|_| "test WR unavailable")?,
        &row.try_get::<String, _>("ont_id")
            .map_err(|_| "test ONT unavailable")?,
        &row.try_get::<String, _>("operator_name")
            .map_err(|_| "test operator unavailable")?,
        true,
    )
    .await?;
    let bounds: Option<Value> = row
        .try_get("precheck_bounds")
        .map_err(|_| "profile snapshot invalid")?;
    let bounds = bounds.and_then(|b| Some((b.get("min")?.as_f64()?, b.get("max")?.as_f64()?)));
    let local = local_string(&ont_acceptance::precheck(Some(measurement.rx_dbm), bounds));
    sqlx::query("UPDATE ont_acceptance_tests SET measurement=$2,rx_dbm=$3,tx_dbm=$4,attenuation_db=$5,local_precheck_status=$6,updated_by=$7,updated_at=now() WHERE test_id=$1")
        .bind(test_id).bind(json!(measurement)).bind(measurement.rx_dbm).bind(measurement.tx_dbm).bind(measurement.attenuation_db).bind(local).bind(actor.principal_id).execute(&mut *tx).await.map_err(|_|"measurement persistence failed")?;
    let result = json!({"test_id":test_id,"local_precheck_status":local,"one_status":"ONE_PENDING","positive_closure_allowed":false});
    operations::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "record_optical_measurement",
        "ONT_TEST",
        test_id,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    tx.commit().await.map_err(|_| "measurement commit failed")?;
    Ok(result)
}

pub async fn save_profile(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &OpticalPrecheckProfile,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    ont_acceptance::validate_profile(input)?;
    let mut tx = operations::begin_transaction(pool).await?;
    let payload = json!(input);
    let digest = operations::input_digest(&payload)?;
    if let Some(result) = operations::begin_operation(
        &mut tx,
        actor,
        operation_id,
        "set_optical_precheck_profile",
        &digest,
    )
    .await?
    {
        tx.rollback()
            .await
            .map_err(|_| "profile replay rollback failed")?;
        return Ok(result);
    }
    sqlx::query("INSERT INTO optical_precheck_profiles(profile_id,operator_name,vendor,model,rx_min_dbm,rx_max_dbm,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(profile_id) DO UPDATE SET operator_name=EXCLUDED.operator_name,vendor=EXCLUDED.vendor,model=EXCLUDED.model,rx_min_dbm=EXCLUDED.rx_min_dbm,rx_max_dbm=EXCLUDED.rx_max_dbm,revision=optical_precheck_profiles.revision+1,updated_by=EXCLUDED.updated_by,updated_at=now()")
        .bind(&input.profile_id).bind(&input.operator).bind(&input.vendor).bind(&input.model).bind(input.rx_min_dbm).bind(input.rx_max_dbm).bind(actor.principal_id).execute(&mut *tx).await.map_err(|_|"optical profile save failed")?;
    let result = json!({"profile_id":input.profile_id,"status":"SAVED","precheck_only":true});
    operations::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "set_optical_precheck_profile",
        "OPTICAL_PROFILE",
        &input.profile_id,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    tx.commit()
        .await
        .map_err(|_| "optical profile commit failed")?;
    Ok(result)
}
fn local_string(status: &LocalPrecheckStatus) -> &'static str {
    match status {
        LocalPrecheckStatus::Pending => "PENDING",
        LocalPrecheckStatus::PrecheckPass => "PRECHECK_PASS",
        LocalPrecheckStatus::PrecheckFail => "PRECHECK_FAIL",
    }
}
fn one_string(status: &OneStatus) -> &'static str {
    match status {
        OneStatus::OnePending => "ONE_PENDING",
        OneStatus::OnePass => "ONE_PASS",
        OneStatus::OneFail => "ONE_FAIL",
        OneStatus::Error => "ERROR",
    }
}

#[cfg(test)]
pub(crate) async fn exercise_synthetic_rollback<'tx>(
    mut tx: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    wr: &str,
    ont_id: &str,
    operation: &str,
) -> Result<Transaction<'tx, Postgres>, Box<dyn std::error::Error>> {
    let customer = format!("{operation}-CUSTOMER");
    sqlx::query("INSERT INTO customers(customer_id,status) VALUES ($1,'ACTIVE')")
        .bind(&customer)
        .execute(&mut *tx)
        .await?;
    let serial: String = sqlx::query_scalar("SELECT serial_number FROM onts WHERE ont_id=$1")
        .bind(ont_id)
        .fetch_one(&mut *tx)
        .await?;
    sqlx::query("UPDATE onts SET customer_id=$2,vendor='SYNTHETIC_ONE_VENDOR',model='SYNTHETIC_ONE_MODEL' WHERE ont_id=$1")
        .bind(ont_id).bind(&customer).execute(&mut *tx).await?;
    sqlx::query("UPDATE activation_requests SET ont_serial=$2 WHERE wr=$1")
        .bind(wr)
        .bind(&serial)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE delivery_work_details SET customer_id=$2,extra_fields=jsonb_build_object('keep_me','unchanged') WHERE work_order_id=$1")
        .bind(wr).bind(&customer).execute(&mut *tx).await?;
    let profile = format!("{operation}-OPTICAL-PROFILE");
    sqlx::query("INSERT INTO optical_precheck_profiles(profile_id,operator_name,vendor,model,rx_min_dbm,rx_max_dbm,updated_by) VALUES ($1,'SYNTHETIC TEST','SYNTHETIC_ONE_VENDOR','SYNTHETIC_ONE_MODEL',-28,-8,$2)")
        .bind(&profile).bind(actor.principal_id).execute(&mut *tx).await?;
    let timestamp: String = sqlx::query_scalar(
        "SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&mut *tx)
    .await?;
    let mut request = OntTestRequest {
        test_id: format!("{operation}-ONE-01"),
        work_order_id: wr.into(),
        ont_id: ont_id.into(),
        operator: "SYNTHETIC TEST".into(),
        test_type: "SYNTHETIC_ROLLBACK".into(),
        technician_id: None,
        optical_profile_id: Some(profile),
        measurement: Some(OpticalMeasurement {
            rx_dbm: -18.4,
            tx_dbm: None,
            attenuation_db: None,
            measured_at: timestamp.clone(),
            instrument: "SYNTHETIC POWER METER".into(),
            measurement_point: "SYNTHETIC ONT".into(),
        }),
        notes: None,
    };
    let (transaction, started) =
        start_transaction(tx, actor, &format!("{operation}-ONE-START-01"), &request).await?;
    tx = transaction;
    assert_eq!(started["one_status"], "ONE_PENDING");
    assert_eq!(started["local_precheck_status"], "PRECHECK_PASS");
    assert!(require_closure_test(&mut tx, wr, &serial).await.is_err());
    let mut result = OneTestResult {
        test_id: request.test_id.clone(),
        work_order_id: wr.into(),
        ont_id: ont_id.into(),
        ont_serial: serial.clone(),
        operator: request.operator.clone(),
        one_status: OneStatus::OnePass,
        one_result_code: "SYNTHETIC_PASS".into(),
        one_test_code: None,
        completed_at: timestamp.clone(),
        failure_reason: None,
        notes: None,
        raw_reference: Some("SYNTHETIC_ROLLBACK_ONLY".into()),
        measurement: None,
    };
    assert!(
        validate_result_against_test(&mut tx, &result, false)
            .await
            .is_err()
    );
    result.one_status = OneStatus::OneFail;
    result.one_result_code = "SYNTHETIC_FAIL".into();
    result.failure_reason = Some("SYNTHETIC ONE failure".into());
    result.one_test_code = Some(format!("{operation}-ONE-FAIL-CODE"));
    let (transaction, _) =
        record_result_transaction(tx, actor, &format!("{operation}-ONE-FAIL"), &result).await?;
    tx = transaction;
    assert!(require_closure_test(&mut tx, wr, &serial).await.is_err());
    request.test_id = format!("{operation}-ONE-02");
    let (transaction, _) =
        start_transaction(tx, actor, &format!("{operation}-ONE-START-02"), &request).await?;
    tx = transaction;
    result.test_id = request.test_id.clone();
    result.one_status = OneStatus::OnePass;
    result.one_result_code = "SYNTHETIC_PASS".into();
    result.failure_reason = None;
    assert!(
        validate_result_against_test(&mut tx, &result, false)
            .await
            .is_err()
    );
    result.one_test_code = Some(format!("{operation}-ONE-PASS-CODE"));
    let mut invalid = result.clone();
    invalid.work_order_id = format!("{operation}-WR-WRONG");
    assert!(
        validate_result_against_test(&mut tx, &invalid, false)
            .await
            .is_err()
    );
    invalid = result.clone();
    invalid.ont_serial = "SYNTHETIC_INCOHERENT_SERIAL".into();
    assert!(
        validate_result_against_test(&mut tx, &invalid, false)
            .await
            .is_err()
    );
    assert!(
        validate_result_against_test(&mut tx, &result, false)
            .await
            .is_ok()
    );
    let record_operation = format!("{operation}-ONE-PASS");
    let (transaction, recorded) =
        record_result_transaction(tx, actor, &record_operation, &result).await?;
    let (transaction, replayed) =
        record_result_transaction(transaction, actor, &record_operation, &result).await?;
    assert_eq!(recorded, replayed);
    tx = transaction;
    assert!(require_closure_test(&mut tx, wr, &serial).await.is_ok());
    let (transaction, closed) = close_transaction(
        tx,
        actor,
        &format!("{operation}-ONE-CLOSE"),
        wr,
        &format!("{operation}-EXTERNAL-WR"),
    )
    .await?;
    tx = transaction;
    assert_eq!(closed["status"], "COMPLETED");
    assert_eq!(closed["delivery_sync_status"], "PENDING");
    let row = sqlx::query(
        "SELECT extra_fields,note_chiusura FROM delivery_work_details WHERE work_order_id=$1",
    )
    .bind(wr)
    .fetch_one(&mut *tx)
    .await?;
    let fields: Value = row.try_get("extra_fields")?;
    assert_eq!(fields["keep_me"], "unchanged");
    assert_eq!(fields["one_test_code"], json!(result.one_test_code));
    assert_eq!(fields["optical_rx_dbm"], json!(-18.4));
    assert!(
        row.try_get::<String, _>("note_chiusura")?
            .contains("Collaudo ONE: PASS - Codice:")
    );
    let history = operations::get_operation_history(&mut *tx, &format!("{operation}-ONE-CLOSE"))
        .await?
        .ok_or("closure operation history missing")?;
    assert_eq!(history["status"], "APPLIED");
    assert_eq!(
        history["result"]["one_test_code"],
        json!(result.one_test_code)
    );
    let audit = operations::read_admin_audit(&mut *tx, Some(&request.test_id), 10).await?;
    assert_eq!(audit.len(), 2);
    Ok(tx)
}
