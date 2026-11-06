use crate::config::OpenStackConfig;
use reqwest::Client;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::{collections::HashMap, error::Error, time::Duration};

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

#[derive(Clone, Deserialize)]
pub struct OpenStackServer {
    pub id: String,
    pub name: String,
    pub status: String,
}

#[derive(Clone, Deserialize)]
pub struct OpenStackNetwork {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(rename = "provider:network_type")]
    pub network_type: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct OpenStackPort {
    pub network_id: String,
    pub device_id: String,
    #[serde(default)]
    pub security_groups: Vec<String>,
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
    pub images: Vec<OpenStackImage>,
    pub compute_services: Vec<ComputeService>,
    pub hypervisors: Vec<Hypervisor>,
    pub console_logs: HashMap<String, String>,
    pub probe_verified_at: Option<String>,
    pub ovn_backend: Option<String>,
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
        let evidence = self.probe_evidence();
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
            images,
            compute_services,
            hypervisors,
            console_logs,
            probe_verified_at,
            ovn_backend,
        })
    }

    fn probe_evidence(&self) -> Option<ProbeEvidence> {
        let metadata = std::fs::metadata(&self.probe_evidence_path).ok()?;
        let age = metadata.modified().ok()?.elapsed().ok()?;
        if age > Duration::from_secs(60 * 60) {
            return None;
        }
        let contents = std::fs::read_to_string(&self.probe_evidence_path).ok()?;
        serde_json::from_str(&contents).ok()
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
struct PortList {
    ports: Vec<OpenStackPort>,
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
    verified_at: String,
    ovn_backend: String,
    console_logs: HashMap<String, String>,
}
