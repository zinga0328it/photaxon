use crate::{
    domain::ont::ReportOntEvent,
    mcp::registry::{ToolClass, class_for},
};
use rmcp::{
    ErrorData as McpError,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock},
    tool, tool_router,
};

#[tool_router(router = ont_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(
        description = "WRITE_SAFE: report a simulated lab ONT/modem event; cannot change network configuration"
    )]
    async fn report_ont_event(
        &self,
        Parameters(input): Parameters<ReportOntEvent>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("report_ont_event") != Some(ToolClass::WriteSafe) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::event_service::report_ont(&self.pool, &self.invocation, input).await
        {
            Ok(()) => Ok(CallToolResult::success(vec![ContentBlock::text(
                "ONT_EVENT_RECORDED",
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}
