use crate::{
    db::{audit, topology},
    domain::pte::CreatePte,
    mcp::auth::InvocationContext,
    policy::topology_validator,
};
use serde_json::json;
use sqlx::PgPool;

pub async fn create(
    pool: &PgPool,
    actor: &InvocationContext,
    input: CreatePte,
) -> Result<String, &'static str> {
    topology_validator::validate_pte(&input)?;
    let line_matches = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ftth_lines WHERE line_id=$1 AND cabinet_id=$2)",
    )
    .bind(&input.line_id)
    .bind(&input.cabinet_id)
    .fetch_one(pool)
    .await
    .map_err(|_| "line lookup failed")?;
    if !line_matches {
        return Err("line does not belong to cabinet");
    }
    if topology::resource_exists(pool, "ptes", "pte_id", &input.pte_id)
        .await
        .map_err(|_| "PTE lookup failed")?
    {
        return Err("PTE identifier already exists");
    }

    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    sqlx::query("INSERT INTO ptes (pte_id,cabinet_id,line_id,address_reference,notes,status) VALUES ($1,$2,$3,$4,$5,'ACTIVE')")
        .bind(&input.pte_id).bind(&input.cabinet_id).bind(&input.line_id)
        .bind(&input.address_reference).bind(&input.notes)
        .execute(&mut *tx).await.map_err(|_| "PTE insert failed")?;
    let sanitized =
        json!({"pte_id":input.pte_id,"cabinet_id":input.cabinet_id,"line_id":input.line_id});
    let state = json!({"status":"ACTIVE"});
    audit::record(
        &mut tx,
        actor,
        audit::AuditRecord {
            tool_name: "create_pte",
            tool_class: "WRITE_TOPOLOGY",
            source_event: Some(input.source_event.clone()),
            sanitized_input: sanitized,
            validation: json!({"status":"PASS"}),
            result: state.clone(),
            resource_id: input.pte_id.clone(),
            next_state: state,
            effect: audit::AuditEffect::Applied,
        },
    )
    .await
    .map_err(|_| "audit write failed")?;
    tx.commit()
        .await
        .map_err(|_| "PTE transaction commit failed")?;
    Ok(input.pte_id)
}
