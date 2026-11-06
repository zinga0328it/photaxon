use crate::{
    domain::roe::CreateRoe,
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
pub struct RoeArgs {
    pub roe_id: String,
}

#[tool_router(router = roe_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(description = "READ: list ROE resources from PostgreSQL")]
    async fn list_roes(&self) -> Result<String, McpError> {
        let records = crate::db::roes::list(&self.pool)
            .await
            .map_err(|_| McpError::internal_error("ROE repository unavailable", None))?;
        serde_json::to_string(&records)
            .map_err(|_| McpError::internal_error("ROE response serialization failed", None))
    }

    #[tool(description = "READ: get one ROE resource")]
    async fn get_roe(&self, Parameters(args): Parameters<RoeArgs>) -> Result<String, McpError> {
        let roe = crate::db::roes::get(&self.pool, &args.roe_id)
            .await
            .map_err(|_| McpError::internal_error("ROE repository unavailable", None))?;
        serde_json::to_string(&roe)
            .map_err(|_| McpError::internal_error("ROE response serialization failed", None))
    }

    #[tool(
        description = "WRITE_TOPOLOGY: validate and create one ROE attached to an existing cabinet and splitter; records audit"
    )]
    async fn create_roe(
        &self,
        Parameters(input): Parameters<CreateRoe>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("create_roe") != Some(ToolClass::WriteTopology) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::roe_service::create(&self.pool, &self.invocation, input).await {
            Ok(roe_id) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({"status":"CREATED","roe_id":roe_id}).to_string(),
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}
