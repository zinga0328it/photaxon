pub mod auth;
pub mod cabinets;
pub mod errors;
pub mod health;
pub mod lab;
mod routes;

use crate::credentials::provider::CredentialProvider;
use crate::openstack::OpenStackClient;
use crate::security::tokens::AuthTokens;
use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub tokens: AuthTokens,
    pub openstack: Option<OpenStackClient>,
    pub client_credentials: Arc<dyn CredentialProvider>,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        tokens: AuthTokens,
        openstack: Option<OpenStackClient>,
        client_credentials: Arc<dyn CredentialProvider>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            tokens,
            openstack,
            client_credentials,
        })
    }
}

pub use routes::router;
