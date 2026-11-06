use sqlx::PgPool;

const MCP_MIGRATION: &str = include_str!("../../../db/migrations/006_mcp_topology_audit.sql");

pub async fn apply_mcp_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::raw_sql(MCP_MIGRATION)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await
}

#[cfg(test)]
mod tests {
    use super::MCP_MIGRATION;

    #[test]
    fn mcp_migration_is_additive_and_idempotent() {
        assert!(MCP_MIGRATION.contains("ADD COLUMN IF NOT EXISTS"));
        assert!(MCP_MIGRATION.contains("CREATE TABLE IF NOT EXISTS"));
        assert!(MCP_MIGRATION.contains("ON CONFLICT (version) DO NOTHING"));
        assert!(!MCP_MIGRATION.to_ascii_lowercase().contains("drop table"));
    }
}
