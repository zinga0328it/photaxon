use crate::{db, mcp::auth::InvocationContext};
use rmcp::{
    ServerHandler, ServiceExt, handler::server::router::tool::ToolRouter, model::ServerConfig,
    tool_handler, transport::stdio,
};
use sqlx::PgPool;
use std::error::Error;

#[derive(Clone)]
pub struct McpServer {
    pub pool: PgPool,
    pub invocation: InvocationContext,
    tool_router: ToolRouter<Self>,
}

impl McpServer {
    pub fn new(pool: PgPool, invocation: InvocationContext) -> Self {
        let mut tool_router = Self::cabinet_tools();
        tool_router.merge(Self::roe_tools());
        tool_router.merge(Self::pte_tools());
        tool_router.merge(Self::splitter_tools());
        tool_router.merge(Self::topology_tools());
        tool_router.merge(Self::anomaly_tools());
        tool_router.merge(Self::ont_tools());
        Self {
            pool,
            invocation,
            tool_router,
        }
    }

    pub fn registered_tool_names() -> Vec<String> {
        let mut router = Self::cabinet_tools();
        router.merge(Self::roe_tools());
        router.merge(Self::pte_tools());
        router.merge(Self::splitter_tools());
        router.merge(Self::topology_tools());
        router.merge(Self::anomaly_tools());
        router.merge(Self::ont_tools());
        router
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }

    pub async fn serve_stdio(self) -> Result<(), Box<dyn Error>> {
        let service = self.serve(stdio()).await?;
        service.waiting().await?;
        Ok(())
    }

    pub async fn ensure_bootstrap(&self) -> Result<(), sqlx::Error> {
        db::bootstrap::initialize(&self.pool).await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
    }
}
