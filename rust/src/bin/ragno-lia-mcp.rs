use fiberspider_api::{config, db, lia_mcp::LiaMcp, mcp::auth::ControlPlaneAdminContext};
use std::{error::Error, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("repository root unavailable")?
        .to_path_buf();
    let database = config::load_database_from_file(&root.join("secrets/runtime.env"))?;
    let pool = db::pool::connect(&database).await?;
    let actor = if std::env::args().any(|argument| argument == "--private-tunnel-admin") {
        ControlPlaneAdminContext::from_authorized_tunnel_parent()?
    } else {
        ControlPlaneAdminContext::for_private_tunnel()
    };
    LiaMcp::new(root, pool, actor)?
        .serve_stdio()
        .await
}
