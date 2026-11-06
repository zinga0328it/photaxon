use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct TopologyPath {
    pub cabinet_id: String,
    pub line_id: String,
    pub splitter_id: String,
    pub roe_id: Option<String>,
    pub pte_id: Option<String>,
    pub source_event: Option<String>,
}
