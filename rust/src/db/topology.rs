use sqlx::{PgPool, Row};

pub async fn get_topology(pool: &PgPool) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows = sqlx::query("SELECT c.cabinet_id,c.status AS cabinet_status,l.line_id,l.status AS line_status,s.splitter_id,s.status AS splitter_status,a.agent_id,a.status AS agent_status FROM cabinets c JOIN ftth_lines l ON l.cabinet_id=c.cabinet_id JOIN splitters s ON s.cabinet_id=c.cabinet_id AND s.line_id=l.line_id JOIN agents a ON a.cabinet_id=c.cabinet_id ORDER BY c.cabinet_id")
        .fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(serde_json::json!({
                "cabinet_id": row.try_get::<String,_>("cabinet_id")?,
                "cabinet_status": row.try_get::<String,_>("cabinet_status")?,
                "line_id": row.try_get::<String,_>("line_id")?,
                "line_status": row.try_get::<String,_>("line_status")?,
                "splitter_id": row.try_get::<String,_>("splitter_id")?,
                "splitter_status": row.try_get::<String,_>("splitter_status")?,
                "agent_id": row.try_get::<String,_>("agent_id")?,
                "agent_status": row.try_get::<String,_>("agent_status")?,
            }))
        })
        .collect()
}

pub async fn pending_changes(pool: &PgPool) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let rows = sqlx::query("SELECT change_id,change_type,resource_id,source_event,validation,status,created_at::text AS created_at FROM topology_changes WHERE status='PENDING' ORDER BY created_at")
        .fetch_all(pool).await?;
    rows.into_iter()
        .map(|row| {
            Ok(serde_json::json!({
                "change_id": row.try_get::<String,_>("change_id")?,
                "change_type": row.try_get::<String,_>("change_type")?,
                "resource_id": row.try_get::<String,_>("resource_id")?,
                "source_event": row.try_get::<String,_>("source_event")?,
                "validation": row.try_get::<serde_json::Value,_>("validation")?,
                "status": row.try_get::<String,_>("status")?,
                "created_at": row.try_get::<String,_>("created_at")?,
            }))
        })
        .collect()
}

pub async fn resource_exists(
    pool: &PgPool,
    table: &'static str,
    id_column: &'static str,
    id: &str,
) -> Result<bool, sqlx::Error> {
    let query = match (table, id_column) {
        ("cabinets", "cabinet_id") => "SELECT EXISTS(SELECT 1 FROM cabinets WHERE cabinet_id=$1)",
        ("ftth_lines", "line_id") => "SELECT EXISTS(SELECT 1 FROM ftth_lines WHERE line_id=$1)",
        ("splitters", "splitter_id") => {
            "SELECT EXISTS(SELECT 1 FROM splitters WHERE splitter_id=$1)"
        }
        ("roes", "roe_id") => "SELECT EXISTS(SELECT 1 FROM roes WHERE roe_id=$1)",
        ("ptes", "pte_id") => "SELECT EXISTS(SELECT 1 FROM ptes WHERE pte_id=$1)",
        _ => return Ok(false),
    };
    sqlx::query(query)
        .bind(id)
        .fetch_one(pool)
        .await?
        .try_get(0)
}
