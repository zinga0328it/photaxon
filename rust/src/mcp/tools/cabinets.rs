use crate::cabinet::model::Cabinet;
use crate::{
    domain::cabinet::CreateCabinet,
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
pub struct GetCabinetArgs {
    /// Stable cabinet identifier, for example CAB-A.
    pub cabinet_id: String,
}

#[tool_router(router = cabinet_tools, vis = "pub")]
impl super::super::server::McpServer {
    #[tool(description = "READ: list existing FiberSpider cabinets from PostgreSQL")]
    async fn list_cabinets(&self) -> Result<String, McpError> {
        let cabinets: Vec<Cabinet> = crate::db::cabinets::list(&self.pool)
            .await
            .map_err(|_| McpError::internal_error("cabinet repository unavailable", None))?;
        serde_json::to_string(&cabinets)
            .map_err(|_| McpError::internal_error("cabinet response serialization failed", None))
    }

    #[tool(description = "READ: get one existing FiberSpider cabinet")]
    async fn get_cabinet(
        &self,
        Parameters(args): Parameters<GetCabinetArgs>,
    ) -> Result<String, McpError> {
        let cabinet = crate::db::cabinets::get(&self.pool, &args.cabinet_id)
            .await
            .map_err(|_| McpError::internal_error("cabinet repository unavailable", None))?;
        serde_json::to_string(&cabinet)
            .map_err(|_| McpError::internal_error("cabinet response serialization failed", None))
    }

    #[tool(
        description = "WRITE_TOPOLOGY: validate and create one FTTH cabinet with line, splitter, ports, and agent; records audit"
    )]
    async fn create_cabinet(
        &self,
        Parameters(input): Parameters<CreateCabinet>,
    ) -> Result<CallToolResult, McpError> {
        if class_for("create_cabinet") != Some(ToolClass::WriteTopology) {
            return Ok(CallToolResult::error(vec![ContentBlock::text(
                "tool is not authorized",
            )]));
        }
        match crate::services::cabinet_service::create(&self.pool, &self.invocation, input).await {
            Ok(cabinet_id) => Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::json!({"status":"CREATED","cabinet_id":cabinet_id}).to_string(),
            )])),
            Err(error) => Ok(CallToolResult::error(vec![ContentBlock::text(error)])),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::mcp::server::McpServer;

    #[test]
    fn only_registered_read_tools_are_exposed() {
        let names = McpServer::registered_tool_names();
        assert!(names.contains(&"list_cabinets".to_owned()));
        assert!(names.contains(&"get_cabinet".to_owned()));
        assert!(names.contains(&"create_cabinet".to_owned()));
        assert!(names.contains(&"create_roe".to_owned()));
        assert!(names.contains(&"create_pte".to_owned()));
        assert!(names.contains(&"create_splitter".to_owned()));
        assert!(names.contains(&"get_topology".to_owned()));
        assert!(names.contains(&"update_physical_path".to_owned()));
        assert!(names.contains(&"report_anomaly".to_owned()));
        assert!(names.contains(&"report_ont_event".to_owned()));
        assert!(!names.contains(&"exec_shell".to_owned()));
        assert!(!names.contains(&"run_sql".to_owned()));
        assert!(!names.contains(&"systemctl".to_owned()));
        assert!(names.contains(&"get_roe".to_owned()));
        assert!(names.contains(&"get_pte".to_owned()));
        assert_eq!(names.len(), 18);
    }
}
