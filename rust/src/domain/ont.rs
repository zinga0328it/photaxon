use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ReportOntEvent {
    pub agent_id: String,
    pub event_type: String,
    pub ont_serial: String,
    pub port_id: Option<String>,
}
