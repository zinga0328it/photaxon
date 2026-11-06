use crate::mcp::auth::InvocationContext;
use serde_json::Value;
use sqlx::{Postgres, Transaction, types::Json};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub enum AuditEffect {
    Safe,
    Applied,
    Pending,
}

pub struct AuditRecord {
    pub tool_name: &'static str,
    pub tool_class: &'static str,
    pub source_event: Option<String>,
    pub sanitized_input: Value,
    pub validation: Value,
    pub result: Value,
    pub resource_id: String,
    pub next_state: Value,
    pub effect: AuditEffect,
}

pub async fn record(
    transaction: &mut Transaction<'_, Postgres>,
    actor: &InvocationContext,
    record: AuditRecord,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query("INSERT INTO mcp_tool_audit (invocation_id, technician_session, tool_name, tool_class, sanitized_input, validation, result, resource_id, previous_state, next_state) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,NULL,$9)")
        .bind(&actor.invocation_id).bind(&actor.technician_session).bind(record.tool_name)
        .bind(record.tool_class).bind(Json(record.sanitized_input))
        .bind(Json(record.validation.clone())).bind(Json(record.result.clone()))
        .bind(&record.resource_id).bind(Json(record.next_state.clone()))
        .execute(&mut **transaction).await?;
    let (Some(source_event), status) = (
        record.source_event.as_deref(),
        match record.effect {
            AuditEffect::Safe => return Ok(None),
            AuditEffect::Applied => "APPLIED",
            AuditEffect::Pending => "PENDING",
        },
    ) else {
        return Err(sqlx::Error::Protocol(
            "topology audit requires source_event".into(),
        ));
    };
    let change_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO topology_changes (change_id,invocation_id,technician_session,change_type,resource_id,source_event,previous_state,next_state,validation,status) VALUES ($1,$2,$3,$4,$5,$6,NULL,$7,$8,$9)")
        .bind(&change_id).bind(&actor.invocation_id).bind(&actor.technician_session)
        .bind(record.tool_name).bind(&record.resource_id).bind(source_event)
        .bind(Json(record.next_state.clone())).bind(Json(record.validation.clone()))
        .bind(status).execute(&mut **transaction).await?;
    if matches!(record.effect, AuditEffect::Pending) {
        sqlx::query("INSERT INTO llm_proposals (proposal_id,invocation_id,technician_session,tool_name,proposal,validator_result,status) VALUES ($1,$2,$3,$4,$5,$6,'PENDING')")
            .bind(&change_id).bind(&actor.invocation_id).bind(&actor.technician_session)
            .bind(record.tool_name).bind(Json(record.result)).bind(Json(record.validation))
            .execute(&mut **transaction).await?;
    }
    Ok(Some(change_id))
}
