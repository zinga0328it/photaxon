use crate::{
    db, domain::anomaly::ReportAnomaly, mcp::auth::InvocationContext, policy::safe_event_validator,
};
use sqlx::PgPool;

pub async fn report(
    pool: &PgPool,
    actor: &InvocationContext,
    input: ReportAnomaly,
) -> Result<(), &'static str> {
    safe_event_validator::validate_anomaly(&input)?;
    db::anomalies::report(pool, actor, &input).await
}
