use crate::{
    domain::topology::TopologyPath,
    mcp::registry::{ToolClass, class_for},
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};

#[tool_router(router = topology_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(description = "READ: return the persisted cabinet, line, splitter, and agent topology")]
    async fn get_topology(&self) -> Result<String, McpError> {
        let rows = crate::db::topology::get_topology(&self.pool)
            .await
            .map_err(|_| McpError::internal_error("topology repository unavailable", None))?;
        serde_json::to_string(&rows)
            .map_err(|_| McpError::internal_error("topology serialization failed", None))
    }

    #[tool(
        description = "READ: validate that cabinet, line, and splitter form an active persisted path"
    )]
    async fn validate_path(
        &self,
        Parameters(path): Parameters<TopologyPath>,
    ) -> Result<CallToolResult, McpError> {
        match crate::services::topology_service::validate_path(&self.pool, &path).await {
            Ok(true) => Ok(CallToolResult::success(vec![ContentBlock::text(
                "PATH_VALID",
            )])),
            Ok(false) => Ok(CallToolResult::error(vec![ContentBlock::text(
                "PATH_REJECTED",
            )])),
            Err(_) => Err(McpError::internal_error(
                "topology validator unavailable",
                None,
            )),
        }
    }

    #[tool(description = "READ: list topology changes waiting for review")]
    async fn get_pending_topology_changes(&self) -> Result<String, McpError> {
        let changes = crate::db::topology::pending_changes(&self.pool)
            .await
            .map_err(|_| {
                McpError::internal_error("topology change repository unavailable", None)
            })?;
        serde_json::to_string(&changes)
            .map_err(|_| McpError::internal_error("topology response serialization failed", None))
    }

    #[tool(
        description = "WRITE_TOPOLOGY: propose a physical path change; this tool does not directly apply a path"
    )]
    async fn update_physical_path(
        &self,
        Parameters(path): Parameters<TopologyPath>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("update_physical_path") != Some(ToolClass::WriteTopology) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::topology_service::propose_path(&self.pool, &self.invocation, &path)
            .await
        {
            Ok(change_id) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({"status":"PENDING_REVIEW","change_id":change_id}).to_string(),
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}
