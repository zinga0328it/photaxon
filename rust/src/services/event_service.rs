use crate::{
    db, domain::ont::ReportOntEvent, mcp::auth::InvocationContext, policy::safe_event_validator,
};
use sqlx::PgPool;

pub async fn report_ont(
    pool: &PgPool,
    actor: &InvocationContext,
    input: ReportOntEvent,
) -> Result<(), &'static str> {
    safe_event_validator::validate_ont_event(&input)?;
    db::events::report_ont(pool, actor, &input).await
}
