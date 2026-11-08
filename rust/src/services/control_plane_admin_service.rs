use crate::domain::field_ops::{
    CreateFieldRoe, RegisterModem, RegisterOnt, validate_new_roe, validate_registration_metadata,
    validate_serial,
};
use crate::mcp::auth::ControlPlaneAdminContext;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Executor, PgPool, Postgres, Row, Transaction, types::Json};
use uuid::Uuid;

pub(crate) fn input_digest(payload: &Value) -> Result<String, &'static str> {
    let bytes = serde_json::to_vec(payload).map_err(|_| "input encoding failed")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

pub async fn read_admin_audit<'e, E>(
    executor: E,
    resource_id: Option<&str>,
    limit: i64,
) -> Result<Vec<Value>, &'static str>
where
    E: Executor<'e, Database = Postgres>,
{
    if let Some(id) = resource_id {
        crate::domain::physical_asset::identifier(id)?;
    }
    sqlx::query_scalar("SELECT jsonb_build_object('audit_id',audit_id,'operation_id',operation_id,'principal_id',principal_id,'tool_name',tool_name,'resource_type',resource_type,'resource_id',resource_id,'sanitized_input',sanitized_input,'validation',validation,'result',result,'created_at',created_at) FROM control_plane_admin_audit WHERE ($1::text IS NULL OR resource_id=$1) ORDER BY audit_id DESC LIMIT $2")
        .bind(resource_id).bind(limit.clamp(1,200)).fetch_all(executor).await.map_err(|_|"admin audit read failed")
}

pub async fn get_operation_history<'e, E>(
    executor: E,
    operation_id: &str,
) -> Result<Option<Value>, &'static str>
where
    E: Executor<'e, Database = Postgres>,
{
    crate::domain::physical_asset::identifier(operation_id)?;
    sqlx::query_scalar("SELECT jsonb_build_object('operation_id',o.operation_id,'principal_id',o.principal_id,'tool_name',o.tool_name,'input_sha256',o.input_sha256,'status',o.status,'resource_id',o.resource_id,'result',o.result,'created_at',o.created_at,'updated_at',o.updated_at,'audit',to_jsonb(a)) FROM control_plane_admin_operations o LEFT JOIN control_plane_admin_audit a ON a.operation_id=o.operation_id WHERE o.operation_id=$1")
        .bind(operation_id).fetch_optional(executor).await.map_err(|_|"operation history read failed")
}

pub(crate) async fn begin_operation(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    tool_name: &str,
    digest: &str,
) -> Result<Option<Value>, &'static str> {
    if operation_id.is_empty()
        || operation_id.len() > 128
        || !operation_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("invalid operation ID");
    }
    actor.require_write_authorization()?;
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(operation_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "operation lock unavailable")?;
    let existing = sqlx::query("SELECT principal_id,tool_name,input_sha256,status,result FROM control_plane_admin_operations WHERE operation_id=$1 FOR UPDATE")
        .bind(operation_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| "operation lookup failed")?;
    if let Some(existing) = existing {
        if existing
            .try_get::<String, _>("principal_id")
            .map_err(|_| "invalid operation actor")?
            != actor.principal_id
            || existing
                .try_get::<String, _>("tool_name")
                .map_err(|_| "invalid operation tool")?
                != tool_name
            || existing
                .try_get::<String, _>("input_sha256")
                .map_err(|_| "invalid operation digest")?
                != digest
        {
            return Err("operation ID already used with different input or principal");
        }
        if existing
            .try_get::<String, _>("status")
            .map_err(|_| "invalid operation status")?
            != "APPLIED"
        {
            return Err("operation is not complete");
        }
        return existing
            .try_get::<Option<Value>, _>("result")
            .map_err(|_| "invalid operation result")?
            .map(Some)
            .ok_or("operation result is missing");
    }
    sqlx::query("INSERT INTO control_plane_admin_operations(operation_id,principal_id,tool_name,input_sha256,status) VALUES ($1,$2,$3,$4,'PENDING')")
        .bind(operation_id).bind(actor.principal_id).bind(tool_name).bind(digest)
        .execute(&mut **transaction).await.map_err(|_| "operation creation failed")?;
    Ok(None)
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn finish_operation(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    tool_name: &str,
    resource_type: &str,
    resource_id: &str,
    digest: &str,
    payload: Value,
    validation: Value,
    result: Value,
) -> Result<(), &'static str> {
    sqlx::query("INSERT INTO control_plane_admin_audit(operation_id,principal_id,tool_name,resource_type,resource_id,sanitized_input,input_sha256,validation,result) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)")
        .bind(operation_id).bind(actor.principal_id).bind(tool_name).bind(resource_type)
        .bind(resource_id).bind(Json(payload)).bind(digest).bind(Json(validation)).bind(Json(result.clone()))
        .execute(&mut **transaction).await.map_err(|_| "admin audit write failed")?;
    let changed = sqlx::query("UPDATE control_plane_admin_operations SET status='APPLIED',resource_id=$2,result=$3,updated_at=now() WHERE operation_id=$1 AND status='PENDING'")
        .bind(operation_id).bind(resource_id).bind(Json(result))
        .execute(&mut **transaction).await.map_err(|_| "operation completion failed")?;
    if changed.rows_affected() != 1 {
        return Err("operation completion conflict");
    }
    Ok(())
}

pub async fn create_roe(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &CreateFieldRoe,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = begin_transaction(pool).await?;
    let (transaction, result) =
        create_roe_transaction(transaction, actor, operation_id, input).await?;
    transaction
        .commit()
        .await
        .map_err(|_| "ROE transaction commit failed")?;
    Ok(result)
}

pub async fn create_roe_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &CreateFieldRoe,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    actor.require_write_authorization()?;
    validate_roe(input)?;
    let payload = serde_json::to_value(input).map_err(|_| "ROE input cannot be serialized")?;
    let digest = input_digest(&payload)?;
    if let Some(result) =
        begin_operation(&mut transaction, actor, operation_id, "create_roe", &digest).await?
    {
        return Ok((transaction, result));
    }

    let topology = sqlx::query("SELECT c.status AS cabinet_status,s.status AS splitter_status FROM cabinets c JOIN splitters s ON s.cabinet_id=c.cabinet_id WHERE c.cabinet_id=$1 AND s.splitter_id=$2 FOR SHARE OF c,s")
        .bind(&input.cabinet_id)
        .bind(&input.splitter_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "cabinet/splitter lookup failed")?
        .ok_or("cabinet or splitter does not exist")?;
    let cabinet_status: String = topology
        .try_get("cabinet_status")
        .map_err(|_| "cabinet state unavailable")?;
    let splitter_status: String = topology
        .try_get("splitter_status")
        .map_err(|_| "splitter state unavailable")?;
    if cabinet_status != "ACTIVE" || splitter_status != "ACTIVE" {
        return Err("cabinet and splitter must be ACTIVE");
    }
    let roe_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM roes WHERE roe_id=$1)")
            .bind(&input.roe_id)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| "ROE identifier lookup failed")?;
    if roe_exists {
        return Err("ROE identifier already exists");
    }
    sqlx::query("INSERT INTO feeder_cables (cable_id,cabinet_id,fiber_count) VALUES ($1,$2,$3) ON CONFLICT (cable_id) DO NOTHING")
        .bind(&input.feeder_cable_id)
        .bind(&input.cabinet_id)
        .bind(input.feeder_fiber_count)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "feeder cable creation failed")?;
    let cable = sqlx::query("SELECT cabinet_id,fiber_count,fibers_used,fibers_reserved,status FROM feeder_cables WHERE cable_id=$1 FOR UPDATE")
        .bind(&input.feeder_cable_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "feeder cable lookup failed")?
        .ok_or("feeder cable unavailable")?;
    let cable_cabinet: String = cable
        .try_get("cabinet_id")
        .map_err(|_| "feeder state unavailable")?;
    let fiber_count: i32 = cable
        .try_get("fiber_count")
        .map_err(|_| "feeder state unavailable")?;
    let fibers_used: i32 = cable
        .try_get("fibers_used")
        .map_err(|_| "feeder state unavailable")?;
    let fibers_reserved: i32 = cable
        .try_get("fibers_reserved")
        .map_err(|_| "feeder state unavailable")?;
    let cable_status: String = cable
        .try_get("status")
        .map_err(|_| "feeder state unavailable")?;
    let available = crate::domain::field_ops::available_feeder_fibers(
        fiber_count,
        fibers_used,
        fibers_reserved,
    )
    .ok_or("feeder capacity state is invalid")?;
    if cable_cabinet != input.cabinet_id
        || fiber_count != input.feeder_fiber_count
        || cable_status != "ACTIVE"
    {
        return Err("feeder cable does not match the selected cabinet or capacity");
    }
    if input.fibers_allocated > available {
        return Err("insufficient free feeder fibers");
    }
    let address = format!(
        "{} {}, {} {}",
        input.street_name, input.street_number, input.postal_code, input.city
    );
    sqlx::query("INSERT INTO roes (roe_id,cabinet_id,splitter_id,address_reference,upstream_resource,notes,address_note,source_event,created_by_session,street_name,street_number,city,postal_code,latitude,longitude,location_accuracy_m,geolocated_at,geolocation_source,feeder_cable_id,created_by_technician,created_by_control_plane_admin) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,now(),$17,$18,NULL,$19)")
        .bind(&input.roe_id).bind(&input.cabinet_id).bind(&input.splitter_id).bind(address)
        .bind(&input.splitter_id).bind(&input.address_note).bind(&input.address_note).bind(&input.source_event)
        .bind(format!("{}:{}", actor.principal_id, operation_id))
        .bind(&input.street_name).bind(&input.street_number).bind(&input.city).bind(&input.postal_code)
        .bind(input.latitude).bind(input.longitude).bind(input.location_accuracy_m)
        .bind(&input.geolocation_source).bind(&input.feeder_cable_id).bind(actor.principal_id)
        .execute(&mut *transaction).await.map_err(|_| "ROE creation failed")?;
    sqlx::query("UPDATE feeder_cables SET fibers_reserved=fibers_reserved+$2,updated_at=now() WHERE cable_id=$1")
        .bind(&input.feeder_cable_id).bind(input.fibers_allocated).execute(&mut *transaction).await
        .map_err(|_| "feeder capacity reservation failed")?;
    sqlx::query("INSERT INTO feeder_cable_allocations (allocation_id,cable_id,roe_id,fibers_allocated,fibers_reserved) VALUES ($1,$2,$3,$4,$4)")
        .bind(Uuid::new_v4().to_string()).bind(&input.feeder_cable_id).bind(&input.roe_id)
        .bind(input.fibers_allocated).execute(&mut *transaction).await
        .map_err(|_| "feeder allocation creation failed")?;
    let result = json!({"status":"CREATED","resource_type":"ROE","resource_id":input.roe_id,"operation_id":operation_id});
    finish_operation(
        &mut transaction,
        actor,
        operation_id,
        "create_roe",
        "ROE",
        &input.roe_id,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((transaction, result))
}

pub async fn register_ont(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &RegisterOnt,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = begin_transaction(pool).await?;
    let (transaction, result) =
        register_ont_transaction(transaction, actor, operation_id, input).await?;
    transaction
        .commit()
        .await
        .map_err(|_| "ONT transaction commit failed")?;
    Ok(result)
}

pub async fn register_ont_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &RegisterOnt,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    actor.require_write_authorization()?;
    validate_ont(input)?;
    let payload = serde_json::to_value(input).map_err(|_| "ONT input cannot be serialized")?;
    let digest = input_digest(&payload)?;
    if let Some(result) = begin_operation(
        &mut transaction,
        actor,
        operation_id,
        "register_ont",
        &digest,
    )
    .await?
    {
        return Ok((transaction, result));
    }
    let port = sqlx::query("SELECT p.resource_state,(p.splitter_id=r.splitter_id AND s.cabinet_id=r.cabinet_id AND s.status='ACTIVE' AND r.status='ACTIVE') AS topology_valid FROM ports p JOIN splitters s ON s.splitter_id=p.splitter_id JOIN roes r ON r.roe_id=$1 WHERE p.port_id=$2 FOR UPDATE OF p")
        .bind(&input.roe_id).bind(&input.gpon_port_id).fetch_optional(&mut *transaction).await
        .map_err(|_| "GPON port lookup failed")?.ok_or("GPON port does not belong to selected ROE")?;
    let port_state: String = port
        .try_get("resource_state")
        .map_err(|_| "GPON port state unavailable")?;
    let topology_valid: bool = port
        .try_get("topology_valid")
        .map_err(|_| "GPON topology state unavailable")?;
    let assigned = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM onts WHERE current_port_id=$1 AND (status='ACTIVE' OR customer_id IS NOT NULL))")
        .bind(&input.gpon_port_id).fetch_one(&mut *transaction).await.map_err(|_| "GPON assignment lookup failed")?;
    let resource_free = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM resource_states WHERE resource_id=$1 AND resource_kind='PORT' AND resource_state='FREE' AND owner_transaction_id IS NULL) AND NOT EXISTS(SELECT 1 FROM resource_leases WHERE resource_id=$1 AND released_at IS NULL AND expires_at>now())")
        .bind(&input.gpon_port_id).fetch_one(&mut *transaction).await.map_err(|_| "GPON resource ownership check failed")?;
    if port_state != "FREE" || assigned || !topology_valid || !resource_free {
        return Err("GPON_PORT_OCCUPIED");
    }
    let roe_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roes WHERE roe_id=$1 AND status='ACTIVE')",
    )
    .bind(&input.roe_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| "ROE lookup failed")?;
    let serial_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM onts WHERE serial_number=$1)")
            .bind(&input.serial)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| "ONT serial lookup failed")?;
    if !roe_active || serial_exists {
        return Err("active ROE required and ONT serial must be unique");
    }
    let ont_id = format!("ONT-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO onts (ont_id,serial_number,roe_id,current_port_id,vendor,model,created_by_technician,created_by_control_plane_admin) VALUES ($1,$2,$3,$4,$5,$6,NULL,$7)")
        .bind(&ont_id).bind(&input.serial).bind(&input.roe_id).bind(&input.gpon_port_id)
        .bind(&input.vendor).bind(&input.model).bind(actor.principal_id).execute(&mut *transaction).await
        .map_err(|_| "ONT registration failed")?;
    let changed = sqlx::query("UPDATE ports SET resource_state='OCCUPIED',updated_at=now() WHERE port_id=$1 AND resource_state='FREE'")
        .bind(&input.gpon_port_id).execute(&mut *transaction).await.map_err(|_| "GPON assignment failed")?;
    if changed.rows_affected() != 1 {
        return Err("GPON_PORT_OCCUPIED");
    }
    let changed = sqlx::query("UPDATE resource_states SET resource_state='OCCUPIED',revision=revision+1,updated_at=now() WHERE resource_id=$1 AND resource_kind='PORT' AND resource_state='FREE' AND owner_transaction_id IS NULL")
        .bind(&input.gpon_port_id).execute(&mut *transaction).await.map_err(|_| "GPON resource state update failed")?;
    if changed.rows_affected() != 1 {
        return Err("GPON_PORT_OCCUPIED");
    }
    let result = json!({"status":"CREATED","resource_type":"ONT","resource_id":ont_id,"operation_id":operation_id});
    finish_operation(
        &mut transaction,
        actor,
        operation_id,
        "register_ont",
        "ONT",
        result["resource_id"].as_str().unwrap_or_default(),
        &digest,
        payload,
        json!({"status":"PASS","port_state":"FREE"}),
        result.clone(),
    )
    .await?;
    Ok((transaction, result))
}

pub async fn register_or_replace_modem(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &RegisterModem,
    replace_modem_id: Option<&str>,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = begin_transaction(pool).await?;
    let (transaction, result) =
        modem_transaction(transaction, actor, operation_id, input, replace_modem_id).await?;
    transaction
        .commit()
        .await
        .map_err(|_| "modem transaction commit failed")?;
    Ok(result)
}

pub async fn modem_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &RegisterModem,
    replace_modem_id: Option<&str>,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    actor.require_write_authorization()?;
    validate_modem(input)?;
    let payload = json!({"input":input,"replace_modem_id":replace_modem_id});
    let digest = input_digest(&payload)?;
    let tool = if replace_modem_id.is_some() {
        "replace_modem"
    } else {
        "register_modem"
    };
    if let Some(result) =
        begin_operation(&mut transaction, actor, operation_id, tool, &digest).await?
    {
        return Ok((transaction, result));
    }
    let ont_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM onts WHERE ont_id=$1 FOR UPDATE")
            .bind(&input.ont_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|_| "ONT lookup failed")?;
    if ont_status.as_deref() != Some("ACTIVE") {
        return Err("active ONT required");
    }
    let serial_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM modems WHERE serial=$1)")
            .bind(&input.serial)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| "modem serial lookup failed")?;
    if serial_exists {
        return Err("modem serial already exists");
    }
    let active_link = sqlx::query("SELECT l.link_id,l.modem_id FROM ont_modem_links l JOIN modems m ON m.modem_id=l.modem_id WHERE l.ont_id=$1 AND l.status='ACTIVE' AND m.status='ACTIVE' FOR UPDATE OF l,m")
        .bind(&input.ont_id).fetch_optional(&mut *transaction).await.map_err(|_| "active modem link lookup failed")?;
    match (replace_modem_id, active_link.as_ref()) {
        (None, None) => {}
        (Some(expected), Some(row))
            if row.try_get::<String, _>("modem_id").ok().as_deref() == Some(expected) =>
        {
            let old_link_id: String = row
                .try_get("link_id")
                .map_err(|_| "active modem link invalid")?;
            sqlx::query("UPDATE ont_modem_links SET status='REPLACED',ended_at=now() WHERE link_id=$1 AND status='ACTIVE'")
                .bind(&old_link_id).execute(&mut *transaction).await.map_err(|_| "old modem link update failed")?;
            sqlx::query("UPDATE modems SET status='INACTIVE',updated_at=now() WHERE modem_id=$1 AND status='ACTIVE'")
                .bind(expected).execute(&mut *transaction).await.map_err(|_| "old modem state update failed")?;
        }
        (None, Some(_)) => return Err("ONT already has a modem; specify its ID for replacement"),
        (Some(_), _) => return Err("selected modem is not active on this ONT"),
    }
    let modem_id = format!("MODEM-{}", Uuid::new_v4());
    let link_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO modems (modem_id,serial,vendor,model,created_by_technician,created_by_control_plane_admin) VALUES ($1,$2,$3,$4,NULL,$5)")
        .bind(&modem_id).bind(&input.serial).bind(&input.vendor).bind(&input.model).bind(actor.principal_id)
        .execute(&mut *transaction).await.map_err(|_| "modem creation failed")?;
    sqlx::query("INSERT INTO ont_modem_links (link_id,ont_id,modem_id,status,created_by_technician,created_by_control_plane_admin,source_event) VALUES ($1,$2,$3,'ACTIVE',NULL,$4,$5)")
        .bind(&link_id).bind(&input.ont_id).bind(&modem_id).bind(actor.principal_id).bind(&input.source_event)
        .execute(&mut *transaction).await.map_err(|_| "ONT modem link creation failed")?;
    if replace_modem_id.is_some() {
        let old_link_id: String = active_link
            .as_ref()
            .ok_or("replacement link unavailable")?
            .try_get("link_id")
            .map_err(|_| "replacement link invalid")?;
        sqlx::query("UPDATE ont_modem_links SET replaced_by_link_id=$2 WHERE link_id=$1 AND status='REPLACED'")
            .bind(old_link_id).bind(&link_id).execute(&mut *transaction).await
            .map_err(|_| "replacement history update failed")?;
    }
    let result = json!({"status":"CREATED","resource_type":"MODEM","resource_id":modem_id,"ont_id":input.ont_id,"operation_id":operation_id});
    finish_operation(
        &mut transaction,
        actor,
        operation_id,
        tool,
        "MODEM",
        result["resource_id"].as_str().unwrap_or_default(),
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((transaction, result))
}

pub(crate) async fn begin_transaction(
    pool: &PgPool,
) -> Result<Transaction<'_, Postgres>, &'static str> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| "AAA transaction unavailable")?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "AAA transaction isolation unavailable")?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "lock timeout unavailable")?;
    sqlx::query("SET LOCAL statement_timeout='8s'")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "statement timeout unavailable")?;
    Ok(transaction)
}

pub fn validate_roe(input: &CreateFieldRoe) -> Result<(), &'static str> {
    validate_new_roe(input)
}

pub async fn replace_ont(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    old_ont_id: &str,
    input: &RegisterOnt,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = begin_transaction(pool).await?;
    let (transaction, result) =
        replace_ont_transaction(transaction, actor, operation_id, old_ont_id, input).await?;
    transaction
        .commit()
        .await
        .map_err(|_| "ONT replacement commit failed")?;
    Ok(result)
}

pub async fn replace_ont_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    old_ont_id: &str,
    input: &RegisterOnt,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    actor.require_write_authorization()?;
    validate_ont(input)?;
    crate::domain::physical_asset::identifier(old_ont_id)?;
    let payload = json!({"old_ont_id":old_ont_id,"replacement":input});
    let digest = input_digest(&payload)?;
    if let Some(result) = begin_operation(
        &mut transaction,
        actor,
        operation_id,
        "replace_ont",
        &digest,
    )
    .await?
    {
        return Ok((transaction, result));
    }
    let old=sqlx::query("SELECT o.serial_number,o.roe_id,o.current_port_id,o.customer_id,p.resource_state FROM onts o JOIN ports p ON p.port_id=o.current_port_id WHERE o.ont_id=$1 AND o.status='ACTIVE' FOR UPDATE OF o,p")
        .bind(old_ont_id).fetch_optional(&mut *transaction).await.map_err(|_|"replacement ONT lock failed")?
        .ok_or("old ONT is not active on a registered port")?;
    let port: String = old
        .try_get("current_port_id")
        .map_err(|_| "old ONT port unavailable")?;
    let roe: Option<String> = old
        .try_get("roe_id")
        .map_err(|_| "old ONT ROE unavailable")?;
    let customer: Option<String> = old
        .try_get("customer_id")
        .map_err(|_| "old ONT customer unavailable")?;
    let serial: String = old
        .try_get("serial_number")
        .map_err(|_| "old ONT serial unavailable")?;
    if port != input.gpon_port_id
        || roe.as_deref() != Some(&input.roe_id)
        || old
            .try_get::<String, _>("resource_state")
            .map_err(|_| "port state unavailable")?
            != "OCCUPIED"
    {
        return Err("replacement must preserve the existing occupied GPON port and ROE");
    }
    let other_assigned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM onts WHERE current_port_id=$1 AND ont_id<>$2 AND status='ACTIVE')")
        .bind(&port).bind(old_ont_id).fetch_one(&mut *transaction).await.map_err(|_|"ONT port exclusivity unavailable")?;
    if other_assigned {
        return Err("GPON port has conflicting active assignment");
    }
    let serial_used: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM onts WHERE serial_number=$1)")
            .bind(&input.serial)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| "replacement serial check failed")?;
    if serial_used {
        return Err("replacement ONT serial already registered");
    }
    let old_link=sqlx::query("SELECT l.link_id,l.modem_id FROM ont_modem_links l JOIN modems m ON m.modem_id=l.modem_id WHERE l.ont_id=$1 AND l.status='ACTIVE' AND m.status='ACTIVE' FOR UPDATE OF l,m")
        .bind(old_ont_id).fetch_optional(&mut *transaction).await.map_err(|_|"ONT modem replacement link unavailable")?;
    sqlx::query(
        "UPDATE onts SET status='INACTIVE',updated_at=now() WHERE ont_id=$1 AND status='ACTIVE'",
    )
    .bind(old_ont_id)
    .execute(&mut *transaction)
    .await
    .map_err(|_| "old ONT retirement failed")?;
    let new_ont_id = format!("ONT-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO onts(ont_id,serial_number,roe_id,current_port_id,customer_id,vendor,model,status,created_by_control_plane_admin) VALUES ($1,$2,$3,$4,$5,$6,$7,'ACTIVE',$8)")
        .bind(&new_ont_id).bind(&input.serial).bind(&input.roe_id).bind(&port).bind(&customer).bind(&input.vendor).bind(&input.model).bind(actor.principal_id)
        .execute(&mut *transaction).await.map_err(|_|"replacement ONT registration failed")?;
    if let Some(link) = old_link {
        let old_link_id: String = link.try_get("link_id").map_err(|_| "old link invalid")?;
        let modem: String = link.try_get("modem_id").map_err(|_| "old modem invalid")?;
        let new_link_id = Uuid::new_v4().to_string();
        sqlx::query("UPDATE ont_modem_links SET status='REPLACED',ended_at=now() WHERE link_id=$1")
            .bind(&old_link_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| "old ONT link replacement failed")?;
        sqlx::query("INSERT INTO ont_modem_links(link_id,ont_id,modem_id,status,created_by_control_plane_admin,source_event) VALUES ($1,$2,$3,'ACTIVE',$4,$5)")
            .bind(&new_link_id).bind(&new_ont_id).bind(modem).bind(actor.principal_id).bind(&input.source_event)
            .execute(&mut *transaction).await.map_err(|_|"replacement modem transfer failed")?;
        sqlx::query("UPDATE ont_modem_links SET replaced_by_link_id=$2 WHERE link_id=$1")
            .bind(&old_link_id)
            .bind(&new_link_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| "ONT replacement history failed")?;
    }
    let result = json!({"status":"REPLACED","resource_id":new_ont_id,"old_ont_id":old_ont_id,"before":{"serial":serial,"port":port,"customer_id":customer},"after":{"serial":input.serial,"port":port,"customer_id":customer}});
    finish_operation(
        &mut transaction,
        actor,
        operation_id,
        "replace_ont",
        "ONT",
        &new_ont_id,
        &digest,
        payload,
        json!({"status":"PASS","port_released":false}),
        result.clone(),
    )
    .await?;
    Ok((transaction, result))
}

pub fn validate_ont(input: &RegisterOnt) -> Result<(), &'static str> {
    validate_serial(&input.serial)?;
    crate::domain::field_ops::validate_gpon_port_id(&input.gpon_port_id)?;
    validate_registration_metadata(
        input.vendor.as_deref(),
        input.model.as_deref(),
        &input.source_event,
    )
}

pub fn validate_modem(input: &RegisterModem) -> Result<(), &'static str> {
    validate_serial(&input.serial)?;
    if input.ont_id.trim().is_empty() || input.ont_id.len() > 64 {
        return Err("ONT identifier is invalid");
    }
    validate_registration_metadata(
        input.vendor.as_deref(),
        input.model.as_deref(),
        &input.source_event,
    )
}

pub async fn dry_run_roe(pool: &PgPool, input: &CreateFieldRoe) -> Result<Value, &'static str> {
    validate_roe(input)?;
    let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM cabinets WHERE cabinet_id=$1 AND status='ACTIVE') AS cabinet_active,EXISTS(SELECT 1 FROM splitters WHERE splitter_id=$2 AND cabinet_id=$1 AND status='ACTIVE') AS splitter_active,EXISTS(SELECT 1 FROM roes WHERE roe_id=$3) AS roe_exists,(SELECT cabinet_id FROM feeder_cables WHERE cable_id=$4) AS cable_cabinet,(SELECT fiber_count FROM feeder_cables WHERE cable_id=$4) AS cable_fiber_count,(SELECT fibers_used FROM feeder_cables WHERE cable_id=$4) AS fibers_used,(SELECT fibers_reserved FROM feeder_cables WHERE cable_id=$4) AS fibers_reserved,(SELECT status FROM feeder_cables WHERE cable_id=$4) AS cable_status")
        .bind(&input.cabinet_id)
        .bind(&input.splitter_id)
        .bind(&input.roe_id)
        .bind(&input.feeder_cable_id)
        .fetch_one(pool)
        .await
        .map_err(|_| "AAA topology validation query failed")?;
    let cabinet_active: bool = row
        .try_get("cabinet_active")
        .map_err(|_| "cabinet state unavailable")?;
    let splitter_active: bool = row
        .try_get("splitter_active")
        .map_err(|_| "splitter state unavailable")?;
    let roe_exists: bool = row
        .try_get("roe_exists")
        .map_err(|_| "ROE state unavailable")?;
    let cable_cabinet: Option<String> = row
        .try_get("cable_cabinet")
        .map_err(|_| "feeder state unavailable")?;
    let cable_fiber_count: Option<i32> = row
        .try_get("cable_fiber_count")
        .map_err(|_| "feeder state unavailable")?;
    let fibers_used: Option<i32> = row
        .try_get("fibers_used")
        .map_err(|_| "feeder state unavailable")?;
    let fibers_reserved: Option<i32> = row
        .try_get("fibers_reserved")
        .map_err(|_| "feeder state unavailable")?;
    let cable_status: Option<String> = row
        .try_get("cable_status")
        .map_err(|_| "feeder state unavailable")?;
    let cable_matches = match (
        cable_cabinet,
        cable_fiber_count,
        fibers_used,
        fibers_reserved,
        cable_status,
    ) {
        (None, None, None, None, None) => true,
        (Some(cabinet), Some(total), Some(used), Some(reserved), Some(status)) => {
            cabinet == input.cabinet_id
                && total == input.feeder_fiber_count
                && status == "ACTIVE"
                && used >= 0
                && reserved >= 0
                && used
                    .checked_add(reserved)
                    .and_then(|allocated| allocated.checked_add(input.fibers_allocated))
                    .is_some_and(|allocated| allocated <= total)
        }
        _ => false,
    };
    let valid = cabinet_active && splitter_active && !roe_exists && cable_matches;
    Ok(json!({
        "status": if valid { "PASS" } else { "DENY" },
        "persisted": false,
        "checks": {
            "cabinet_active": cabinet_active,
            "splitter_belongs_to_cabinet": splitter_active,
            "roe_id_available": !roe_exists,
            "feeder_capacity_available": cable_matches
        },
        "roe_id": input.roe_id,
        "feeder_cable_id": input.feeder_cable_id,
        "fibers_allocated": input.fibers_allocated
    }))
}

pub async fn dry_run_ont(pool: &PgPool, input: &RegisterOnt) -> Result<Value, &'static str> {
    validate_ont(input)?;
    let row = sqlx::query("SELECT p.resource_state,(p.splitter_id=r.splitter_id AND s.cabinet_id=r.cabinet_id AND s.status='ACTIVE' AND r.status='ACTIVE') AS topology_valid,EXISTS(SELECT 1 FROM onts o WHERE o.current_port_id=p.port_id AND (o.status='ACTIVE' OR o.customer_id IS NOT NULL)) AS assignment_exists,EXISTS(SELECT 1 FROM onts o WHERE o.serial_number=$3) AS serial_exists FROM ports p JOIN splitters s ON s.splitter_id=p.splitter_id JOIN roes r ON r.roe_id=$1 WHERE p.port_id=$2")
        .bind(&input.roe_id)
        .bind(&input.gpon_port_id)
        .bind(&input.serial)
        .fetch_optional(pool)
        .await
        .map_err(|_| "AAA ONT validation query failed")?
        .ok_or("GPON port does not belong to the selected ROE")?;
    let port_state: String = row
        .try_get("resource_state")
        .map_err(|_| "GPON port state unavailable")?;
    let topology_valid: bool = row
        .try_get("topology_valid")
        .map_err(|_| "GPON topology state unavailable")?;
    let assignment_exists: bool = row
        .try_get("assignment_exists")
        .map_err(|_| "GPON assignment state unavailable")?;
    let serial_exists: bool = row
        .try_get("serial_exists")
        .map_err(|_| "ONT serial state unavailable")?;
    let resource_free = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM resource_states WHERE resource_id=$1 AND resource_kind='PORT' AND resource_state='FREE' AND owner_transaction_id IS NULL) AND NOT EXISTS(SELECT 1 FROM resource_leases WHERE resource_id=$1 AND released_at IS NULL AND expires_at>now())")
        .bind(&input.gpon_port_id).fetch_one(pool).await.map_err(|_| "GPON resource ownership check failed")?;
    let port_free = port_state == "FREE" && !assignment_exists && topology_valid && resource_free;
    let valid = port_free && !serial_exists;
    Ok(json!({
        "status": if valid { "PASS" } else { "DENY" },
        "persisted": false,
        "checks": {
            "port_free_and_unassigned": port_free,
            "serial_available": !serial_exists
        },
        "roe_id": input.roe_id,
        "gpon_port_id": input.gpon_port_id
    }))
}

pub async fn dry_run_modem(
    pool: &PgPool,
    input: &RegisterModem,
    replace_modem_id: Option<&str>,
) -> Result<Value, &'static str> {
    validate_modem(input)?;
    let ont_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM onts WHERE ont_id=$1 AND status='ACTIVE')",
    )
    .bind(&input.ont_id)
    .fetch_one(pool)
    .await
    .map_err(|_| "AAA modem validation query failed")?;
    let serial_exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM modems WHERE serial=$1)")
            .bind(&input.serial)
            .fetch_one(pool)
            .await
            .map_err(|_| "AAA modem serial check failed")?;
    let existing_link_valid = if let Some(old_modem_id) = replace_modem_id {
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM ont_modem_links l JOIN modems m ON m.modem_id=l.modem_id WHERE l.ont_id=$1 AND l.modem_id=$2 AND l.status='ACTIVE' AND m.status='ACTIVE')")
            .bind(&input.ont_id)
            .bind(old_modem_id)
            .fetch_one(pool)
            .await
            .map_err(|_| "active modem link check failed")?
    } else {
        !sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM ont_modem_links WHERE ont_id=$1 AND status='ACTIVE')",
        )
        .bind(&input.ont_id)
        .fetch_one(pool)
        .await
        .map_err(|_| "existing modem link check failed")?
    };
    let valid = ont_active && !serial_exists && existing_link_valid;
    Ok(json!({
        "status": if valid { "PASS" } else { "DENY" },
        "persisted": false,
        "checks": {
            "ont_active": ont_active,
            "serial_available": !serial_exists,
            "link_state_valid": existing_link_valid
        },
        "ont_id": input.ont_id,
        "replacement": replace_modem_id.is_some()
    }))
}

#[cfg(test)]
mod tests {
    struct SyntheticPhotoDirectory(std::path::PathBuf);
    impl Drop for SyntheticPhotoDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[tokio::test]
    #[ignore = "requires user-authorized AAA rollback test and verified encrypted backup"]
    async fn aaa_services_create_update_rollback() -> Result<(), Box<dyn std::error::Error>> {
        if std::env::var("FIBERSPIDER_AUTHORIZED_AAA_ROLLBACK").as_deref() != Ok("yes") {
            return Err("explicit AAA rollback test opt-in required".into());
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .ok_or("root unavailable")?;
        let config = crate::config::load_database_from_file(&root.join("secrets/runtime.env"))?;
        let pool = crate::db::pool::connect(&config).await?;
        let actor = crate::mcp::auth::ControlPlaneAdminContext::for_authorized_rollback_test();
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let cabinet = format!("CAB-TEST-{suffix}");
        let line = format!("LINE-TEST-{suffix}");
        let splitter = format!("SPL-TEST-{suffix}");
        let port = format!("PORT-TEST-{suffix}");
        let roe = format!("ROE-TEST-{suffix}");
        let cable = format!("CAV-TEST-{suffix}");
        let operation = format!("TEST-{suffix}");
        let work_order = format!("WR-TEST-{suffix}");
        let technician = format!("TECH-TEST-{suffix}");
        let photo_root = SyntheticPhotoDirectory(
            std::env::temp_dir().join(format!("photaxon-photo-test-{suffix}")),
        );
        std::fs::create_dir(&photo_root.0)?;
        let mut transaction = super::begin_transaction(&pool).await?;
        let schema_existed: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.control_plane_admin_operations') IS NOT NULL",
        )
        .fetch_one(&mut *transaction)
        .await?;
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/011_control_plane_admin.sql"
        ))
        .execute(&mut *transaction)
        .await?;
        let photo_schema_existed: bool =
            sqlx::query_scalar("SELECT to_regclass('public.admin_photo_catalog') IS NOT NULL")
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/012_admin_photo_catalog.sql"
        ))
        .execute(&mut *transaction)
        .await?;
        let physical_schema_existed: bool =
            sqlx::query_scalar("SELECT to_regclass('public.physical_asset_records') IS NOT NULL")
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/013_physical_assets.sql"
        ))
        .execute(&mut *transaction)
        .await?;
        let delivery_schema_existed: bool =
            sqlx::query_scalar("SELECT to_regclass('public.delivery_work_details') IS NOT NULL")
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/014_delivery_work_details.sql"
        ))
        .execute(&mut *transaction)
        .await?;
        let one_schema_existed: bool =
            sqlx::query_scalar("SELECT to_regclass('public.ont_acceptance_tests') IS NOT NULL")
                .fetch_one(&mut *transaction)
                .await?;
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/015_one_acceptance_tests.sql"
        ))
        .execute(&mut *transaction)
        .await?;
        sqlx::raw_sql(include_str!(
            "../../../db/migrations/016_legacy_asset_provenance.sql"
        ))
        .execute(&mut *transaction)
        .await?;
        sqlx::query("INSERT INTO technicians(technician_id,status) VALUES ($1,'ACTIVE')")
            .bind(&technician)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO activation_requests(activation_id,technician_id,wr,ont_serial,status) VALUES ($1,$2,$3,$1,'TEST_ROLLBACK')")
            .bind(&operation).bind(&technician).bind(&work_order).execute(&mut *transaction).await?;
        sqlx::query("INSERT INTO cabinets(cabinet_id,status) VALUES ($1,'ACTIVE')")
            .bind(&cabinet)
            .execute(&mut *transaction)
            .await?;
        sqlx::query("INSERT INTO ftth_lines(line_id,cabinet_id,openstack_network_name,status) VALUES ($1,$2,$1,'ACTIVE')")
            .bind(&line).bind(&cabinet).execute(&mut *transaction).await?;
        sqlx::query("INSERT INTO splitters(splitter_id,cabinet_id,line_id,ratio,capacity_total,status) VALUES ($1,$2,$3,8,8,'ACTIVE')")
            .bind(&splitter).bind(&cabinet).bind(&line).execute(&mut *transaction).await?;
        sqlx::query("INSERT INTO ports(port_id,splitter_id,position,resource_state) VALUES ($1,$2,1,'FREE')")
            .bind(&port).bind(&splitter).execute(&mut *transaction).await?;
        sqlx::query("INSERT INTO resource_states(resource_id,resource_kind,resource_state) VALUES ($1,'PORT','FREE')")
            .bind(&port).execute(&mut *transaction).await?;
        let input = crate::domain::field_ops::CreateFieldRoe {
            roe_id: roe.clone(),
            cabinet_id: cabinet.clone(),
            splitter_id: splitter.clone(),
            street_name: "SYNTHETIC ROLLBACK TEST".into(),
            street_number: "1".into(),
            city: "TEST".into(),
            postal_code: "00000".into(),
            latitude: 0.0,
            longitude: 0.0,
            location_accuracy_m: 1.0,
            geolocation_source: "CONTROL_PLANE_ADMIN".into(),
            feeder_cable_id: cable.clone(),
            feeder_fiber_count: 12,
            fibers_allocated: 1,
            address_note: None,
            source_event: operation.clone(),
        };
        let (transaction, first) =
            super::create_roe_transaction(transaction, &actor, &operation, &input).await?;
        let (transaction, replay) =
            super::create_roe_transaction(transaction, &actor, &operation, &input).await?;
        assert_eq!(first, replay);
        let ont_input = crate::domain::field_ops::RegisterOnt {
            serial: format!("ONT-TEST-{suffix}"),
            gpon_port_id: port.clone(),
            roe_id: roe.clone(),
            vendor: None,
            model: None,
            source_event: operation.clone(),
        };
        let (transaction, ont) = super::register_ont_transaction(
            transaction,
            &actor,
            &format!("{operation}-ONT"),
            &ont_input,
        )
        .await?;
        let ont_id = ont["resource_id"]
            .as_str()
            .ok_or("ONT ID missing")?
            .to_owned();
        let modem_input = crate::domain::field_ops::RegisterModem {
            serial: format!("MODEM-TEST-{suffix}"),
            ont_id: ont_id.clone(),
            vendor: None,
            model: None,
            source_event: operation.clone(),
        };
        let (transaction, modem) = super::modem_transaction(
            transaction,
            &actor,
            &format!("{operation}-MODEM"),
            &modem_input,
            None,
        )
        .await?;
        let old_modem = modem["resource_id"]
            .as_str()
            .ok_or("modem ID missing")?
            .to_owned();
        let replacement = crate::domain::field_ops::RegisterModem {
            serial: format!("MODEM-REPLACEMENT-{suffix}"),
            ..modem_input
        };
        let (mut transaction, replaced) = super::modem_transaction(
            transaction,
            &actor,
            &format!("{operation}-REPLACE"),
            &replacement,
            Some(&old_modem),
        )
        .await?;
        let new_modem = replaced["resource_id"]
            .as_str()
            .ok_or("replacement ID missing")?
            .to_owned();
        let updated: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM ont_modem_links WHERE modem_id=$1 AND status='REPLACED' AND ended_at IS NOT NULL AND replaced_by_link_id IS NOT NULL) AND EXISTS(SELECT 1 FROM ont_modem_links WHERE modem_id=$2 AND ont_id=$3 AND status='ACTIVE')")
            .bind(&old_modem).bind(&new_modem).bind(&ont_id).fetch_one(&mut *transaction).await?;
        assert!(updated);
        use crate::services::admin_photo_service::{
            PhotoAssetType, PhotoAssociation, PhotoWrite, write_photo_transaction,
        };
        let association = PhotoAssociation {
            work_order_id: work_order.clone(),
            asset_type: PhotoAssetType::Roe,
            asset_id: roe.clone(),
            evidence_type: "TEST_BEFORE".into(),
            notes: None,
        };
        let image = [255, 216, 255, 0];
        let upload_operation = format!("{operation}-PHOTO");
        let (transaction, photo) = write_photo_transaction(
            transaction,
            &actor,
            &photo_root.0,
            PhotoWrite {
                operation_id: &upload_operation,
                association: &association,
                content_type: "image/jpeg",
                bytes: &image,
                previous_storage_key: None,
            },
        )
        .await?;
        let (transaction, replayed) = write_photo_transaction(
            transaction,
            &actor,
            &photo_root.0,
            PhotoWrite {
                operation_id: &upload_operation,
                association: &association,
                content_type: "image/jpeg",
                bytes: &image,
                previous_storage_key: None,
            },
        )
        .await?;
        assert_eq!(photo, replayed);
        let storage = photo["storage_key"]
            .as_str()
            .ok_or("photo storage missing")?;
        let original_path = photo_root.0.join("secrets/field-media").join(storage);
        assert_eq!(std::fs::read(&original_path)?, image);
        let replacement_image = [255, 216, 255, 1];
        let (mut transaction, replacement_photo) = write_photo_transaction(
            transaction,
            &actor,
            &photo_root.0,
            PhotoWrite {
                operation_id: &format!("{operation}-PHOTO-REPLACE"),
                association: &association,
                content_type: "image/jpeg",
                bytes: &replacement_image,
                previous_storage_key: Some(storage),
            },
        )
        .await?;
        assert_eq!(std::fs::read(&original_path)?, image);
        let replacement_path = photo_root.0.join("secrets/field-media").join(
            replacement_photo["storage_key"]
                .as_str()
                .ok_or("replacement path missing")?,
        );
        assert_eq!(std::fs::read(&replacement_path)?, replacement_image);
        let versions: i64 =
            sqlx::query_scalar("SELECT count(*) FROM admin_photo_versions WHERE media_id=$1")
                .bind(photo["media_id"].as_str().ok_or("photo ID missing")?)
                .fetch_one(&mut *transaction)
                .await?;
        assert_eq!(versions, 2);
        use crate::domain::physical_asset::{
            Address, AssetDetails, AssetState, CableType, PhysicalAsset,
        };
        use crate::services::physical_asset_service::save_transaction;
        let physical_cab = format!("PHYSICAL-CAB-{suffix}");
        let physical_closure = format!("PHYSICAL-CLOSURE-{suffix}");
        let physical_cable = format!("PHYSICAL-CABLE-{suffix}");
        let physical_tube = format!("PHYSICAL-TUBE-{suffix}");
        let physical_fiber = format!("PHYSICAL-FIBER-{suffix}");
        let address = Address {
            street_name: "SYNTHETIC TEST".into(),
            street_number: "1".into(),
            postal_code: "00000".into(),
            city: "TEST".into(),
            province: "TT".into(),
            address_note: None,
        };
        let import_operation = format!("{operation}-LEGACY-CABINET");
        let (transaction, legacy_import) =
            crate::services::legacy_asset_service::import_transaction(
                transaction,
                &actor,
                &import_operation,
                &crate::services::legacy_asset_service::LegacyAssetKind::Cabinet,
                &cabinet,
            )
            .await?;
        let (mut transaction, legacy_replay) =
            crate::services::legacy_asset_service::import_transaction(
                transaction,
                &actor,
                &import_operation,
                &crate::services::legacy_asset_service::LegacyAssetKind::Cabinet,
                &cabinet,
            )
            .await?;
        assert_eq!(legacy_import, legacy_replay);
        let imported: Value =
            sqlx::query_scalar("SELECT attributes FROM physical_asset_records WHERE asset_id=$1")
                .bind(&cabinet)
                .fetch_one(&mut *transaction)
                .await?;
        assert!(imported["address"].is_null());
        assert!(imported["geolocation"].is_null());
        let secondary_id = format!("SPL-SECONDARY-TEST-{suffix}");
        let secondary = PhysicalAsset {
            asset_id: secondary_id.clone(),
            operational_code: format!("SECONDARY TEST {suffix}"),
            state: AssetState::Active,
            address: address.clone(),
            geolocation: None,
            asset: AssetDetails::Splitter {
                level: 2,
                ratio: 16,
                location_type: crate::domain::physical_asset::SplitterLocation::Cro,
                container_id: cabinet.clone(),
                physical_position: "SYNTHETIC TEST SLOT".into(),
                attenuation_db: None,
            },
            notes: None,
        };
        let (transaction, _) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-LEGACY-SECONDARY"),
            &secondary,
            None,
        )
        .await?;
        let existing_key = format!("{work_order}/existing-test.jpg");
        let existing_path = photo_root.0.join("secrets/field-media").join(&existing_key);
        std::fs::write(&existing_path, image)?;
        let catalog_operation = format!("{operation}-CATALOG-EXISTING");
        let (transaction, cataloged) =
            crate::services::admin_photo_service::catalog_existing_transaction(
                transaction,
                &actor,
                crate::services::admin_photo_service::ExistingPhoto {
                    operation_id: &catalog_operation,
                    association: &association,
                    storage_key: &existing_key,
                    content_type: "image/jpeg",
                    bytes: &image,
                },
            )
            .await?;
        let (transaction, catalog_replay) =
            crate::services::admin_photo_service::catalog_existing_transaction(
                transaction,
                &actor,
                crate::services::admin_photo_service::ExistingPhoto {
                    operation_id: &catalog_operation,
                    association: &association,
                    storage_key: &existing_key,
                    content_type: "image/jpeg",
                    bytes: &image,
                },
            )
            .await?;
        assert_eq!(cataloged, catalog_replay);
        assert_eq!(std::fs::read(&existing_path)?, image);
        let cabinet_asset = PhysicalAsset {
            asset_id: physical_cab.clone(),
            operational_code: format!("ARLO {suffix}"),
            state: AssetState::Active,
            address: address.clone(),
            geolocation: None,
            asset: AssetDetails::Cabinet {
                cabinet_type: "ARLO".into(),
                central_area: "TEST".into(),
            },
            notes: None,
        };
        let (transaction, _) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-PHYCAB"),
            &cabinet_asset,
            None,
        )
        .await?;
        let mut closure_asset = PhysicalAsset {
            asset_id: physical_closure.clone(),
            operational_code: format!("MUFFOLA {suffix}"),
            state: AssetState::Active,
            address: address.clone(),
            geolocation: None,
            asset: AssetDetails::Closure {
                model: "SYNTHETIC".into(),
                placement: "POZZETTO".into(),
                tray_count: 2,
            },
            notes: None,
        };
        let (transaction, _) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-PHYCLOSURE"),
            &closure_asset,
            None,
        )
        .await?;
        closure_asset.notes = Some("synthetic update".into());
        let (transaction, updated) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-PHYCLOSURE-UPDATE"),
            &closure_asset,
            Some(1),
        )
        .await?;
        assert_eq!(updated["revision"], 2);
        let cable_asset = PhysicalAsset {
            asset_id: physical_cable.clone(),
            operational_code: format!("CABLE {suffix}"),
            state: AssetState::Active,
            address: address.clone(),
            geolocation: None,
            asset: AssetDetails::Cable {
                cable_type: CableType::Secondary,
                fiber_count: 48,
                origin_id: physical_cab.clone(),
                destination_id: physical_closure.clone(),
                length_m: 100.0,
                route: "TEST ROUTE".into(),
            },
            notes: None,
        };
        let (transaction, _) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-PHYCABLE"),
            &cable_asset,
            None,
        )
        .await?;
        let tube_asset = PhysicalAsset {
            asset_id: physical_tube.clone(),
            operational_code: format!("TUBE {suffix}"),
            state: AssetState::Active,
            address: address.clone(),
            geolocation: None,
            asset: AssetDetails::Tube {
                cable_id: physical_cable.clone(),
                number: 1,
                color: "verde".into(),
                fiber_count: 12,
            },
            notes: None,
        };
        let (transaction, _) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-PHYTUBE"),
            &tube_asset,
            None,
        )
        .await?;
        let fiber_asset = PhysicalAsset {
            asset_id: physical_fiber.clone(),
            operational_code: format!("FIBER {suffix}"),
            state: AssetState::Active,
            address,
            geolocation: None,
            asset: AssetDetails::Fiber {
                cable_id: physical_cable.clone(),
                tube_id: physical_tube.clone(),
                number: 1,
                color: "rosso".into(),
            },
            notes: None,
        };
        let (mut transaction, _) = save_transaction(
            transaction,
            &actor,
            &format!("{operation}-PHYFIBER"),
            &fiber_asset,
            None,
        )
        .await?;
        let stored_fiber: serde_json::Value =
            sqlx::query_scalar("SELECT attributes FROM physical_asset_records WHERE asset_id=$1")
                .bind(&physical_fiber)
                .fetch_one(&mut *transaction)
                .await?;
        assert_eq!(stored_fiber["asset"]["details"]["color"], "rosso");
        let delivery_wr = format!("WR-DELIVERY-{suffix}");
        let delivery_activation = format!("ACT-DELIVERY-{suffix}");
        let mut delivery = crate::domain::delivery::Work {
            activation_id: delivery_activation.clone(),
            work_order_id: delivery_wr.clone(),
            operator_name: "SYNTHETIC TEST".into(),
            technician_id: None,
            ont_serial: None,
            customer_id: None,
            customer_phone: None,
            address: cabinet_asset.address.clone(),
            geolocation: None,
            appointment_at: None,
            team_id: None,
            status: crate::domain::delivery::WorkStatus::New,
            ko_reason: None,
            notes: None,
        };
        let (transaction, work_created) = crate::services::delivery_service::save_work_transaction(
            transaction,
            &actor,
            &format!("{operation}-WORK"),
            &delivery,
            None,
        )
        .await?;
        let (transaction, work_replayed) =
            crate::services::delivery_service::save_work_transaction(
                transaction,
                &actor,
                &format!("{operation}-WORK"),
                &delivery,
                None,
            )
            .await?;
        assert_eq!(work_created, work_replayed);
        delivery.status = crate::domain::delivery::WorkStatus::Scheduled;
        delivery.technician_id = Some(technician.clone());
        delivery.appointment_at = Some("2026-11-09T09:00:00+01:00".into());
        let (mut transaction, work_updated) =
            crate::services::delivery_service::save_work_transaction(
                transaction,
                &actor,
                &format!("{operation}-WORK-UPDATE"),
                &delivery,
                Some(1),
            )
            .await?;
        assert_eq!(work_updated["revision"], 2);
        let scheduled:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM delivery_work_details WHERE activation_id=$1 AND work_status='SCHEDULED' AND appointment_at IS NOT NULL)")
            .bind(&delivery_activation).fetch_one(&mut *transaction).await?;
        assert!(scheduled);
        let replacement_ont = crate::domain::field_ops::RegisterOnt {
            serial: format!("ONT-REPLACEMENT-{suffix}"),
            ..ont_input
        };
        let (mut transaction, ont_replaced) = super::replace_ont_transaction(
            transaction,
            &actor,
            &format!("{operation}-ONT-REPLACE"),
            &ont_id,
            &replacement_ont,
        )
        .await?;
        let replacement_ont_id = ont_replaced["resource_id"]
            .as_str()
            .ok_or("replacement ONT ID missing")?
            .to_owned();
        let preserved:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM onts WHERE ont_id=$1 AND status='INACTIVE') AND EXISTS(SELECT 1 FROM onts WHERE ont_id=$2 AND current_port_id=$3 AND status='ACTIVE') AND EXISTS(SELECT 1 FROM ports WHERE port_id=$3 AND resource_state='OCCUPIED') AND EXISTS(SELECT 1 FROM ont_modem_links WHERE ont_id=$2 AND modem_id=$4 AND status='ACTIVE')")
            .bind(&ont_id).bind(&replacement_ont_id).bind(&port).bind(&new_modem).fetch_one(&mut *transaction).await?;
        assert!(preserved);
        let mut assignment = crate::domain::resource_assignment::ResourceAssignment {
            resource_id: physical_fiber.clone(),
            work_order_id: work_order.clone(),
            customer_id: None,
            action: crate::domain::resource_assignment::AssignmentAction::Reserve,
            reason: "synthetic rollback test".into(),
        };
        let (transaction, reserved) =
            crate::services::resource_assignment_service::change_transaction(
                transaction,
                &actor,
                &format!("{operation}-RESERVE"),
                &assignment,
            )
            .await?;
        assignment.action = crate::domain::resource_assignment::AssignmentAction::Release;
        let (mut transaction, released) =
            crate::services::resource_assignment_service::change_transaction(
                transaction,
                &actor,
                &format!("{operation}-RELEASE"),
                &assignment,
            )
            .await?;
        assert_eq!(reserved["assignment_id"], released["assignment_id"]);
        let history:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM physical_resource_assignments WHERE resource_id=$1 AND status='RELEASED' AND released_at IS NOT NULL)")
            .bind(&physical_fiber).fetch_one(&mut *transaction).await?;
        assert!(history);
        transaction = crate::services::ont_acceptance_service::exercise_synthetic_rollback(
            transaction,
            &actor,
            &delivery_wr,
            &replacement_ont_id,
            &operation,
        )
        .await?;
        let pattern = format!("{operation}%");
        let audits: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM control_plane_admin_audit WHERE operation_id LIKE $1",
        )
        .bind(&pattern)
        .fetch_one(&mut *transaction)
        .await?;
        assert_eq!(audits, 25);
        transaction.rollback().await?;
        let remaining: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM cabinets WHERE cabinet_id=$1)+(SELECT count(*) FROM ftth_lines WHERE line_id=$2)+(SELECT count(*) FROM splitters WHERE splitter_id=$3)+(SELECT count(*) FROM ports WHERE port_id=$4)+(SELECT count(*) FROM resource_states WHERE resource_id=$4)+(SELECT count(*) FROM roes WHERE roe_id=$5)+(SELECT count(*) FROM feeder_cables WHERE cable_id=$6)+(SELECT count(*) FROM feeder_cable_allocations WHERE roe_id=$5)+(SELECT count(*) FROM onts WHERE ont_id=$7)+(SELECT count(*) FROM modems WHERE modem_id IN ($8,$9))+(SELECT count(*) FROM ont_modem_links WHERE ont_id=$7)")
            .bind(&cabinet).bind(&line).bind(&splitter).bind(&port).bind(&roe).bind(&cable)
            .bind(&ont_id).bind(&old_modem).bind(&new_modem).fetch_one(&pool).await?;
        assert_eq!(remaining, 0);
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM onts WHERE ont_id=$1)+(SELECT count(*) FROM ont_modem_links WHERE ont_id=$1)")
            .bind(&replacement_ont_id).fetch_one(&pool).await?;
        assert_eq!(remaining, 0);
        let one_schema_after: bool =
            sqlx::query_scalar("SELECT to_regclass('public.ont_acceptance_tests') IS NOT NULL")
                .fetch_one(&pool)
                .await?;
        assert_eq!(one_schema_existed, one_schema_after);
        let synthetic_customer_remains: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM customers WHERE customer_id=$1)")
                .bind(format!("{operation}-CUSTOMER"))
                .fetch_one(&pool)
                .await?;
        assert!(!synthetic_customer_remains);
        if one_schema_after {
            let remaining:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM ont_acceptance_tests WHERE work_order_id=$1)+(SELECT count(*) FROM delivery_closure_outbox WHERE work_order_id=$1)+(SELECT count(*) FROM optical_precheck_profiles WHERE profile_id=$2)")
                .bind(&delivery_wr).bind(format!("{operation}-OPTICAL-PROFILE")).fetch_one(&pool).await?;
            assert_eq!(remaining, 0);
        }
        let remaining:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM technicians WHERE technician_id=$1)+(SELECT count(*) FROM activation_requests WHERE wr=$2)")
            .bind(&technician).bind(&work_order).fetch_one(&pool).await?;
        assert_eq!(remaining, 0);
        let remaining: i64 =
            sqlx::query_scalar("SELECT count(*) FROM activation_requests WHERE activation_id=$1")
                .bind(&delivery_activation)
                .fetch_one(&pool)
                .await?;
        assert_eq!(remaining, 0);
        let delivery_schema_after: bool =
            sqlx::query_scalar("SELECT to_regclass('public.delivery_work_details') IS NOT NULL")
                .fetch_one(&pool)
                .await?;
        assert_eq!(delivery_schema_after, delivery_schema_existed);
        if delivery_schema_after {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM delivery_work_details WHERE activation_id=$1",
            )
            .bind(&delivery_activation)
            .fetch_one(&pool)
            .await?;
            assert_eq!(remaining, 0);
        }
        let photo_schema_after: bool =
            sqlx::query_scalar("SELECT to_regclass('public.admin_photo_catalog') IS NOT NULL")
                .fetch_one(&pool)
                .await?;
        assert_eq!(photo_schema_existed, photo_schema_after);
        if photo_schema_after {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM admin_photo_catalog WHERE work_order_id=$1",
            )
            .bind(&work_order)
            .fetch_one(&pool)
            .await?;
            assert_eq!(remaining, 0);
        }
        let physical_schema_after: bool =
            sqlx::query_scalar("SELECT to_regclass('public.physical_asset_records') IS NOT NULL")
                .fetch_one(&pool)
                .await?;
        assert_eq!(physical_schema_after, physical_schema_existed);
        if physical_schema_after {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM physical_asset_records WHERE asset_id IN ($1,$2)",
            )
            .bind(&cabinet)
            .bind(&secondary_id)
            .fetch_one(&pool)
            .await?;
            assert_eq!(remaining, 0);
            let remaining: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM physical_asset_records WHERE asset_id IN ($1,$2,$3,$4,$5)",
            )
            .bind(&physical_cab)
            .bind(&physical_closure)
            .bind(&physical_cable)
            .bind(&physical_tube)
            .bind(&physical_fiber)
            .fetch_one(&pool)
            .await?;
            assert_eq!(remaining, 0);
            let remaining: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM physical_resource_assignments WHERE resource_id=$1",
            )
            .bind(&physical_fiber)
            .fetch_one(&pool)
            .await?;
            assert_eq!(remaining, 0);
        }
        let schema_after: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.control_plane_admin_operations') IS NOT NULL",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(schema_existed, schema_after);
        if schema_after {
            let remaining: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM control_plane_admin_operations WHERE operation_id LIKE $1)+(SELECT count(*) FROM control_plane_admin_audit WHERE operation_id LIKE $1)")
                .bind(&pattern).fetch_one(&pool).await?;
            assert_eq!(remaining, 0);
        }
        pool.close().await;
        Ok(())
    }
    use super::*;

    #[test]
    fn operation_digest_changes_with_input_but_not_json_key_order() {
        assert_eq!(
            input_digest(&json!({"a":1,"b":2})),
            input_digest(&json!({"b":2,"a":1}))
        );
        assert_ne!(input_digest(&json!({"a":1})), input_digest(&json!({"a":2})));
    }

    #[test]
    fn ont_validator_rejects_invalid_port_and_serial() {
        let mut input = RegisterOnt {
            serial: "SYNTHETIC-ONT".into(),
            gpon_port_id: "SPL-CHECK-PORT-01".into(),
            roe_id: "ROE-CHECK".into(),
            vendor: None,
            model: None,
            source_event: "CHECK-1".into(),
        };
        assert!(validate_ont(&input).is_ok());
        input.gpon_port_id = "../port".into();
        assert!(validate_ont(&input).is_err());
        input.gpon_port_id = "PORT-1".into();
        input.serial = "invalid\nserial".into();
        assert!(validate_ont(&input).is_err());
    }

    #[test]
    fn modem_validator_requires_target_and_source_event() {
        let mut input = RegisterModem {
            serial: "SYNTHETIC-MODEM".into(),
            ont_id: "ONT-CHECK".into(),
            vendor: None,
            model: None,
            source_event: "CHECK-1".into(),
        };
        assert!(validate_modem(&input).is_ok());
        input.ont_id.clear();
        assert!(validate_modem(&input).is_err());
        input.ont_id = "ONT-CHECK".into();
        input.source_event.clear();
        assert!(validate_modem(&input).is_err());
    }
}
