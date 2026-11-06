use sqlx::PgPool;

pub async fn initialize(pool: &PgPool) -> Result<(), sqlx::Error> {
    super::migration::apply_mcp_schema(pool).await?;
    super::seed::ensure_lab_topology(pool).await
}
