use crate::{
    domain::anomaly::ReportAnomaly,
    mcp::registry::{ToolClass, class_for},
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};

#[tool_router(router = anomaly_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(description = "WRITE_SAFE: report a validated FTTH lab anomaly; records an audit event")]
    async fn report_anomaly(
        &self,
        Parameters(input): Parameters<ReportAnomaly>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("report_anomaly") != Some(ToolClass::WriteSafe) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::anomaly_service::report(&self.pool, &self.invocation, input).await {
            Ok(()) => Ok(CallToolResult::success(vec![ContentBlock::text(
                "ANOMALY_RECORDED",
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}
