use fiberspider_api::{config, db, mcp};
use std::error::Error;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let config = config::load()?;
    let pool = db::pool::connect(&config.database).await?;
    let invocation = mcp::auth::InvocationContext::from_verified_activation(&pool).await?;
    let server = mcp::server::McpServer::new(pool, invocation);
    server.ensure_bootstrap().await?;
    server.serve_stdio().await
}
