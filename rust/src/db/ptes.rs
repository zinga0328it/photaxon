use serde::Serialize;
use sqlx::{PgPool, Row};

#[derive(Debug, Serialize)]
pub struct PteRecord {
    pub pte_id: String,
    pub cabinet_id: Option<String>,
    pub line_id: Option<String>,
    pub address_reference: Option<String>,
    pub status: String,
}

pub async fn list(pool: &PgPool) -> Result<Vec<PteRecord>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT pte_id,cabinet_id,line_id,address_reference,status FROM ptes ORDER BY pte_id",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(PteRecord {
                pte_id: row.try_get("pte_id")?,
                cabinet_id: row.try_get("cabinet_id")?,
                line_id: row.try_get("line_id")?,
                address_reference: row.try_get("address_reference")?,
                status: row.try_get("status")?,
            })
        })
        .collect()
}

pub async fn get(pool: &PgPool, pte_id: &str) -> Result<Option<PteRecord>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT pte_id,cabinet_id,line_id,address_reference,status FROM ptes WHERE pte_id=$1",
    )
    .bind(pte_id)
    .fetch_optional(pool)
    .await?;
    row.map(|row| {
        Ok(PteRecord {
            pte_id: row.try_get("pte_id")?,
            cabinet_id: row.try_get("cabinet_id")?,
            line_id: row.try_get("line_id")?,
            address_reference: row.try_get("address_reference")?,
            status: row.try_get("status")?,
        })
    })
    .transpose()
}
