use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
pub struct CreateRoe {
    pub roe_id: String,
    pub cabinet_id: String,
    pub splitter_id: String,
    pub address_reference: String,
    pub upstream_resource: String,
    pub source_event: String,
    pub notes: Option<String>,
}
