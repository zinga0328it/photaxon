use crate::{
    domain::delivery::{self, Work},
    mcp::auth::ControlPlaneAdminContext,
    services::control_plane_admin_service as operations,
};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};

pub async fn get_work(pool: &PgPool, wr: &str) -> Result<Option<Value>, &'static str> {
    crate::domain::physical_asset::identifier(wr)?;
    let rows=sqlx::query_scalar("SELECT jsonb_build_object('activation_id',a.activation_id,'work_order_id',a.wr,'technician_id',a.technician_id,'ont_serial',a.ont_serial,'activation_status',a.status,'details',to_jsonb(w)) FROM activation_requests a LEFT JOIN delivery_work_details w ON w.activation_id=a.activation_id WHERE a.wr=$1 ORDER BY a.activation_id LIMIT 2")
        .bind(wr).fetch_all(pool).await.map_err(|_|"AAA work lookup failed")?;
    if rows.len() > 1 {
        return Err("WR is ambiguous; resolve duplicate activation records");
    }
    Ok(rows.into_iter().next())
}

pub async fn save_work(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &Work,
    expected_revision: Option<i64>,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = operations::begin_transaction(pool).await?;
    let (transaction, result) =
        save_work_transaction(transaction, actor, operation_id, input, expected_revision).await?;
    transaction
        .commit()
        .await
        .map_err(|_| "AAA work commit failed")?;
    Ok(result)
}

pub async fn save_work_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    input: &Work,
    expected_revision: Option<i64>,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    actor.require_write_authorization()?;
    delivery::validate(input)?;
    let payload = json!({"input":input,"expected_revision":expected_revision});
    let digest = operations::input_digest(&payload)?;
    let tool = if expected_revision.is_some() {
        "update_work"
    } else {
        "create_work"
    };
    if let Some(result) =
        operations::begin_operation(&mut transaction, actor, operation_id, tool, &digest).await?
    {
        return Ok((transaction, result));
    }
    if let Some(technician) = &input.technician_id {
        let tech: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM technicians WHERE technician_id=$1 AND status='ACTIVE')",
        )
        .bind(technician)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| "technician lookup failed")?;
        if !tech {
            return Err("assigned technician must exist and be ACTIVE");
        }
    }
    if matches!(input.status, crate::domain::delivery::WorkStatus::Completed) {
        return Err(
            "positive closure must use close_work_with_test_result with a recorded ONE result/code",
        );
    }
    if let Some(customer) = &input.customer_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM customers WHERE customer_id=$1 AND status='ACTIVE')",
        )
        .bind(customer)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| "customer lookup failed")?;
        if !valid {
            return Err("ACTIVE customer does not exist");
        }
    }
    if let Some(team) = &input.team_id {
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM delivery_teams t JOIN delivery_team_members m ON m.team_id=t.team_id WHERE t.team_id=$1 AND t.status='ACTIVE' AND m.technician_id=$2)")
            .bind(team).bind(&input.technician_id).fetch_one(&mut *transaction).await.map_err(|_|"team membership lookup failed")?;
        if !valid {
            return Err("technician is not a member of this ACTIVE team");
        }
    }
    let prior=sqlx::query("SELECT a.wr,a.status AS activation_status,to_jsonb(w) AS details,w.revision FROM activation_requests a LEFT JOIN delivery_work_details w ON w.activation_id=a.activation_id WHERE a.activation_id=$1 FOR UPDATE OF a")
        .bind(&input.activation_id).fetch_optional(&mut *transaction).await.map_err(|_|"work lock failed")?;
    let before = prior
        .as_ref()
        .map(|row| row.try_get::<Option<Value>, _>("details"))
        .transpose()
        .map_err(|_| "work prior state unavailable")?
        .flatten();
    let activation_status = if let Some(row) = &prior {
        if row
            .try_get::<String, _>("wr")
            .map_err(|_| "work identity unavailable")?
            != input.work_order_id
        {
            return Err("activation ID belongs to a different WR");
        }
        let actual = row
            .try_get::<Option<i64>, _>("revision")
            .map_err(|_| "work revision unavailable")?;
        if actual != expected_revision {
            return Err("work revision changed or details already exist");
        }
        row.try_get::<String, _>("activation_status")
            .map_err(|_| "activation status unavailable")?
    } else {
        if expected_revision.is_some() {
            return Err("work does not exist");
        }
        let duplicate: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activation_requests WHERE wr=$1)")
                .bind(&input.work_order_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| "WR uniqueness lookup failed")?;
        if duplicate {
            return Err("WR already exists; enrich its activation record instead");
        }
        sqlx::query("INSERT INTO activation_requests(activation_id,technician_id,wr,ont_serial,status) VALUES ($1,$2,$3,$4,'DRAFT')")
            .bind(&input.activation_id).bind(&input.technician_id).bind(&input.work_order_id).bind(&input.ont_serial).execute(&mut *transaction).await.map_err(|_|"activation work creation failed")?;
        "DRAFT".to_owned()
    };
    if expected_revision.is_some() && activation_status != "DRAFT" {
        return Err("active activation lifecycle cannot be overwritten by delivery editing");
    }
    let status = serde_json::to_value(&input.status)
        .map_err(|_| "work status invalid")?
        .as_str()
        .ok_or("work status invalid")?
        .to_owned();
    let address = serde_json::to_value(&input.address).map_err(|_| "work address invalid")?;
    let geolocation = serde_json::to_value(&input.geolocation).map_err(|_| "work GPS invalid")?;
    let appointment = input
        .appointment_at
        .as_deref()
        .map(chrono::DateTime::parse_from_rfc3339)
        .transpose()
        .map_err(|_| "invalid appointment")?
        .map(|date| date.to_rfc3339());
    let revision:i64=sqlx::query_scalar("INSERT INTO delivery_work_details(activation_id,work_order_id,operator_name,customer_id,customer_phone,address,geolocation,appointment_at,team_id,work_status,ko_reason,notes,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$7,$8::text::timestamptz,$9,$10,$11,$12,$13,$13) ON CONFLICT(activation_id) DO UPDATE SET operator_name=EXCLUDED.operator_name,customer_id=EXCLUDED.customer_id,customer_phone=EXCLUDED.customer_phone,address=EXCLUDED.address,geolocation=EXCLUDED.geolocation,appointment_at=EXCLUDED.appointment_at,team_id=EXCLUDED.team_id,work_status=EXCLUDED.work_status,ko_reason=EXCLUDED.ko_reason,notes=EXCLUDED.notes,updated_by=EXCLUDED.updated_by,updated_at=now(),revision=delivery_work_details.revision+1 RETURNING revision")
        .bind(&input.activation_id).bind(&input.work_order_id).bind(&input.operator_name).bind(&input.customer_id).bind(&input.customer_phone).bind(address).bind(geolocation).bind(appointment).bind(&input.team_id).bind(&status).bind(&input.ko_reason).bind(&input.notes).bind(actor.principal_id)
        .fetch_one(&mut *transaction).await.map_err(|_|"delivery work save failed")?;
    sqlx::query("UPDATE activation_requests SET technician_id=$2,ont_serial=$3,updated_at=now() WHERE activation_id=$1 AND status='DRAFT'")
        .bind(&input.activation_id).bind(&input.technician_id).bind(&input.ont_serial).execute(&mut *transaction).await.map_err(|_|"work technician assignment failed")?;
    let result = json!({"activation_id":input.activation_id,"work_order_id":input.work_order_id,"revision":revision,"status":status,"before":before});
    let sanitized = json!({"activation_id":input.activation_id,"work_order_id":input.work_order_id,"technician_id":input.technician_id,"status":status});
    operations::finish_operation(
        &mut transaction,
        actor,
        operation_id,
        tool,
        "WORK",
        &input.activation_id,
        &digest,
        sanitized,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((transaction, result))
}
