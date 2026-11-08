use crate::domain::field_ops::{NearbyRoeCandidate, available_feeder_fibers, distance_meters};
use sqlx::{Executor, PgPool, Postgres, Row};

pub async fn cabinet_status(pool: &PgPool) -> Result<String, sqlx::Error> {
    let rows = sqlx::query("SELECT c.cabinet_id,c.status AS cabinet_status,a.agent_id,a.status AS agent_status,COUNT(DISTINCT s.splitter_id) FILTER (WHERE s.status='ACTIVE') AS active_splitters,COUNT(DISTINCT r.roe_id) FILTER (WHERE r.status='ACTIVE') AS active_roes,COUNT(DISTINCT o.ont_id) FILTER (WHERE o.status='ACTIVE') AS active_onts,COUNT(DISTINCT m.modem_id) FILTER (WHERE m.status='ACTIVE') AS active_modems FROM cabinets c LEFT JOIN agents a ON a.cabinet_id=c.cabinet_id AND a.role='CABINET_AGENT' LEFT JOIN splitters s ON s.cabinet_id=c.cabinet_id LEFT JOIN roes r ON r.cabinet_id=c.cabinet_id LEFT JOIN onts o ON o.roe_id=r.roe_id LEFT JOIN ont_modem_links l ON l.ont_id=o.ont_id AND l.status='ACTIVE' LEFT JOIN modems m ON m.modem_id=l.modem_id GROUP BY c.cabinet_id,c.status,a.agent_id,a.status ORDER BY c.cabinet_id")
        .fetch_all(pool)
        .await?;
    let lines = rows
        .into_iter()
        .map(|row| {
            Ok(format!(
                "{}: {} | Agent {} ({}) | splitter {} | ROE {} | ONT {} | modem {}",
                row.try_get::<String, _>("cabinet_id")?,
                row.try_get::<String, _>("cabinet_status")?,
                row.try_get::<Option<String>, _>("agent_id")?
                    .unwrap_or_else(|| "non associato".into()),
                row.try_get::<Option<String>, _>("agent_status")?
                    .unwrap_or_else(|| "non disponibile".into()),
                row.try_get::<i64, _>("active_splitters")?,
                row.try_get::<i64, _>("active_roes")?,
                row.try_get::<i64, _>("active_onts")?,
                row.try_get::<i64, _>("active_modems")?,
            ))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(if lines.is_empty() {
        "Nessun armadio registrato.".into()
    } else {
        lines.join("\n")
    })
}

pub async fn connections(pool: &PgPool) -> Result<String, sqlx::Error> {
    let rows = sqlx::query("SELECT c.cabinet_id,COALESCE(fc.cable_id,'non assegnato') AS cable_id,s.splitter_id,r.roe_id,COALESCE(o.ont_id,'-') AS ont_id,COALESCE(o.serial_number,'-') AS ont_serial,COALESCE(m.modem_id,'-') AS modem_id,COALESCE(m.serial,'-') AS modem_serial FROM cabinets c JOIN splitters s ON s.cabinet_id=c.cabinet_id JOIN roes r ON r.cabinet_id=c.cabinet_id AND r.splitter_id=s.splitter_id LEFT JOIN feeder_cables fc ON fc.cable_id=r.feeder_cable_id LEFT JOIN onts o ON o.roe_id=r.roe_id AND o.status='ACTIVE' LEFT JOIN ont_modem_links l ON l.ont_id=o.ont_id AND l.status='ACTIVE' LEFT JOIN modems m ON m.modem_id=l.modem_id AND m.status='ACTIVE' WHERE c.status='ACTIVE' AND s.status='ACTIVE' AND r.status='ACTIVE' ORDER BY c.cabinet_id,fc.cable_id,s.splitter_id,r.roe_id,o.ont_id,m.modem_id LIMIT 200")
        .fetch_all(pool)
        .await?;
    let lines = rows
        .into_iter()
        .map(|row| {
            Ok(format!(
                "{} → {} → {} → {} → {} ({}) → {} ({})",
                row.try_get::<String, _>("cabinet_id")?,
                row.try_get::<String, _>("cable_id")?,
                row.try_get::<String, _>("splitter_id")?,
                row.try_get::<String, _>("roe_id")?,
                row.try_get::<String, _>("ont_id")?,
                row.try_get::<String, _>("ont_serial")?,
                row.try_get::<String, _>("modem_id")?,
                row.try_get::<String, _>("modem_serial")?,
            ))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(if lines.is_empty() {
        "Nessun collegamento attivo trovato.".into()
    } else {
        lines.join("\n")
    })
}

pub async fn search(pool: &PgPool, query: &str) -> Result<String, sqlx::Error> {
    let query = query.trim();
    if query.is_empty() || query.len() > 128 || query.chars().any(char::is_control) {
        return Ok("Termine di ricerca non valido.".into());
    }
    let pattern = format!("%{query}%");
    let rows = sqlx::query("SELECT 'ROE' AS kind,r.roe_id AS resource_id,r.address_reference,COALESCE(r.street_name,'') AS street_name,COALESCE(r.street_number,'') AS street_number,COALESCE(r.city,'') AS city,COALESCE(r.postal_code,'') AS postal_code,r.cabinet_id,r.splitter_id,NULL::text AS serial FROM roes r WHERE r.roe_id ILIKE $1 OR r.address_reference ILIKE $1 OR r.street_name ILIKE $1 OR r.street_number ILIKE $1 OR r.city ILIKE $1 OR r.postal_code ILIKE $1 UNION ALL SELECT 'ONT',o.ont_id,'', '', '', '', '',r.cabinet_id,r.splitter_id,o.serial_number FROM onts o JOIN roes r ON r.roe_id=o.roe_id WHERE o.ont_id ILIKE $1 OR o.serial_number ILIKE $1 UNION ALL SELECT 'MODEM',m.modem_id,'', '', '', '', '',r.cabinet_id,r.splitter_id,m.serial FROM modems m JOIN ont_modem_links l ON l.modem_id=m.modem_id AND l.status='ACTIVE' JOIN onts o ON o.ont_id=l.ont_id JOIN roes r ON r.roe_id=o.roe_id WHERE m.modem_id ILIKE $1 OR m.serial ILIKE $1 ORDER BY kind,resource_id LIMIT 50")
        .bind(pattern)
        .fetch_all(pool)
        .await?;
    let lines = rows
        .into_iter()
        .map(|row| {
            let kind: String = row.try_get("kind")?;
            let resource_id: String = row.try_get("resource_id")?;
            let cabinet_id: String = row.try_get("cabinet_id")?;
            let splitter_id: String = row.try_get("splitter_id")?;
            let serial: Option<String> = row.try_get("serial")?;
            if kind == "ROE" {
                let address: String = row.try_get("address_reference")?;
                let street: String = row.try_get("street_name")?;
                let number: String = row.try_get("street_number")?;
                let city: String = row.try_get("city")?;
                let postal: String = row.try_get("postal_code")?;
                let address = if street.is_empty() {
                    address
                } else {
                    format!("{street} {number}, {postal} {city}")
                };
                Ok(format!(
                    "ROE {resource_id} | {address} | {cabinet_id} → {splitter_id}"
                ))
            } else {
                Ok(format!(
                    "{kind} {resource_id} | seriale {} | {cabinet_id} → {splitter_id}",
                    serial.unwrap_or_else(|| "-".into())
                ))
            }
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(if lines.is_empty() {
        "Nessun apparato corrispondente.".into()
    } else {
        lines.join("\n")
    })
}

pub async fn roe_locations(pool: &PgPool) -> Result<String, sqlx::Error> {
    let rows = sqlx::query("SELECT roe_id,COALESCE(street_name,'') AS street_name,COALESCE(street_number,'') AS street_number,COALESCE(city,'') AS city,latitude,longitude,location_accuracy_m FROM roes WHERE status='ACTIVE' ORDER BY roe_id LIMIT 100")
        .fetch_all(pool)
        .await?;
    let lines = rows
        .into_iter()
        .map(|row| {
            let roe_id: String = row.try_get("roe_id")?;
            let street: String = row.try_get("street_name")?;
            let number: String = row.try_get("street_number")?;
            let city: String = row.try_get("city")?;
            let latitude: Option<f64> = row.try_get("latitude")?;
            let longitude: Option<f64> = row.try_get("longitude")?;
            let accuracy: Option<f64> = row.try_get("location_accuracy_m")?;
            Ok(match (latitude, longitude) {
                (Some(latitude), Some(longitude)) => format!(
                    "{roe_id} | {street} {number}, {city} | {latitude:.6}, {longitude:.6} | precisione {} m",
                    accuracy.map(|value| format!("{value:.1}")).unwrap_or_else(|| "n/d".into())
                ),
                _ => format!("{roe_id} | {street} {number}, {city} | posizione assente"),
            })
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    Ok(if lines.is_empty() {
        "Nessun ROE attivo.".into()
    } else {
        lines.join("\n")
    })
}

pub async fn nearby_roes<'e, E>(
    executor: E,
    technician_latitude: f64,
    technician_longitude: f64,
    radius_m: f64,
) -> Result<Vec<NearbyRoeCandidate>, sqlx::Error>
where
    E: Executor<'e, Database = Postgres>,
{
    if distance_meters(
        technician_latitude,
        technician_longitude,
        technician_latitude,
        technician_longitude,
    )
    .is_none()
        || !radius_m.is_finite()
        || !(100.0..=100_000.0).contains(&radius_m)
    {
        return Ok(Vec::new());
    }
    let rows = sqlx::query("SELECT r.roe_id,COALESCE(r.street_name,'') AS street_name,COALESCE(r.street_number,'') AS street_number,COALESCE(r.city,'') AS city,r.cabinet_id,r.splitter_id,r.latitude,r.longitude,fc.fiber_count,fc.fibers_used,fc.fibers_reserved FROM roes r JOIN cabinets c ON c.cabinet_id=r.cabinet_id AND c.status='ACTIVE' JOIN splitters s ON s.splitter_id=r.splitter_id AND s.cabinet_id=r.cabinet_id AND s.status='ACTIVE' LEFT JOIN feeder_cables fc ON fc.cable_id=r.feeder_cable_id WHERE r.status='ACTIVE' AND r.latitude IS NOT NULL AND r.longitude IS NOT NULL ORDER BY r.roe_id")
        .fetch_all(executor)
        .await?;
    let rows = rows
        .into_iter()
        .map(|row| -> Result<Option<NearbyRoeCandidate>, sqlx::Error> {
            let latitude: f64 = row.try_get("latitude")?;
            let longitude: f64 = row.try_get("longitude")?;
            let Some(distance_m) = distance_meters(
                technician_latitude,
                technician_longitude,
                latitude,
                longitude,
            ) else {
                return Ok(None);
            };
            if distance_m > radius_m {
                return Ok(None);
            }
            let fiber_count: Option<i32> = row.try_get("fiber_count")?;
            let fibers_used: Option<i32> = row.try_get("fibers_used")?;
            let fibers_reserved: Option<i32> = row.try_get("fibers_reserved")?;
            let available_fibers = fiber_count.zip(fibers_used).zip(fibers_reserved).and_then(
                |((total, used), reserved)| available_feeder_fibers(total, used, reserved),
            );
            Ok(Some(NearbyRoeCandidate {
                roe_id: row.try_get("roe_id")?,
                street_name: row.try_get("street_name")?,
                street_number: row.try_get("street_number")?,
                city: row.try_get("city")?,
                cabinet_id: row.try_get("cabinet_id")?,
                splitter_id: row.try_get("splitter_id")?,
                available_fibers,
                distance_m,
            }))
        })
        .collect::<Result<Vec<_>, sqlx::Error>>()?;
    let mut candidates = rows.into_iter().flatten().collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.distance_m.total_cmp(&right.distance_m));
    candidates.truncate(20);
    Ok(candidates)
}
