#![forbid(unsafe_code)]

use axum::serve;
use fiberspider_api::{
    api::{self, AppState},
    config, db,
    openstack::OpenStackClient,
};
use std::error::Error;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = config::load()?;
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let pool = db::pool::connect(&config.database).await?;
    db::bootstrap::initialize(&pool).await?;
    let listener = TcpListener::bind(config.server.bind_addr).await?;
    let openstack = config
        .openstack
        .as_ref()
        .map(OpenStackClient::new)
        .transpose()?;
    let app = api::router(AppState::new(
        pool,
        config.tokens,
        openstack,
        config.client_credentials,
    ));

    tracing::info!("FiberSpider Rust API listening on the configured Yggdrasil address");
    serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
