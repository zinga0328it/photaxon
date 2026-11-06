use crate::{domain::anomaly::ReportAnomaly, mcp::auth::InvocationContext};
use serde_json::json;
use sqlx::PgPool;

pub async fn report(
    pool: &PgPool,
    actor: &InvocationContext,
    input: &ReportAnomaly,
) -> Result<(), &'static str> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    let exists =
        sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM anomalies WHERE anomaly_id=$1)")
            .bind(&input.anomaly_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "anomaly lookup failed")?;
    if exists {
        return Err("anomaly identifier already exists");
    }
    sqlx::query("INSERT INTO anomalies (anomaly_id,correlation_id,severity,code,description,status) VALUES ($1,$2,$3,$4,$5,'OPEN')")
        .bind(&input.anomaly_id).bind(&input.correlation_id).bind(&input.severity)
        .bind(&input.code).bind(&input.description).execute(&mut *tx).await.map_err(|_| "anomaly insert failed")?;
    let sanitized = json!({"anomaly_id":input.anomaly_id,"code":input.code,"severity":input.severity,"correlation_id":input.correlation_id});
    crate::db::audit::record(
        &mut tx,
        actor,
        crate::db::audit::AuditRecord {
            tool_name: "report_anomaly",
            tool_class: "WRITE_SAFE",
            source_event: None,
            sanitized_input: sanitized,
            validation: json!({"status":"PASS"}),
            result: json!({"status":"OPEN"}),
            resource_id: input.anomaly_id.clone(),
            next_state: json!({"status":"OPEN"}),
            effect: crate::db::audit::AuditEffect::Safe,
        },
    )
    .await
    .map_err(|_| "audit write failed")?;
    tx.commit()
        .await
        .map_err(|_| "anomaly transaction commit failed")
}
