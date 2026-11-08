use sqlx::PgPool;

const MCP_MIGRATION: &str = include_str!("../../../db/migrations/006_mcp_topology_audit.sql");
const FIELD_OPERATIONS_MIGRATION: &str =
    include_str!("../../../db/migrations/007_telegram_field_operations.sql");
const ONT_COMPATIBILITY_MIGRATION: &str =
    include_str!("../../../db/migrations/008_telegram_ont_compatibility.sql");

pub async fn apply_mcp_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::raw_sql(MCP_MIGRATION)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await
}

pub async fn apply_field_operations_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::raw_sql(FIELD_OPERATIONS_MIGRATION)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await
}

pub async fn apply_ont_compatibility_schema(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::raw_sql(ONT_COMPATIBILITY_MIGRATION)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await
}

#[cfg(test)]
mod tests {
    use super::{FIELD_OPERATIONS_MIGRATION, MCP_MIGRATION};

    #[test]
    fn mcp_migration_is_additive_and_idempotent() {
        assert!(MCP_MIGRATION.contains("ADD COLUMN IF NOT EXISTS"));
        assert!(MCP_MIGRATION.contains("CREATE TABLE IF NOT EXISTS"));
        assert!(MCP_MIGRATION.contains("ON CONFLICT (version) DO NOTHING"));
        assert!(!MCP_MIGRATION.to_ascii_lowercase().contains("drop table"));
    }

    #[test]
    fn field_operations_migration_is_additive_and_not_applied_at_api_boot() {
        assert!(FIELD_OPERATIONS_MIGRATION.contains("CREATE TABLE IF NOT EXISTS feeder_cables"));
        assert!(FIELD_OPERATIONS_MIGRATION.contains("CREATE TABLE IF NOT EXISTS onts"));
        assert!(FIELD_OPERATIONS_MIGRATION.contains("CREATE TABLE IF NOT EXISTS modems"));
        assert!(FIELD_OPERATIONS_MIGRATION.contains("CREATE TABLE IF NOT EXISTS ont_modem_links"));
        assert!(
            FIELD_OPERATIONS_MIGRATION.contains("CREATE TABLE IF NOT EXISTS telegram_workflows")
        );
        assert!(
            FIELD_OPERATIONS_MIGRATION.contains("CREATE TABLE IF NOT EXISTS field_operation_audit")
        );
        assert!(FIELD_OPERATIONS_MIGRATION.contains("ON CONFLICT (version) DO NOTHING"));
        assert!(
            !FIELD_OPERATIONS_MIGRATION
                .to_ascii_lowercase()
                .contains("drop table")
        );
        assert!(!include_str!("bootstrap.rs").contains("apply_field_operations_schema"));
    }
}
