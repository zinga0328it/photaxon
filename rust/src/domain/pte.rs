use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
pub struct CreatePte {
    pub pte_id: String,
    pub cabinet_id: String,
    pub line_id: String,
    pub address_reference: String,
    pub source_event: String,
    pub notes: Option<String>,
}
