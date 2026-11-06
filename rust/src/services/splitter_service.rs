use crate::{
    db::audit, domain::splitter::CreateSplitter, mcp::auth::InvocationContext,
    policy::topology_validator,
};
use serde_json::json;
use sqlx::PgPool;

pub async fn create(
    pool: &PgPool,
    actor: &InvocationContext,
    input: CreateSplitter,
) -> Result<String, &'static str> {
    topology_validator::validate_splitter(&input)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    let line_matches = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ftth_lines WHERE line_id=$1 AND cabinet_id=$2)",
    )
    .bind(&input.line_id)
    .bind(&input.cabinet_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| "line lookup failed")?;
    if !line_matches {
        return Err("line does not belong to cabinet");
    }
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM splitters WHERE splitter_id=$1)",
    )
    .bind(&input.splitter_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| "splitter lookup failed")?;
    if exists {
        return Err("splitter identifier already exists");
    }

    sqlx::query("INSERT INTO splitters (splitter_id,cabinet_id,line_id,ratio,capacity_total,secondary_splitter_allowed,status) VALUES ($1,$2,$3,$4,$5,FALSE,'ACTIVE')")
        .bind(&input.splitter_id).bind(&input.cabinet_id).bind(&input.line_id)
        .bind(input.ratio).bind(input.capacity_total).execute(&mut *tx).await
        .map_err(|_| "splitter insert failed")?;
    for position in 1..=input.capacity_total {
        let port_id = format!("{}-PORT-{position:02}", input.splitter_id);
        sqlx::query("INSERT INTO ports (port_id,splitter_id,position,resource_state) VALUES ($1,$2,$3,'FREE')")
            .bind(&port_id).bind(&input.splitter_id).bind(position)
            .execute(&mut *tx).await.map_err(|_| "splitter port insert failed")?;
        sqlx::query("INSERT INTO resource_states (resource_id,resource_kind,resource_state,metadata) VALUES ($1,'PORT','FREE',$2)")
            .bind(&port_id).bind(json!({"cabinet_id": input.cabinet_id, "splitter_id": input.splitter_id}))
            .execute(&mut *tx).await.map_err(|_| "port state insert failed")?;
    }
    let sanitized = json!({"splitter_id":input.splitter_id,"cabinet_id":input.cabinet_id,"line_id":input.line_id,"capacity_total":input.capacity_total});
    let state = json!({"status":"ACTIVE","capacity_total":input.capacity_total});
    audit::record(
        &mut tx,
        actor,
        audit::AuditRecord {
            tool_name: "create_splitter",
            tool_class: "WRITE_TOPOLOGY",
            source_event: Some(input.source_event.clone()),
            sanitized_input: sanitized,
            validation: json!({"status":"PASS"}),
            result: state.clone(),
            resource_id: input.splitter_id.clone(),
            next_state: state,
            effect: audit::AuditEffect::Applied,
        },
    )
    .await
    .map_err(|_| "audit write failed")?;
    tx.commit()
        .await
        .map_err(|_| "splitter transaction commit failed")?;
    Ok(input.splitter_id)
}
