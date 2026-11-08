use crate::telegram::workflow::{WorkflowDraft, WorkflowStatus};
use sqlx::{PgPool, Row};
use std::error::Error;
use uuid::Uuid;

#[derive(Debug)]
pub enum WorkflowStoreError {
    Database,
    Serialization,
    Conflict,
}

impl std::fmt::Display for WorkflowStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Database => "workflow store unavailable",
            Self::Serialization => "workflow state is invalid",
            Self::Conflict => "workflow changed or is no longer active",
        })
    }
}

impl Error for WorkflowStoreError {}

pub async fn create(
    pool: &PgPool,
    session_id: &str,
    technician_id: &str,
    kind: crate::telegram::workflow::WorkflowKind,
) -> Result<WorkflowDraft, WorkflowStoreError> {
    if kind == crate::telegram::workflow::WorkflowKind::UnlinkModem {
        return Err(WorkflowStoreError::Conflict);
    }
    let draft = WorkflowDraft::new(Uuid::new_v4().to_string(), kind);
    let payload = serde_json::to_value(&draft).map_err(|_| WorkflowStoreError::Serialization)?;
    let status = database_status(draft.status);
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    sqlx::query("UPDATE telegram_workflows SET status='CANCELLED',cancelled_at=now(),updated_at=now() WHERE session_id=$1 AND status IN ('DRAFT','REVIEW')")
        .bind(session_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    let inserted = sqlx::query("INSERT INTO telegram_workflows (workflow_id,session_id,technician_id,workflow_type,state,status,draft_payload,version,expires_at,work_order_id) SELECT $1,$2,$3,$4,$5,$6,$7,$8,now() + interval '30 minutes',$9 WHERE EXISTS (SELECT 1 FROM field_technician_sessions WHERE session_id=$2 AND technician_id=$3 AND status='ACTIVE' AND expires_at>now())")
        .bind(&draft.workflow_id)
        .bind(session_id)
        .bind(technician_id)
        .bind(database_kind(draft.kind))
        .bind(format!("{:?}", draft.step))
        .bind(status)
        .bind(payload)
        .bind(draft.version)
        .bind(draft.fields.get("work_order_id").and_then(serde_json::Value::as_str))
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    if inserted.rows_affected() != 1 {
        return Err(WorkflowStoreError::Conflict);
    }
    transaction
        .commit()
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    Ok(draft)
}

pub async fn load_open(
    pool: &PgPool,
    session_id: &str,
) -> Result<Option<WorkflowDraft>, WorkflowStoreError> {
    sqlx::query("UPDATE telegram_workflows SET status='EXPIRED',updated_at=now() WHERE session_id=$1 AND status IN ('DRAFT','REVIEW') AND expires_at<=now()")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    let row = sqlx::query("SELECT draft_payload FROM telegram_workflows WHERE session_id=$1 AND status IN ('DRAFT','REVIEW') AND expires_at>now() ORDER BY created_at DESC LIMIT 1")
        .bind(session_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    row.map(|row| {
        let payload: serde_json::Value = row
            .try_get("draft_payload")
            .map_err(|_| WorkflowStoreError::Database)?;
        serde_json::from_value(payload).map_err(|_| WorkflowStoreError::Serialization)
    })
    .transpose()
}

pub async fn save(
    pool: &PgPool,
    session_id: &str,
    draft: &WorkflowDraft,
    expected_version: i32,
) -> Result<(), WorkflowStoreError> {
    if !matches!(draft.status, WorkflowStatus::Draft | WorkflowStatus::Review)
        || draft.version != expected_version + 1
    {
        return Err(WorkflowStoreError::Conflict);
    }
    if let Some(work_order_id) = draft
        .fields
        .get("work_order_id")
        .and_then(serde_json::Value::as_str)
    {
        let valid_work_order = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM activation_requests a JOIN field_technician_sessions s ON s.technician_id=a.technician_id JOIN technicians t ON t.technician_id=s.technician_id WHERE s.session_id=$1 AND s.status='ACTIVE' AND s.expires_at>now() AND t.status='ACTIVE' AND a.wr=$2)")
            .bind(session_id)
            .bind(work_order_id)
            .fetch_one(pool)
            .await
            .map_err(|_| WorkflowStoreError::Database)?;
        if !valid_work_order {
            return Err(WorkflowStoreError::Conflict);
        }
    }
    let payload = serde_json::to_value(draft).map_err(|_| WorkflowStoreError::Serialization)?;
    let updated = sqlx::query("UPDATE telegram_workflows SET state=$4,status=$5,draft_payload=$6,version=$7,updated_at=now(),work_order_id=$8 WHERE workflow_id=$1 AND session_id=$2 AND version=$3 AND status IN ('DRAFT','REVIEW') AND expires_at>now() AND EXISTS (SELECT 1 FROM field_technician_sessions WHERE session_id=$2 AND status='ACTIVE' AND expires_at>now())")
        .bind(&draft.workflow_id)
        .bind(session_id)
        .bind(expected_version)
        .bind(format!("{:?}", draft.step))
        .bind(database_status(draft.status))
        .bind(payload)
        .bind(draft.version)
        .bind(
            draft
                .fields
                .get("work_order_id")
                .and_then(serde_json::Value::as_str),
        )
        .execute(pool)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    if updated.rows_affected() == 1 {
        Ok(())
    } else {
        Err(WorkflowStoreError::Conflict)
    }
}

pub async fn cancel_open(pool: &PgPool, session_id: &str) -> Result<bool, WorkflowStoreError> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    let workflow_ids = sqlx::query_scalar::<_, String>("UPDATE telegram_workflows SET status='CANCELLED',cancelled_at=now(),updated_at=now(),version=version+1 WHERE session_id=$1 AND status IN ('DRAFT','REVIEW') AND expires_at>now() AND EXISTS (SELECT 1 FROM field_technician_sessions WHERE session_id=$1 AND status='ACTIVE' AND expires_at>now()) RETURNING workflow_id")
        .bind(session_id)
        .fetch_all(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    if !workflow_ids.is_empty() {
        sqlx::query("UPDATE roe_selection_proposals SET status='REJECTED' WHERE workflow_id=ANY($1) AND status='PENDING'")
            .bind(&workflow_ids)
            .execute(&mut *transaction)
            .await
            .map_err(|_| WorkflowStoreError::Database)?;
    }
    transaction
        .commit()
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    Ok(!workflow_ids.is_empty())
}

pub async fn close_intervention(
    pool: &PgPool,
    session_id: &str,
) -> Result<bool, WorkflowStoreError> {
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    let session = sqlx::query("SELECT technician_id,status,(expires_at<=now()) AS expired FROM field_technician_sessions WHERE session_id=$1 FOR UPDATE")
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database)?
        .ok_or(WorkflowStoreError::Conflict)?;
    let technician_id: String = session
        .try_get("technician_id")
        .map_err(|_| WorkflowStoreError::Database)?;
    let status: String = session
        .try_get("status")
        .map_err(|_| WorkflowStoreError::Database)?;
    let expired: bool = session
        .try_get("expired")
        .map_err(|_| WorkflowStoreError::Database)?;
    let idempotency_key = format!("CLOSE_INTERVENTION:{session_id}");
    let existing_audit = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM field_operation_audit WHERE idempotency_key=$1 AND operation_type='CLOSE_INTERVENTION')",
    )
    .bind(&idempotency_key)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| WorkflowStoreError::Database)?;
    if existing_audit {
        transaction
            .commit()
            .await
            .map_err(|_| WorkflowStoreError::Database)?;
        return Ok(false);
    }
    if status != "ACTIVE" || expired {
        return Err(WorkflowStoreError::Conflict);
    }
    let has_open_workflow = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM telegram_workflows WHERE session_id=$1 AND status IN ('DRAFT','REVIEW') AND expires_at>now())",
    )
    .bind(session_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| WorkflowStoreError::Database)?;
    if has_open_workflow {
        return Err(WorkflowStoreError::Conflict);
    }
    sqlx::query("UPDATE field_technician_sessions SET status='REVOKED',revoked_at=now() WHERE session_id=$1 AND status='ACTIVE'")
        .bind(session_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    sqlx::query("INSERT INTO field_operation_audit (event_id,correlation_id,idempotency_key,actor_type,actor_id,operation_type,resource_type,resource_id,sanitized_input,before_state,after_state,validator_result,agent_decision,result_code) VALUES ($1,$2,$3,'TECHNICIAN',$4,'CLOSE_INTERVENTION','TECHNICIAN_SESSION',$5,'{}'::jsonb,$6,$7,$8,NULL,'CLOSED')")
        .bind(Uuid::new_v4().to_string())
        .bind(&idempotency_key)
        .bind(idempotency_key.clone())
        .bind(technician_id)
        .bind(session_id)
        .bind(serde_json::json!({"status":"ACTIVE"}))
        .bind(serde_json::json!({"status":"REVOKED"}))
        .bind(serde_json::json!({"status":"PASS","open_workflows":0}))
        .execute(&mut *transaction)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    transaction
        .commit()
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    Ok(true)
}

pub async fn claim_telegram_update(
    pool: &PgPool,
    session_id: &str,
    telegram_chat_id: &str,
    update_id: i64,
) -> Result<bool, WorkflowStoreError> {
    let inserted = sqlx::query("INSERT INTO telegram_update_receipts (update_id,telegram_chat_id,session_id,result_code) SELECT $1,$2,$3,'RECEIVED' WHERE EXISTS (SELECT 1 FROM field_technician_sessions WHERE session_id=$3 AND status='ACTIVE' AND expires_at>now()) ON CONFLICT (telegram_chat_id,update_id) DO NOTHING")
        .bind(update_id)
        .bind(telegram_chat_id)
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(|_| WorkflowStoreError::Database)?;
    Ok(inserted.rows_affected() == 1)
}

fn database_kind(kind: crate::telegram::workflow::WorkflowKind) -> &'static str {
    use crate::telegram::workflow::WorkflowKind;
    match kind {
        WorkflowKind::CreateRoe => "CREATE_ROE",
        WorkflowKind::RegisterOnt => "REGISTER_ONT",
        WorkflowKind::RegisterModem => "REGISTER_MODEM",
        WorkflowKind::ReplaceModem => "REPLACE_MODEM",
        WorkflowKind::UnlinkModem => "UNLINK_MODEM",
        WorkflowKind::SearchAsset => "SEARCH_ASSET",
        WorkflowKind::NearbyRoe => "NEARBY_ROE",
    }
}

fn database_status(status: WorkflowStatus) -> &'static str {
    match status {
        WorkflowStatus::Draft => "DRAFT",
        WorkflowStatus::Review => "REVIEW",
        WorkflowStatus::Confirmed => "CONFIRMED",
        WorkflowStatus::Cancelled => "CANCELLED",
        WorkflowStatus::Expired => "EXPIRED",
        WorkflowStatus::Failed => "FAILED",
    }
}

#[cfg(test)]
mod tests {
    use super::{WorkflowStoreError, create};
    use crate::telegram::workflow::WorkflowKind;

    #[tokio::test]
    async fn technician_unlink_is_denied_before_any_database_connection() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgresql://localhost/fiberspider_unused")
            .unwrap();
        let result = create(&pool, "session", "technician", WorkflowKind::UnlinkModem).await;
        assert!(matches!(result, Err(WorkflowStoreError::Conflict)));
    }
}
