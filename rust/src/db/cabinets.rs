use crate::cabinet::model::Cabinet;
use sqlx::{PgPool, Row};

pub async fn list(pool: &PgPool) -> Result<Vec<Cabinet>, sqlx::Error> {
    let rows = sqlx::query("SELECT cabinet_id, status FROM cabinets ORDER BY cabinet_id")
        .fetch_all(pool)
        .await?;
    rows.into_iter()
        .map(|row| {
            Ok(Cabinet {
                cabinet_id: row.try_get("cabinet_id")?,
                status: row.try_get("status")?,
            })
        })
        .collect()
}

pub async fn get(pool: &PgPool, cabinet_id: &str) -> Result<Option<Cabinet>, sqlx::Error> {
    let row = sqlx::query("SELECT cabinet_id, status FROM cabinets WHERE cabinet_id = $1")
        .bind(cabinet_id)
        .fetch_optional(pool)
        .await?;
    row.map(|row| {
        Ok(Cabinet {
            cabinet_id: row.try_get("cabinet_id")?,
            status: row.try_get("status")?,
        })
    })
    .transpose()
}
