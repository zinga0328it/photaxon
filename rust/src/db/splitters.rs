use crate::domain::splitter::SplitterCapacity;
use sqlx::{PgPool, Row};

pub async fn list(pool: &PgPool) -> Result<Vec<SplitterCapacity>, sqlx::Error> {
    let rows = sqlx::query("SELECT s.splitter_id,s.cabinet_id,s.line_id,s.ratio,s.capacity_total,s.status,count(p.port_id) FILTER (WHERE p.resource_state='FREE') AS free_ports FROM splitters s LEFT JOIN ports p ON p.splitter_id=s.splitter_id GROUP BY s.splitter_id ORDER BY s.splitter_id")
        .fetch_all(pool).await?;
    rows.into_iter().map(decode).collect()
}

pub async fn get(
    pool: &PgPool,
    splitter_id: &str,
) -> Result<Option<SplitterCapacity>, sqlx::Error> {
    let row = sqlx::query("SELECT s.splitter_id,s.cabinet_id,s.line_id,s.ratio,s.capacity_total,s.status,count(p.port_id) FILTER (WHERE p.resource_state='FREE') AS free_ports FROM splitters s LEFT JOIN ports p ON p.splitter_id=s.splitter_id WHERE s.splitter_id=$1 GROUP BY s.splitter_id")
        .bind(splitter_id).fetch_optional(pool).await?;
    row.map(decode).transpose()
}

fn decode(row: sqlx::postgres::PgRow) -> Result<SplitterCapacity, sqlx::Error> {
    Ok(SplitterCapacity {
        splitter_id: row.try_get("splitter_id")?,
        cabinet_id: row.try_get("cabinet_id")?,
        line_id: row.try_get("line_id")?,
        ratio: row.try_get("ratio")?,
        capacity_total: row.try_get("capacity_total")?,
        free_ports: row.try_get("free_ports")?,
        status: row.try_get("status")?,
    })
}
