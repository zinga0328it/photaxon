mod env;
pub mod telegram;
pub mod validation;

use crate::credentials::provider::CredentialProvider;
use crate::security::tokens::AuthTokens;
use std::{
    net::{Ipv6Addr, SocketAddr},
    sync::Arc,
};

pub use env::{load, load_database_from_file};

pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub tokens: AuthTokens,
    pub openstack: Option<OpenStackConfig>,
    pub client_credentials: Arc<dyn CredentialProvider>,
}

pub struct OpenStackConfig {
    pub auth_url: String,
    pub application_credential_id: String,
    pub application_credential_secret: String,
    pub compute_url: String,
    pub network_url: String,
    pub image_url: String,
    pub probe_evidence_path: String,
}

pub struct ServerConfig {
    pub bind_addr: SocketAddr,
}

pub struct DatabaseConfig {
    pub host: Ipv6Addr,
    pub port: u16,
    pub name: String,
    pub user: String,
    pub password: String,
}
