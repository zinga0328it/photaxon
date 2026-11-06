use crate::{
    domain::pte::CreatePte,
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
pub struct PteArgs {
    pub pte_id: String,
}

#[tool_router(router = pte_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(description = "READ: list PTE resources from PostgreSQL")]
    async fn list_ptes(&self) -> Result<String, McpError> {
        let records = crate::db::ptes::list(&self.pool)
            .await
            .map_err(|_| McpError::internal_error("PTE repository unavailable", None))?;
        serde_json::to_string(&records)
            .map_err(|_| McpError::internal_error("PTE response serialization failed", None))
    }

    #[tool(description = "READ: get one PTE resource")]
    async fn get_pte(&self, Parameters(args): Parameters<PteArgs>) -> Result<String, McpError> {
        let pte = crate::db::ptes::get(&self.pool, &args.pte_id)
            .await
            .map_err(|_| McpError::internal_error("PTE repository unavailable", None))?;
        serde_json::to_string(&pte)
            .map_err(|_| McpError::internal_error("PTE response serialization failed", None))
    }

    #[tool(
        description = "WRITE_TOPOLOGY: validate and create one PTE attached to an existing cabinet and line; records audit"
    )]
    async fn create_pte(
        &self,
        Parameters(input): Parameters<CreatePte>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("create_pte") != Some(ToolClass::WriteTopology) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::pte_service::create(&self.pool, &self.invocation, input).await {
            Ok(pte_id) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({"status":"CREATED","pte_id":pte_id}).to_string(),
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}
