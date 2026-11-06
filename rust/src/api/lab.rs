use super::{AppState, auth, errors};
use crate::openstack::{OpenStackServer, OpenStackSnapshot};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, Method, StatusCode},
};
use serde_json::{Value, json};
use sqlx::Row;
use std::sync::Arc;

pub async fn openstack(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = auth::authorize_route(&headers, &state, &Method::GET, "/api/lab/openstack")
    {
        return auth::denial(error);
    }
    let Some(client) = &state.openstack else {
        return errors::unavailable("openstack_not_configured");
    };
    let snapshot = match client.snapshot(false).await {
        Ok(snapshot) => snapshot,
        Err(error) => return errors::unavailable(error),
    };
    let nova = snapshot.hypervisors.iter().any(|item| item.state == "up")
        || snapshot
            .servers
            .iter()
            .any(|server| server.status == "ACTIVE");
    let glance = snapshot
        .images
        .iter()
        .any(|image| image.name == "cirros-0.6.3" && image.status == "active");
    let network_names = [
        "FS-MGMT-A",
        "FS-LINE-A",
        "FS-MGMT-B",
        "FS-LINE-B",
        "FS-MGMT-C",
        "FS-LINE-C",
    ];
    let neutron = network_names.iter().all(|name| {
        snapshot
            .networks
            .iter()
            .any(|network| network.name == *name && network.status == "ACTIVE")
    });
    let geneve_visible = network_names.iter().all(|name| {
        snapshot.networks.iter().any(|network| {
            network.name == *name
                && network.status == "ACTIVE"
                && network.network_type.as_deref() == Some("geneve")
        })
    });
    let ovn = geneve_visible
        || (snapshot.ovn_backend.as_deref() == Some("geneve")
            && snapshot.probe_verified_at.is_some());
    let online = nova && glance && neutron && ovn;
    let status = if online { "ONLINE" } else { "DEGRADED" };
    let http_status = if online {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    let hypervisor_count = (!snapshot.hypervisors.is_empty()).then_some(snapshot.hypervisors.len());
    (
        http_status,
        Json(json!({
            "status": status,
            "source": "OpenStack REST API",
            "keystone": "PASS",
            "glance": if glance { "PASS" } else { "FAIL" },
            "nova": if nova { "PASS" } else { "FAIL" },
            "neutron": if neutron { "PASS" } else { "FAIL" },
            "ovn": if ovn { "PASS" } else { "FAIL" },
            "server_count": snapshot.servers.len(),
            "hypervisor_count": hypervisor_count,
            "probe_verified_at": snapshot.probe_verified_at,
        })),
    )
}

pub async fn cabinets(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = auth::authorize_route(&headers, &state, &Method::GET, "/api/lab/cabinets") {
        return auth::denial(error);
    }
    let snapshot = match state.openstack.as_ref() {
        Some(client) => match client.snapshot(true).await {
            Ok(snapshot) => snapshot,
            Err(_) => return errors::unavailable("openstack_unavailable"),
        },
        None => return errors::unavailable("openstack_not_configured"),
    };
    match cabinet_inventory(&state, &snapshot).await {
        Ok(items) => (StatusCode::OK, Json(json!({"cabinets":items}))),
        Err(_) => errors::unavailable("cabinet_store_unavailable"),
    }
}

pub async fn roes(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = auth::authorize_route(&headers, &state, &Method::GET, "/api/lab/roes") {
        return auth::denial(error);
    }
    match crate::db::roes::list(&state.pool).await {
        Ok(rows) => (StatusCode::OK, Json(json!({"roes":rows}))),
        Err(_) => errors::unavailable("roe_store_unavailable"),
    }
}

pub async fn topology(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> (StatusCode, Json<Value>) {
    if let Err(error) = auth::authorize_route(&headers, &state, &Method::GET, "/api/lab/topology") {
        return auth::denial(error);
    }
    let rows = match crate::db::topology::get_topology(&state.pool).await {
        Ok(rows) => rows,
        Err(_) => return errors::unavailable("topology_store_unavailable"),
    };
    let Some(client) = &state.openstack else {
        return errors::unavailable("openstack_not_configured");
    };
    let snapshot = match client.snapshot(true).await {
        Ok(snapshot) => snapshot,
        Err(error) => return errors::unavailable(error),
    };
    let cabinets = match cabinet_inventory(&state, &snapshot).await {
        Ok(cabinets) => cabinets,
        Err(_) => return errors::unavailable("cabinet_store_unavailable"),
    };
    (
        StatusCode::OK,
        Json(json!({"topology":rows,"cabinets":cabinets})),
    )
}

async fn cabinet_inventory(
    state: &AppState,
    snapshot: &OpenStackSnapshot,
) -> Result<Vec<Value>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT c.cabinet_id,c.status AS db_state, COALESCE((SELECT json_agg(json_build_object('splitter_id',s.splitter_id,'line_id',s.line_id,'status',s.status) ORDER BY s.splitter_id) FROM splitters s WHERE s.cabinet_id=c.cabinet_id),'[]'::json) AS splitters, COALESCE((SELECT json_agg(json_build_object('roe_id',r.roe_id,'splitter_id',r.splitter_id,'status',r.status,'address_reference',r.address_reference) ORDER BY r.roe_id) FROM roes r WHERE r.cabinet_id=c.cabinet_id),'[]'::json) AS roes, (SELECT count(*) FROM roes r WHERE r.cabinet_id=c.cabinet_id) AS roe_count FROM cabinets c ORDER BY c.cabinet_id",
    )
    .fetch_all(&state.pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let cabinet_id = row.try_get::<String, _>("cabinet_id")?;
            let db_state = row.try_get::<String, _>("db_state")?;
            let splitters = row.try_get::<Value, _>("splitters")?;
            let roes = row.try_get::<Value, _>("roes")?;
            let roe_count = row.try_get::<i64, _>("roe_count")?;
            Ok(cabinet_status(
                &cabinet_id,
                &db_state,
                splitters,
                roes,
                roe_count,
                snapshot,
            ))
        })
        .collect()
}

fn cabinet_status(
    cabinet_id: &str,
    db_state: &str,
    splitters: Value,
    roes: Value,
    roe_count: i64,
    snapshot: &OpenStackSnapshot,
) -> Value {
    let suffix = cabinet_id.strip_prefix("CAB-").unwrap_or("");
    let mgmt_name = format!("FS-MGMT-{suffix}");
    let ftth_name = format!("FS-LINE-{suffix}");
    let server = snapshot
        .servers
        .iter()
        .find(|server| server.name == cabinet_id);
    let nova_state = server
        .map(|server| server.status.as_str())
        .unwrap_or("MISSING");
    let mgmt_present = network_attached(snapshot, server, &mgmt_name);
    let ftth_present = network_attached(snapshot, server, &ftth_name);
    let isolation = if snapshot.probe_verified_at.is_some() {
        isolation_status(cabinet_id, snapshot)
    } else {
        "UNVERIFIED"
    };
    let central_connectivity = if snapshot.probe_verified_at.is_some() {
        central_connectivity_status(cabinet_id, snapshot)
    } else {
        "UNVERIFIED"
    };
    let active = db_state == "ACTIVE"
        && nova_state == "ACTIVE"
        && mgmt_present
        && ftth_present
        && isolation == "PASS"
        && central_connectivity == "PASS";
    json!({
        "cabinet_id": cabinet_id,
        "db_state": db_state,
        "nova_state": nova_state,
        "server_id": server.map(|server| server.id.as_str()),
        "mgmt_network": mgmt_name,
        "mgmt_network_state": if mgmt_present { "PRESENT" } else { "MISSING" },
        "ftth_network": ftth_name,
        "ftth_network_state": if ftth_present { "PRESENT" } else { "MISSING" },
        "splitters": splitters,
        "roe_count": roe_count,
        "roes": roes,
        "isolation": isolation,
        "central_connectivity": central_connectivity,
        "probe_verified_at": snapshot.probe_verified_at,
        "status": if active { "ACTIVE" } else { "INACTIVE" },
    })
}

fn network_attached(
    snapshot: &OpenStackSnapshot,
    server: Option<&OpenStackServer>,
    network_name: &str,
) -> bool {
    let (Some(server), Some(network)) = (
        server,
        snapshot
            .networks
            .iter()
            .find(|network| network.name == network_name),
    ) else {
        return false;
    };
    network.status == "ACTIVE"
        && snapshot
            .ports
            .iter()
            .any(|port| port.device_id == server.id && port.network_id == network.id)
}

fn isolation_status(cabinet_id: &str, snapshot: &OpenStackSnapshot) -> &'static str {
    let peers = ["CAB-A", "CAB-B", "CAB-C"]
        .into_iter()
        .filter(|peer| *peer != cabinet_id);
    let own_label = cabinet_id.replace('-', "_");
    let mut verified = true;
    for peer in peers {
        let peer_label = peer.replace('-', "_");
        let outbound = format!("{own_label}_TO_{peer_label}");
        let inbound = format!("{peer_label}_TO_{own_label}");
        for (server_name, label) in [(cabinet_id, outbound), (peer, inbound)] {
            match snapshot
                .console_logs
                .get(server_name)
                .and_then(|log| probe_matches(log, &label, "BLOCKED"))
            {
                Some(true) => {}
                Some(false) => return "FAIL",
                None => verified = false,
            }
        }
    }
    if verified { "PASS" } else { "UNVERIFIED" }
}

fn central_connectivity_status(cabinet_id: &str, snapshot: &OpenStackSnapshot) -> &'static str {
    let label = cabinet_id.replace('-', "_");
    let central_to_cabinet = format!("CENTRAL_TO_{label}");
    let cabinet_to_central = format!("{label}_TO_CENTRAL");
    let central_result = snapshot
        .console_logs
        .get("CENTRAL-01")
        .and_then(|log| probe_matches(log, &central_to_cabinet, "PASS"));
    let cabinet_result = snapshot
        .console_logs
        .get(cabinet_id)
        .and_then(|log| probe_matches(log, &cabinet_to_central, "PASS"));
    match (central_result, cabinet_result) {
        (Some(true), Some(true)) => "PASS",
        (Some(false), _) | (_, Some(false)) => "FAIL",
        _ => "UNVERIFIED",
    }
}

fn probe_matches(log: &str, label: &str, expected: &str) -> Option<bool> {
    let marker = format!("FS_TEST {label} ");
    let line = log.lines().find(|line| line.contains(&marker))?;
    let has = |field: &str| line.split_ascii_whitespace().any(|part| part == field);
    Some(has(&format!("ICMP={expected}")) && has(&format!("TCP={expected}")) && has("RESULT=PASS"))
}

#[cfg(test)]
mod tests {
    use super::probe_matches;

    #[test]
    fn probe_requires_both_protocols_and_success_result() {
        let log = "FS_TEST CAB_A_TO_CAB_B ICMP=BLOCKED TCP=BLOCKED RESULT=PASS\n";
        assert_eq!(probe_matches(log, "CAB_A_TO_CAB_B", "BLOCKED"), Some(true));
        assert_eq!(probe_matches(log, "CAB_A_TO_CAB_B", "PASS"), Some(false));
        assert_eq!(probe_matches(log, "CAB_A_TO_CAB_C", "BLOCKED"), None);
    }

    #[test]
    fn failed_tcp_probe_is_not_reported_as_isolated() {
        let log = "FS_TEST CAB_A_TO_CAB_B ICMP=BLOCKED TCP=PASS RESULT=FAIL\n";
        assert_eq!(probe_matches(log, "CAB_A_TO_CAB_B", "BLOCKED"), Some(false));
    }
}
