use crate::{domain::ont::ReportOntEvent, mcp::auth::InvocationContext};
use serde_json::json;
use sqlx::PgPool;

pub async fn report_ont(
    pool: &PgPool,
    actor: &InvocationContext,
    input: &ReportOntEvent,
) -> Result<(), &'static str> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    let agent_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM agents WHERE agent_id=$1 AND status='ACTIVE')",
    )
    .bind(&input.agent_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| "agent lookup failed")?;
    if !agent_exists {
        return Err("active cabinet agent does not exist");
    }
    let payload = json!({"ont_serial":input.ont_serial,"port_id":input.port_id,"technician_session":actor.technician_session});
    let row = sqlx::query("INSERT INTO agent_events (agent_id,correlation_id,event_type,payload) VALUES ($1,$2,$3,$4) RETURNING event_id")
        .bind(&input.agent_id).bind(&actor.invocation_id).bind(&input.event_type).bind(sqlx::types::Json(payload.clone()))
        .fetch_one(&mut *tx).await.map_err(|_| "agent event insert failed")?;
    let event_id: i64 =
        sqlx::Row::try_get(&row, "event_id").map_err(|_| "agent event id unavailable")?;
    let sanitized = json!({"agent_id":input.agent_id,"event_type":input.event_type,"ont_serial":input.ont_serial,"port_id":input.port_id});
    crate::db::audit::record(
        &mut tx,
        actor,
        crate::db::audit::AuditRecord {
            tool_name: "report_ont_event",
            tool_class: "WRITE_SAFE",
            source_event: None,
            sanitized_input: sanitized,
            validation: json!({"status":"PASS"}),
            result: json!({"event_id":event_id}),
            resource_id: input.agent_id.clone(),
            next_state: json!({"last_event_id":event_id}),
            effect: crate::db::audit::AuditEffect::Safe,
        },
    )
    .await
    .map_err(|_| "audit write failed")?;
    tx.commit()
        .await
        .map_err(|_| "event transaction commit failed")
}
