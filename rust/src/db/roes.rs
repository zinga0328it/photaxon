use serde::Serialize;
use sqlx::{PgPool, Row};

#[derive(Debug, Serialize)]
pub struct RoeRecord {
    pub roe_id: String,
    pub cabinet_id: String,
    pub splitter_id: String,
    pub address_reference: String,
    pub upstream_resource: String,
    pub status: String,
}

pub async fn list(pool: &PgPool) -> Result<Vec<RoeRecord>, sqlx::Error> {
    let rows = sqlx::query("SELECT roe_id,cabinet_id,splitter_id,address_reference,upstream_resource,status FROM roes ORDER BY roe_id")
        .fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(RoeRecord {
                roe_id: row.try_get("roe_id")?,
                cabinet_id: row.try_get("cabinet_id")?,
                splitter_id: row.try_get("splitter_id")?,
                address_reference: row.try_get("address_reference")?,
                upstream_resource: row.try_get("upstream_resource")?,
                status: row.try_get("status")?,
            })
        })
        .collect()
}

pub async fn get(pool: &PgPool, roe_id: &str) -> Result<Option<RoeRecord>, sqlx::Error> {
    let row = sqlx::query("SELECT roe_id,cabinet_id,splitter_id,address_reference,upstream_resource,status FROM roes WHERE roe_id=$1")
        .bind(roe_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(RoeRecord {
            roe_id: row.try_get("roe_id")?,
            cabinet_id: row.try_get("cabinet_id")?,
            splitter_id: row.try_get("splitter_id")?,
            address_reference: row.try_get("address_reference")?,
            upstream_resource: row.try_get("upstream_resource")?,
            status: row.try_get("status")?,
        })
    })
    .transpose()
}
