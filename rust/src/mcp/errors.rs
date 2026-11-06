use rmcp::model::{CallToolResult, ContentBlock};

pub fn tool_error(message: &'static str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message)])
}
