use crate::config::OpenStackConfig;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
pub struct OpenStackClient {
    client: Client,
    auth_url: String,
    application_credential_id: String,
    application_credential_secret: String,
    compute_url: String,
    network_url: String,
    image_url: String,
    probe_evidence_path: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OpenStackServer {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OpenStackNetwork {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(rename = "provider:network_type")]
    pub network_type: Option<String>,
    #[serde(default)]
    pub shared: bool,
    #[serde(default, rename = "router:external")]
    pub external: bool,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OpenStackPort {
    pub id: String,
    pub name: String,
    pub network_id: String,
    pub device_id: String,
    pub device_owner: String,
    pub status: String,
    #[serde(default, rename = "security_groups", alias = "security_group_ids")]
    pub security_group_ids: Vec<String>,
    #[serde(default)]
    pub fixed_ips: Vec<FixedIp>,
    pub port_security_enabled: Option<bool>,
}

#[derive(Clone, Deserialize, Serialize, Eq, Ord, PartialEq, PartialOrd)]
pub struct FixedIp {
    pub subnet_id: String,
    pub ip_address: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OpenStackSubnet {
    pub id: String,
    pub name: String,
    pub network_id: String,
    pub cidr: String,
    pub gateway_ip: Option<String>,
    pub ip_version: u8,
    pub enable_dhcp: bool,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OpenStackSecurityGroup {
    pub id: String,
    pub name: String,
    pub stateful: Option<bool>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct OpenStackSecurityGroupRule {
    pub id: String,
    pub security_group_id: String,
    pub direction: String,
    pub ethertype: String,
    pub protocol: Option<String>,
    pub port_range_min: Option<i32>,
    pub port_range_max: Option<i32>,
    pub remote_ip_prefix: Option<String>,
    pub remote_group_id: Option<String>,
    pub remote_address_group_id: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct OpenStackImage {
    pub name: String,
    pub status: String,
}

#[derive(Clone, Deserialize)]
pub struct ComputeService {
    pub binary: String,
    pub status: String,
    pub state: String,
}

#[derive(Clone, Deserialize)]
pub struct Hypervisor {
    pub state: String,
}

pub struct OpenStackSnapshot {
    pub servers: Vec<OpenStackServer>,
    pub networks: Vec<OpenStackNetwork>,
    pub ports: Vec<OpenStackPort>,
    pub subnets: Vec<OpenStackSubnet>,
    pub security_groups: Vec<OpenStackSecurityGroup>,
    pub security_group_rules: Vec<OpenStackSecurityGroupRule>,
    pub images: Vec<OpenStackImage>,
    pub compute_services: Vec<ComputeService>,
    pub hypervisors: Vec<Hypervisor>,
    pub console_logs: HashMap<String, String>,
    pub probe_verified_at: Option<String>,
    pub ovn_backend: Option<String>,
    pub current_topology_fingerprint: Option<String>,
    pub probe_topology_fingerprint: Option<String>,
    pub guest_probe_pass: bool,
    pub probe_valid: bool,
    pub topology_unchanged: bool,
    pub guest_probe_age_seconds: Option<u64>,
}

#[derive(Clone, Serialize)]
struct TopologyMaterial {
    schema_version: u32,
    servers: Vec<OpenStackServer>,
    networks: Vec<OpenStackNetwork>,
    subnets: Vec<OpenStackSubnet>,
    ports: Vec<OpenStackPort>,
    security_groups: Vec<OpenStackSecurityGroup>,
    security_group_rules: Vec<OpenStackSecurityGroupRule>,
}

impl TopologyMaterial {
    fn fingerprint(&self) -> Option<String> {
        let mut canonical = self.clone();
        canonical
            .servers
            .sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        canonical
            .networks
            .sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
        canonical
            .subnets
            .sort_by(|left, right| left.id.cmp(&right.id));
        for port in &mut canonical.ports {
            port.security_group_ids.sort();
            port.fixed_ips.sort();
        }
        canonical
            .ports
            .sort_by(|left, right| left.id.cmp(&right.id));
        canonical
            .security_groups
            .sort_by(|left, right| left.id.cmp(&right.id));
        canonical
            .security_group_rules
            .sort_by(|left, right| left.id.cmp(&right.id));
        let canonical = serde_json::to_vec(&canonical).ok()?;
        let digest = Sha256::digest(canonical);
        Some(digest.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

impl OpenStackClient {
    pub fn new(config: &OpenStackConfig) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            client: Client::builder()
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(10))
                .build()?,
            auth_url: config.auth_url.trim_end_matches('/').to_owned(),
            application_credential_id: config.application_credential_id.clone(),
            application_credential_secret: config.application_credential_secret.clone(),
            compute_url: config.compute_url.trim_end_matches('/').to_owned(),
            network_url: config.network_url.trim_end_matches('/').to_owned(),
            image_url: config.image_url.trim_end_matches('/').to_owned(),
            probe_evidence_path: config.probe_evidence_path.clone(),
        })
    }

    pub async fn snapshot(
        &self,
        include_console_logs: bool,
    ) -> Result<OpenStackSnapshot, &'static str> {
        let token = self.authenticate().await?;
        let servers = self
            .get::<ServerList>(&token, &format!("{}/servers/detail", self.compute_url))
            .await
            .map_err(|error| match error {
                "openstack_unauthorized" => "nova_servers_unauthorized",
                "openstack_forbidden" => "nova_servers_forbidden",
                "openstack_not_found" => "nova_servers_endpoint_not_found",
                "openstack_response_invalid" => "nova_servers_invalid_response",
                _ => "nova_servers_unavailable",
            })?
            .servers;
        let networks = self
            .get::<NetworkList>(&token, &format!("{}/networks", self.network_url))
            .await
            .map_err(|_| "neutron_networks_unavailable")?
            .networks;
        let ports = self
            .get::<PortList>(&token, &format!("{}/ports", self.network_url))
            .await
            .map_err(|_| "neutron_ports_unavailable")?
            .ports;
        let subnets = self
            .get::<SubnetList>(&token, &format!("{}/subnets", self.network_url))
            .await
            .map_err(|_| "neutron_subnets_unavailable")?
            .subnets;
        let security_groups = self
            .get::<SecurityGroupList>(&token, &format!("{}/security-groups", self.network_url))
            .await
            .map_err(|_| "neutron_security_groups_unavailable")?
            .security_groups;
        let security_group_rules = self
            .get::<SecurityGroupRuleList>(
                &token,
                &format!("{}/security-group-rules", self.network_url),
            )
            .await
            .map_err(|_| "neutron_security_group_rules_unavailable")?
            .security_group_rules;
        let images = self
            .get::<ImageList>(&token, &format!("{}/images?limit=1000", self.image_url))
            .await
            .map_err(|_| "glance_images_unavailable")?
            .images;
        let compute_services = self
            .get::<ComputeServiceList>(&token, &format!("{}/os-services", self.compute_url))
            .await
            .map(|response| response.services)
            .unwrap_or_default();
        let hypervisors = self
            .get::<HypervisorList>(
                &token,
                &format!("{}/os-hypervisors/detail", self.compute_url),
            )
            .await
            .map(|response| response.hypervisors)
            .unwrap_or_default();
        let current_topology_fingerprint = topology_fingerprint(
            &servers,
            &networks,
            &subnets,
            &ports,
            &security_groups,
            &security_group_rules,
        );
        let evidence = self.probe_evidence();
        let guest_probe_pass = evidence
            .as_ref()
            .is_some_and(ProbeEvidence::all_guest_probes_passed);
        let probe_topology_fingerprint = evidence
            .as_ref()
            .map(|probe| probe.topology_fingerprint.clone());
        let topology_unchanged = evidence
            .as_ref()
            .is_some_and(|probe| probe.schema_version == 1)
            && probe_topology_fingerprint.is_some()
            && current_topology_fingerprint.is_some()
            && probe_topology_fingerprint == current_topology_fingerprint;
        let probe_valid =
            probe_matches_fingerprint(evidence.as_ref(), current_topology_fingerprint.as_deref());
        let guest_probe_age_seconds = evidence
            .as_ref()
            .and_then(|probe| rfc3339_age_seconds(&probe.verified_at));
        let mut console_logs = evidence
            .as_ref()
            .map(|evidence| evidence.console_logs.clone())
            .unwrap_or_default();
        if include_console_logs {
            for server in servers
                .iter()
                .filter(|server| server.name == "CENTRAL-01" || server.name.starts_with("CAB-"))
            {
                if let Some(log) = self
                    .console_output(&token, &self.compute_url, &server.id)
                    .await
                {
                    console_logs.insert(server.name.clone(), log);
                }
            }
        }
        let probe_verified_at = evidence
            .as_ref()
            .map(|evidence| evidence.verified_at.clone());
        let ovn_backend = evidence.map(|evidence| evidence.ovn_backend);
        Ok(OpenStackSnapshot {
            servers,
            networks,
            ports,
            subnets,
            security_groups,
            security_group_rules,
            images,
            compute_services,
            hypervisors,
            console_logs,
            probe_verified_at,
            ovn_backend,
            current_topology_fingerprint,
            probe_topology_fingerprint,
            guest_probe_pass,
            probe_valid,
            topology_unchanged,
            guest_probe_age_seconds,
        })
    }

    fn probe_evidence(&self) -> Option<ProbeEvidence> {
        let contents = std::fs::read_to_string(&self.probe_evidence_path).ok()?;
        let evidence = serde_json::from_str::<ProbeEvidence>(&contents).ok()?;
        (evidence.schema_version == 1 && valid_sha256(&evidence.topology_fingerprint))
            .then_some(evidence)
    }

    async fn authenticate(&self) -> Result<String, &'static str> {
        let response = self
            .client
            .post(format!("{}/auth/tokens", self.auth_url))
            .json(&json!({
                "auth": {
                    "identity": {
                        "methods": ["application_credential"],
                        "application_credential": {
                            "id": self.application_credential_id,
                            "secret": self.application_credential_secret
                        }
                    }
                }
            }))
            .send()
            .await
            .map_err(|_| "keystone_unavailable")?;
        if !response.status().is_success() {
            return Err("keystone_authentication_failed");
        }
        response
            .headers()
            .get("x-subject-token")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or("keystone_token_missing")
    }

    async fn get<T: DeserializeOwned>(&self, token: &str, url: &str) -> Result<T, &'static str> {
        let response = self
            .client
            .get(url)
            .header("X-Auth-Token", token)
            .send()
            .await
            .map_err(|_| "openstack_api_unavailable")?;
        if !response.status().is_success() {
            return Err(match response.status().as_u16() {
                401 => "openstack_unauthorized",
                403 => "openstack_forbidden",
                404 => "openstack_not_found",
                _ => "openstack_api_unavailable",
            });
        }
        response
            .json()
            .await
            .map_err(|_| "openstack_response_invalid")
    }

    async fn console_output(
        &self,
        token: &str,
        project_url: &str,
        server_id: &str,
    ) -> Option<String> {
        let response = self
            .client
            .post(format!("{project_url}/servers/{server_id}/action"))
            .header("X-Auth-Token", token)
            .json(&json!({"os-getConsoleOutput": {"length": 20000}}))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        response
            .json::<ConsoleOutput>()
            .await
            .ok()
            .map(|output| output.output)
    }
}

#[derive(Deserialize)]
struct ServerList {
    servers: Vec<OpenStackServer>,
}

#[derive(Deserialize)]
struct NetworkList {
    networks: Vec<OpenStackNetwork>,
}

#[derive(Deserialize)]
struct SubnetList {
    subnets: Vec<OpenStackSubnet>,
}

#[derive(Deserialize)]
struct PortList {
    ports: Vec<OpenStackPort>,
}

#[derive(Deserialize)]
struct SecurityGroupList {
    security_groups: Vec<OpenStackSecurityGroup>,
}

#[derive(Deserialize)]
struct SecurityGroupRuleList {
    security_group_rules: Vec<OpenStackSecurityGroupRule>,
}

#[derive(Deserialize)]
struct ImageList {
    images: Vec<OpenStackImage>,
}

#[derive(Deserialize)]
struct ComputeServiceList {
    services: Vec<ComputeService>,
}

#[derive(Deserialize)]
struct HypervisorList {
    hypervisors: Vec<Hypervisor>,
}

#[derive(Deserialize)]
struct ConsoleOutput {
    output: String,
}

#[derive(Deserialize)]
struct ProbeEvidence {
    schema_version: u32,
    topology_fingerprint: String,
    verified_at: String,
    ovn_backend: String,
    console_logs: HashMap<String, String>,
}

impl ProbeEvidence {
    fn all_guest_probes_passed(&self) -> bool {
        if self.ovn_backend != "geneve" {
            return false;
        }
        let expectations = [
            ("CENTRAL-01", "CENTRAL_TO_CAB_A", "PASS"),
            ("CENTRAL-01", "CENTRAL_TO_CAB_B", "PASS"),
            ("CENTRAL-01", "CENTRAL_TO_CAB_C", "PASS"),
            ("CAB-A", "CAB_A_TO_CENTRAL", "PASS"),
            ("CAB-A", "CAB_A_TO_CAB_B", "BLOCKED"),
            ("CAB-A", "CAB_A_TO_CAB_C", "BLOCKED"),
            ("CAB-B", "CAB_B_TO_CENTRAL", "PASS"),
            ("CAB-B", "CAB_B_TO_CAB_A", "BLOCKED"),
            ("CAB-B", "CAB_B_TO_CAB_C", "BLOCKED"),
            ("CAB-C", "CAB_C_TO_CENTRAL", "PASS"),
            ("CAB-C", "CAB_C_TO_CAB_A", "BLOCKED"),
            ("CAB-C", "CAB_C_TO_CAB_B", "BLOCKED"),
        ];
        expectations.into_iter().all(|(server, label, expected)| {
            self.console_logs
                .get(server)
                .and_then(|log| guest_probe_matches(log, label, expected))
                == Some(true)
        })
    }

    fn matches_topology(&self, current_fingerprint: Option<&str>) -> bool {
        self.schema_version == 1
            && valid_sha256(&self.topology_fingerprint)
            && rfc3339_age_seconds(&self.verified_at).is_some()
            && self.all_guest_probes_passed()
            && current_fingerprint == Some(self.topology_fingerprint.as_str())
    }
}

fn topology_fingerprint(
    servers: &[OpenStackServer],
    networks: &[OpenStackNetwork],
    subnets: &[OpenStackSubnet],
    ports: &[OpenStackPort],
    security_groups: &[OpenStackSecurityGroup],
    security_group_rules: &[OpenStackSecurityGroupRule],
) -> Option<String> {
    const SERVER_NAMES: [&str; 4] = ["CAB-A", "CAB-B", "CAB-C", "CENTRAL-01"];
    const NETWORK_NAMES: [&str; 6] = [
        "FS-LINE-A",
        "FS-LINE-B",
        "FS-LINE-C",
        "FS-MGMT-A",
        "FS-MGMT-B",
        "FS-MGMT-C",
    ];

    let mut relevant_servers = SERVER_NAMES
        .iter()
        .map(|name| servers.iter().find(|server| server.name == *name).cloned())
        .collect::<Option<Vec<_>>>()?;
    relevant_servers.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    let server_ids = relevant_servers
        .iter()
        .map(|server| server.id.as_str())
        .collect::<HashSet<_>>();

    let mut relevant_networks = NETWORK_NAMES
        .iter()
        .map(|name| {
            networks
                .iter()
                .find(|network| network.name == *name)
                .cloned()
        })
        .collect::<Option<Vec<_>>>()?;
    relevant_networks
        .sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    let network_ids = relevant_networks
        .iter()
        .map(|network| network.id.as_str())
        .collect::<HashSet<_>>();

    let mut relevant_ports = ports
        .iter()
        .filter(|port| {
            server_ids.contains(port.device_id.as_str())
                || network_ids.contains(port.network_id.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    for port in &mut relevant_ports {
        port.security_group_ids.sort();
        port.fixed_ips.sort();
    }
    relevant_ports.sort_by(|left, right| left.id.cmp(&right.id));
    let security_group_ids = relevant_ports
        .iter()
        .flat_map(|port| port.security_group_ids.iter().map(String::as_str))
        .collect::<HashSet<_>>();

    let mut relevant_subnets = subnets
        .iter()
        .filter(|subnet| network_ids.contains(subnet.network_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if NETWORK_NAMES.iter().any(|name| {
        let network_id = relevant_networks
            .iter()
            .find(|network| network.name == *name)
            .map(|network| network.id.as_str());
        network_id.is_none_or(|id| {
            !relevant_subnets
                .iter()
                .any(|subnet| subnet.network_id == id)
        })
    }) {
        return None;
    }
    relevant_subnets.sort_by(|left, right| left.id.cmp(&right.id));

    let mut relevant_security_groups = security_groups
        .iter()
        .filter(|group| security_group_ids.contains(group.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if security_group_ids
        .iter()
        .any(|id| !relevant_security_groups.iter().any(|group| group.id == *id))
    {
        return None;
    }
    relevant_security_groups.sort_by(|left, right| left.id.cmp(&right.id));
    let mut relevant_rules = security_group_rules
        .iter()
        .filter(|rule| security_group_ids.contains(rule.security_group_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    relevant_rules.sort_by(|left, right| left.id.cmp(&right.id));

    TopologyMaterial {
        schema_version: 1,
        servers: relevant_servers,
        networks: relevant_networks,
        subnets: relevant_subnets,
        ports: relevant_ports,
        security_groups: relevant_security_groups,
        security_group_rules: relevant_rules,
    }
    .fingerprint()
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn probe_matches_fingerprint(
    evidence: Option<&ProbeEvidence>,
    current_fingerprint: Option<&str>,
) -> bool {
    evidence.is_some_and(|probe| probe.matches_topology(current_fingerprint))
}

fn guest_probe_matches(log: &str, label: &str, expected: &str) -> Option<bool> {
    let marker = format!("FS_TEST {label} ");
    let line = log.lines().find(|line| line.contains(&marker))?;
    let has = |field: &str| line.split_ascii_whitespace().any(|part| part == field);
    Some(has(&format!("ICMP={expected}")) && has(&format!("TCP={expected}")) && has("RESULT=PASS"))
}

fn rfc3339_age_seconds(timestamp: &str) -> Option<u64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(timestamp).ok()?;
    let verified = u64::try_from(parsed.timestamp()).ok()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    now.checked_sub(verified)
}

pub fn required_nova_servers_active(servers: &[OpenStackServer]) -> bool {
    ["CAB-A", "CAB-B", "CAB-C", "CENTRAL-01"]
        .iter()
        .all(|name| {
            servers
                .iter()
                .any(|server| server.name == *name && server.status == "ACTIVE")
        })
}

#[cfg(test)]
mod fingerprint_tests {
    use super::{
        FixedIp, OpenStackNetwork, OpenStackPort, OpenStackSecurityGroup,
        OpenStackSecurityGroupRule, OpenStackServer, OpenStackSubnet, ProbeEvidence,
        TopologyMaterial, probe_matches_fingerprint, required_nova_servers_active,
    };
    use std::collections::HashMap;

    fn topology() -> TopologyMaterial {
        TopologyMaterial {
            schema_version: 1,
            servers: vec![OpenStackServer {
                id: "server-cab-a".to_owned(),
                name: "CAB-A".to_owned(),
                status: "ACTIVE".to_owned(),
            }],
            networks: vec![OpenStackNetwork {
                id: "network-mgmt-a".to_owned(),
                name: "FS-MGMT-A".to_owned(),
                status: "ACTIVE".to_owned(),
                network_type: None,
                shared: false,
                external: false,
            }],
            subnets: vec![OpenStackSubnet {
                id: "subnet-mgmt-a".to_owned(),
                name: "FS-MGMT-A-subnet".to_owned(),
                network_id: "network-mgmt-a".to_owned(),
                cidr: "10.91.10.0/24".to_owned(),
                gateway_ip: None,
                ip_version: 4,
                enable_dhcp: true,
            }],
            ports: vec![OpenStackPort {
                id: "port-cab-a-mgmt".to_owned(),
                name: "cab-a-mgmt".to_owned(),
                network_id: "network-mgmt-a".to_owned(),
                device_id: "server-cab-a".to_owned(),
                device_owner: "compute:nova".to_owned(),
                status: "ACTIVE".to_owned(),
                security_group_ids: vec!["sg-cab-a".to_owned()],
                fixed_ips: vec![FixedIp {
                    subnet_id: "subnet-mgmt-a".to_owned(),
                    ip_address: "10.91.10.20".to_owned(),
                }],
                port_security_enabled: Some(true),
            }],
            security_groups: vec![OpenStackSecurityGroup {
                id: "sg-cab-a".to_owned(),
                name: "fs-cab-a".to_owned(),
                stateful: Some(true),
            }],
            security_group_rules: vec![OpenStackSecurityGroupRule {
                id: "rule-cab-a-central-icmp".to_owned(),
                security_group_id: "sg-cab-a".to_owned(),
                direction: "ingress".to_owned(),
                ethertype: "IPv4".to_owned(),
                protocol: Some("icmp".to_owned()),
                port_range_min: None,
                port_range_max: None,
                remote_ip_prefix: Some("10.91.10.10/32".to_owned()),
                remote_group_id: None,
                remote_address_group_id: None,
            }],
        }
    }

    fn passing_probe(fingerprint: String) -> ProbeEvidence {
        let mut console_logs = HashMap::new();
        console_logs.insert(
            "CENTRAL-01".to_owned(),
            "FS_TEST CENTRAL_TO_CAB_A ICMP=PASS TCP=PASS RESULT=PASS\nFS_TEST CENTRAL_TO_CAB_B ICMP=PASS TCP=PASS RESULT=PASS\nFS_TEST CENTRAL_TO_CAB_C ICMP=PASS TCP=PASS RESULT=PASS".to_owned(),
        );
        console_logs.insert(
            "CAB-A".to_owned(),
            "FS_TEST CAB_A_TO_CENTRAL ICMP=PASS TCP=PASS RESULT=PASS\nFS_TEST CAB_A_TO_CAB_B ICMP=BLOCKED TCP=BLOCKED RESULT=PASS\nFS_TEST CAB_A_TO_CAB_C ICMP=BLOCKED TCP=BLOCKED RESULT=PASS".to_owned(),
        );
        console_logs.insert(
            "CAB-B".to_owned(),
            "FS_TEST CAB_B_TO_CENTRAL ICMP=PASS TCP=PASS RESULT=PASS\nFS_TEST CAB_B_TO_CAB_A ICMP=BLOCKED TCP=BLOCKED RESULT=PASS\nFS_TEST CAB_B_TO_CAB_C ICMP=BLOCKED TCP=BLOCKED RESULT=PASS".to_owned(),
        );
        console_logs.insert(
            "CAB-C".to_owned(),
            "FS_TEST CAB_C_TO_CENTRAL ICMP=PASS TCP=PASS RESULT=PASS\nFS_TEST CAB_C_TO_CAB_A ICMP=BLOCKED TCP=BLOCKED RESULT=PASS\nFS_TEST CAB_C_TO_CAB_B ICMP=BLOCKED TCP=BLOCKED RESULT=PASS".to_owned(),
        );
        ProbeEvidence {
            schema_version: 1,
            topology_fingerprint: fingerprint,
            verified_at: "2026-01-01T00:00:00Z".to_owned(),
            ovn_backend: "geneve".to_owned(),
            console_logs,
        }
    }

    #[test]
    fn old_passing_probe_is_valid_while_topology_fingerprint_matches() {
        let fingerprint = topology().fingerprint().unwrap();
        let probe = passing_probe(fingerprint.clone());
        assert!(probe.matches_topology(Some(&fingerprint)));
    }

    #[test]
    fn failing_probe_is_not_valid_even_when_topology_matches() {
        let fingerprint = topology().fingerprint().unwrap();
        let mut probe = passing_probe(fingerprint.clone());
        probe.console_logs.get_mut("CAB-A").unwrap().replace_range(
            0.."FS_TEST CAB_A_TO_CENTRAL ICMP=PASS TCP=PASS RESULT=PASS".len(),
            "FS_TEST CAB_A_TO_CENTRAL ICMP=FAIL TCP=PASS RESULT=FAIL",
        );
        assert!(!probe.matches_topology(Some(&fingerprint)));
    }

    #[test]
    fn changed_port_network_security_group_or_rule_changes_fingerprint() {
        let original = topology().fingerprint().unwrap();
        let mut changed_port = topology();
        changed_port.ports[0].device_id = "other-server".to_owned();
        assert_ne!(original, changed_port.fingerprint().unwrap());

        let mut changed_network = topology();
        changed_network.networks[0].id = "replacement-network".to_owned();
        assert_ne!(original, changed_network.fingerprint().unwrap());

        let mut changed_rule = topology();
        changed_rule.security_group_rules[0].remote_ip_prefix = Some("10.91.10.0/24".to_owned());
        assert_ne!(original, changed_rule.fingerprint().unwrap());

        let mut changed_group = topology();
        changed_group.security_groups[0].stateful = Some(false);
        assert_ne!(original, changed_group.fingerprint().unwrap());
    }

    #[test]
    fn fingerprint_is_independent_of_input_order() {
        let original = topology().fingerprint().unwrap();
        let mut reordered = topology();
        reordered.ports[0].fixed_ips.reverse();
        reordered.ports[0].security_group_ids.reverse();
        reordered.security_group_rules.reverse();
        assert_eq!(original, reordered.fingerprint().unwrap());
    }

    #[test]
    fn mismatched_missing_or_invalid_probe_fails_closed() {
        let fingerprint = topology().fingerprint().unwrap();
        let probe = passing_probe(fingerprint);
        assert!(!probe.matches_topology(Some(&"f".repeat(64))));
        assert!(!probe.matches_topology(None));
        let mut invalid = passing_probe("not-a-sha256".to_owned());
        invalid.schema_version = 2;
        assert!(!invalid.matches_topology(Some("not-a-sha256")));
        let fingerprint = topology().fingerprint().unwrap();
        let mut invalid_timestamp = passing_probe(fingerprint.clone());
        invalid_timestamp.verified_at = "not-a-timestamp".to_owned();
        assert!(!invalid_timestamp.matches_topology(Some(&fingerprint)));
        let mut future_timestamp = passing_probe(fingerprint.clone());
        future_timestamp.verified_at = "2999-01-01T00:00:00Z".to_owned();
        assert!(!future_timestamp.matches_topology(Some(&fingerprint)));
        assert!(!probe_matches_fingerprint(
            None,
            Some("a".repeat(64).as_str())
        ));
    }

    #[test]
    fn nova_requires_all_four_dashboard_servers_active() {
        let active = ["CAB-A", "CAB-B", "CAB-C", "CENTRAL-01"]
            .into_iter()
            .map(|name| OpenStackServer {
                id: format!("id-{name}"),
                name: name.to_owned(),
                status: "ACTIVE".to_owned(),
            })
            .collect::<Vec<_>>();
        assert!(required_nova_servers_active(&active));
        let mut failed = active;
        failed[0].status = "ERROR".to_owned();
        assert!(!required_nova_servers_active(&failed));
    }
}
