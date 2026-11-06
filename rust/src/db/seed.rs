use sqlx::{PgPool, Row};

const CABINETS: [(&str, &str, &str, &str); 3] = [
    ("CAB-A", "FTTH-LINE-A", "FS-LINE-A", "SPL-A-01"),
    ("CAB-B", "FTTH-LINE-B", "FS-LINE-B", "SPL-B-01"),
    ("CAB-C", "FTTH-LINE-C", "FS-LINE-C", "SPL-C-01"),
];
const INSERT_CABINET: &str = "INSERT INTO cabinets (cabinet_id, status) VALUES ($1, 'ACTIVE') ON CONFLICT (cabinet_id) DO NOTHING";
const INSERT_LINE: &str = "INSERT INTO ftth_lines (line_id, cabinet_id, openstack_network_name, status) VALUES ($1, $2, $3, 'ACTIVE') ON CONFLICT (line_id) DO NOTHING";
const INSERT_SPLITTER: &str = "INSERT INTO splitters (splitter_id, cabinet_id, line_id, ratio, capacity_total, secondary_splitter_allowed, status) VALUES ($1, $2, $3, 8, 8, TRUE, 'ACTIVE') ON CONFLICT (splitter_id) DO NOTHING";
const INSERT_AGENT: &str = "INSERT INTO agents (agent_id, cabinet_id, role, status) VALUES ($1, $2, 'CABINET_AGENT', 'ACTIVE') ON CONFLICT (agent_id) DO NOTHING";

pub async fn ensure_lab_topology(pool: &PgPool) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    for (cabinet_id, line_id, network_name, splitter_id) in CABINETS {
        sqlx::query(INSERT_CABINET)
            .bind(cabinet_id)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(INSERT_LINE)
            .bind(line_id)
            .bind(cabinet_id)
            .bind(network_name)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(INSERT_SPLITTER)
            .bind(splitter_id)
            .bind(cabinet_id)
            .bind(line_id)
            .execute(&mut *transaction)
            .await?;
        let agent_id = format!("AGENT-{cabinet_id}");
        sqlx::query(INSERT_AGENT)
            .bind(agent_id)
            .bind(cabinet_id)
            .execute(&mut *transaction)
            .await?;
    }
    verify_seed(&mut transaction).await?;
    transaction.commit().await
}

async fn verify_seed(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), sqlx::Error> {
    for (cabinet_id, line_id, network_name, splitter_id) in CABINETS {
        let row = sqlx::query("SELECT c.status AS cabinet_status, l.line_id, l.openstack_network_name, s.splitter_id, a.status AS agent_status FROM cabinets c JOIN ftth_lines l ON l.cabinet_id = c.cabinet_id JOIN splitters s ON s.cabinet_id = c.cabinet_id AND s.line_id = l.line_id JOIN agents a ON a.cabinet_id = c.cabinet_id WHERE c.cabinet_id = $1 AND a.agent_id = $2")
            .bind(cabinet_id)
            .bind(format!("AGENT-{cabinet_id}"))
            .fetch_optional(&mut **transaction)
            .await?;
        let matches = row.is_some_and(|row| {
            row.get::<String, _>("cabinet_status") == "ACTIVE"
                && row.get::<String, _>("line_id") == line_id
                && row.get::<String, _>("openstack_network_name") == network_name
                && row.get::<String, _>("splitter_id") == splitter_id
                && row.get::<String, _>("agent_status") == "ACTIVE"
        });
        if !matches {
            return Err(sqlx::Error::Protocol(format!(
                "FiberSpider seed topology mismatch for {cabinet_id}"
            )));
        }
    }
    Ok(())
}

pub fn seed_contract_is_idempotent() -> bool {
    [INSERT_CABINET, INSERT_LINE, INSERT_SPLITTER, INSERT_AGENT]
        .iter()
        .all(|statement| statement.contains("ON CONFLICT") && statement.contains("DO NOTHING"))
}

#[cfg(test)]
mod tests {
    use super::{CABINETS, seed_contract_is_idempotent};
    use std::collections::HashSet;

    #[test]
    fn seed_has_three_unique_initial_cabinets() {
        let ids: HashSet<_> = CABINETS.iter().map(|seed| seed.0).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains("CAB-A"));
        assert!(ids.contains("CAB-B"));
        assert!(ids.contains("CAB-C"));
    }

    #[test]
    fn checked_in_seed_migration_is_idempotent() {
        assert!(seed_contract_is_idempotent());
    }
}
