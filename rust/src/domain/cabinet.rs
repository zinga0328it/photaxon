use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
pub struct CreateCabinet {
    pub cabinet_id: String,
    pub display_name: String,
    pub location_reference: String,
    pub parent_node: String,
    pub line_id: String,
    pub network_name: String,
    pub splitter_id: String,
    pub splitter_ratio: i32,
    pub splitter_capacity: i32,
    pub source_event: String,
    pub notes: Option<String>,
}
