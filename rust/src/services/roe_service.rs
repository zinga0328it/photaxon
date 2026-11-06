use crate::{
    db::{audit, topology},
    domain::roe::CreateRoe,
    mcp::auth::InvocationContext,
    policy::topology_validator,
};
use serde_json::json;
use sqlx::PgPool;

pub async fn create(
    pool: &PgPool,
    actor: &InvocationContext,
    input: CreateRoe,
) -> Result<String, &'static str> {
    topology_validator::validate_roe(&input)?;
    if !topology::resource_exists(pool, "cabinets", "cabinet_id", &input.cabinet_id)
        .await
        .map_err(|_| "cabinet lookup failed")?
    {
        return Err("cabinet does not exist");
    }
    let splitter_matches = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM splitters WHERE splitter_id=$1 AND cabinet_id=$2)",
    )
    .bind(&input.splitter_id)
    .bind(&input.cabinet_id)
    .fetch_one(pool)
    .await
    .map_err(|_| "splitter lookup failed")?;
    if !splitter_matches {
        return Err("splitter does not belong to cabinet");
    }
    if topology::resource_exists(pool, "roes", "roe_id", &input.roe_id)
        .await
        .map_err(|_| "ROE lookup failed")?
    {
        return Err("ROE identifier already exists");
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    sqlx::query("INSERT INTO roes (roe_id,cabinet_id,splitter_id,address_reference,upstream_resource,notes,source_event,created_by_session) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(&input.roe_id).bind(&input.cabinet_id).bind(&input.splitter_id)
        .bind(&input.address_reference).bind(&input.upstream_resource).bind(&input.notes)
        .bind(&input.source_event).bind(&actor.technician_session)
        .execute(&mut *tx).await.map_err(|_| "ROE insert failed")?;
    let sanitized = json!({"roe_id":input.roe_id,"cabinet_id":input.cabinet_id,"splitter_id":input.splitter_id,"upstream_resource":input.upstream_resource});
    let state = json!({"status":"ACTIVE"});
    audit::record(
        &mut tx,
        actor,
        audit::AuditRecord {
            tool_name: "create_roe",
            tool_class: "WRITE_TOPOLOGY",
            source_event: Some(input.source_event.clone()),
            sanitized_input: sanitized,
            validation: json!({"status":"PASS"}),
            result: state.clone(),
            resource_id: input.roe_id.clone(),
            next_state: state,
            effect: audit::AuditEffect::Applied,
        },
    )
    .await
    .map_err(|_| "audit write failed")?;
    tx.commit()
        .await
        .map_err(|_| "ROE transaction commit failed")?;
    Ok(input.roe_id)
}
