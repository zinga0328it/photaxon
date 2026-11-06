use serde::{Deserialize, Serialize};

use rmcp::schemars::JsonSchema;

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
pub struct CreateSplitter {
    pub splitter_id: String,
    pub cabinet_id: String,
    pub line_id: String,
    pub ratio: i32,
    pub capacity_total: i32,
    pub source_event: String,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct SplitterCapacity {
    pub splitter_id: String,
    pub cabinet_id: String,
    pub line_id: String,
    pub ratio: i32,
    pub capacity_total: i32,
    pub free_ports: i64,
    pub status: String,
}
