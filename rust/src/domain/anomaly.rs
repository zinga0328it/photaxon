use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct ReportAnomaly {
    pub anomaly_id: String,
    pub code: String,
    pub severity: String,
    pub description: String,
    pub correlation_id: Option<String>,
    pub source_event: String,
}
