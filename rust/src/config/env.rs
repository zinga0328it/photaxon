use super::{AppConfig, DatabaseConfig, OpenStackConfig, ServerConfig, validation};
use crate::credentials::{env_provider::EnvCredentialProvider, provider::CredentialProvider};
use crate::security::{secrets, tokens::AuthTokens};
use std::{error::Error, net::SocketAddr, sync::Arc};

pub fn load_database_from_file(path: &std::path::Path) -> Result<DatabaseConfig, Box<dyn Error>> {
    let database = DatabaseConfig {
        host: secrets::required_from_file(path, "FIBERSPIDER_DB_HOST")?.parse()?,
        port: secrets::required_from_file(path, "FIBERSPIDER_DB_PORT")?.parse()?,
        name: secrets::required_from_file(path, "FIBERSPIDER_DB_NAME")?,
        user: secrets::required_from_file(path, "FIBERSPIDER_DB_USER")?,
        password: secrets::required_from_file(path, "FIBERSPIDER_DB_PASSWORD")?,
    };
    if !validation::is_yggdrasil(database.host) || database.port == 0 {
        return Err("AAA must use a configured Yggdrasil IPv6 address and valid port".into());
    }
    Ok(database)
}

pub fn load() -> Result<AppConfig, Box<dyn Error>> {
    secrets::load_env_file()?;
    let bind_addr: SocketAddr = secrets::required("FIBERSPIDER_BIND_ADDR")?.parse()?;
    let api_port: u16 = secrets::required("FIBERSPIDER_API_PORT")?.parse()?;
    let database = DatabaseConfig {
        host: secrets::required("FIBERSPIDER_DB_HOST")?.parse()?,
        port: secrets::required("FIBERSPIDER_DB_PORT")?.parse()?,
        name: secrets::required("FIBERSPIDER_DB_NAME")?,
        user: secrets::required("FIBERSPIDER_DB_USER")?,
        password: secrets::required("FIBERSPIDER_DB_PASSWORD")?,
    };
    let tokens = AuthTokens::new(
        secrets::required("FIBERSPIDER_API_TOKEN")?,
        secrets::required("FIBERSPIDER_ADMIN_TOKEN")?,
        secrets::required("FIBERSPIDER_INTERNAL_TOKEN")?,
    )?;
    let client_credentials: Arc<dyn CredentialProvider> =
        Arc::new(EnvCredentialProvider::from_env()?);

    let openstack = if secrets::optional("FIBERSPIDER_OPENSTACK_APP_CRED_ID").is_some() {
        Some(OpenStackConfig {
            auth_url: secrets::required("FIBERSPIDER_OPENSTACK_AUTH_URL")?,
            application_credential_id: secrets::required("FIBERSPIDER_OPENSTACK_APP_CRED_ID")?,
            application_credential_secret: secrets::required(
                "FIBERSPIDER_OPENSTACK_APP_CRED_SECRET",
            )?,
            compute_url: secrets::optional("FIBERSPIDER_OPENSTACK_COMPUTE_URL")
                .unwrap_or_else(|| "http://127.0.0.1/compute/v2.1".to_owned()),
            network_url: secrets::optional("FIBERSPIDER_OPENSTACK_NETWORK_URL")
                .unwrap_or_else(|| "http://127.0.0.1/networking/v2.0".to_owned()),
            image_url: secrets::optional("FIBERSPIDER_OPENSTACK_IMAGE_URL")
                .unwrap_or_else(|| "http://127.0.0.1/image/v2".to_owned()),
            probe_evidence_path: secrets::optional("FIBERSPIDER_OPENSTACK_PROBE_FILE")
                .unwrap_or_else(|| {
                    "/home/ragno/apps/photaxon/secrets/openstack-probes.json".to_owned()
                }),
        })
    } else {
        None
    };

    let config = AppConfig {
        server: ServerConfig { bind_addr },
        database,
        tokens,
        openstack,
        client_credentials,
    };
    validation::validate(&config, api_port)?;
    Ok(config)
}
