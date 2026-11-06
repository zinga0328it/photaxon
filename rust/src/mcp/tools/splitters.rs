use crate::{
    domain::splitter::{CreateSplitter, SplitterCapacity},
    mcp::registry::{ToolClass, class_for},
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    schemars::JsonSchema,
    tool, tool_router,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SplitterArgs {
    /// Stable splitter identifier.
    pub splitter_id: String,
}

#[tool_router(router = splitter_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(description = "READ: list persisted FTTH splitters and free port counts")]
    async fn list_splitters(&self) -> Result<String, McpError> {
        let rows: Vec<SplitterCapacity> = crate::db::splitters::list(&self.pool)
            .await
            .map_err(|_| McpError::internal_error("splitter repository unavailable", None))?;
        serde_json::to_string(&rows)
            .map_err(|_| McpError::internal_error("splitter response serialization failed", None))
    }

    #[tool(description = "READ: get one splitter capacity and free port count")]
    async fn get_splitter_capacity(
        &self,
        Parameters(args): Parameters<SplitterArgs>,
    ) -> Result<String, McpError> {
        let row = crate::db::splitters::get(&self.pool, &args.splitter_id)
            .await
            .map_err(|_| McpError::internal_error("splitter repository unavailable", None))?;
        serde_json::to_string(&row)
            .map_err(|_| McpError::internal_error("splitter response serialization failed", None))
    }

    #[tool(
        description = "WRITE_TOPOLOGY: create a splitter only through the topology service and audit"
    )]
    async fn create_splitter(
        &self,
        Parameters(input): Parameters<CreateSplitter>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("create_splitter") != Some(ToolClass::WriteTopology) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::splitter_service::create(&self.pool, &self.invocation, input).await {
            Ok(splitter_id) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({"status":"CREATED","splitter_id":splitter_id}).to_string(),
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}
