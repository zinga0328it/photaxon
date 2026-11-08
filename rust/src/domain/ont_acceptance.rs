use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OneStatus {
    OnePending,
    OnePass,
    OneFail,
    Error,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalPrecheckStatus {
    Pending,
    PrecheckPass,
    PrecheckFail,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct OpticalMeasurement {
    pub rx_dbm: f64,
    pub tx_dbm: Option<f64>,
    pub attenuation_db: Option<f64>,
    pub measured_at: String,
    pub instrument: String,
    pub measurement_point: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct OntTestRequest {
    pub test_id: String,
    pub work_order_id: String,
    pub ont_id: String,
    pub operator: String,
    pub test_type: String,
    pub technician_id: Option<String>,
    pub optical_profile_id: Option<String>,
    pub measurement: Option<OpticalMeasurement>,
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct OneTestResult {
    pub test_id: String,
    pub work_order_id: String,
    pub ont_id: String,
    pub ont_serial: String,
    pub operator: String,
    pub one_status: OneStatus,
    pub one_result_code: String,
    pub one_test_code: Option<String>,
    pub completed_at: String,
    pub failure_reason: Option<String>,
    pub notes: Option<String>,
    pub raw_reference: Option<String>,
    pub measurement: Option<OpticalMeasurement>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct OpticalPrecheckProfile {
    pub profile_id: String,
    pub operator: String,
    pub vendor: String,
    pub model: String,
    pub rx_min_dbm: f64,
    pub rx_max_dbm: f64,
}

pub fn precheck(rx: Option<f64>, bounds: Option<(f64, f64)>) -> LocalPrecheckStatus {
    match (rx, bounds) {
        (Some(rx), Some((min, max)))
            if rx.is_finite() && min.is_finite() && max.is_finite() && min <= max =>
        {
            if (min..=max).contains(&rx) {
                LocalPrecheckStatus::PrecheckPass
            } else {
                LocalPrecheckStatus::PrecheckFail
            }
        }
        _ => LocalPrecheckStatus::Pending,
    }
}

pub fn positive_closure_allowed(
    status: &OneStatus,
    code: Option<&str>,
) -> Result<(), &'static str> {
    if *status != OneStatus::OnePass {
        return Err("positive closure requires ONE_PASS");
    }
    validate_test_code(code.ok_or("ONE_PASS requires codice_collaudo")?)
}

pub fn validate_test_code(code: &str) -> Result<(), &'static str> {
    if code.is_empty()
        || code.len() > 128
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/'))
    {
        return Err("invalid ONE codice_collaudo");
    }
    Ok(())
}

pub fn validate_measurement(measurement: &OpticalMeasurement) -> Result<(), &'static str> {
    if !measurement.rx_dbm.is_finite()
        || measurement.tx_dbm.is_some_and(|dbm| !dbm.is_finite())
        || measurement
            .attenuation_db
            .is_some_and(|db| !db.is_finite() || db < 0.0)
    {
        return Err("invalid optical measurement");
    }
    time(&measurement.measured_at)?;
    text(&measurement.instrument, 256)?;
    text(&measurement.measurement_point, 256)
}

pub fn validate_request(input: &OntTestRequest) -> Result<(), &'static str> {
    for id in [&input.test_id, &input.work_order_id, &input.ont_id] {
        crate::domain::physical_asset::identifier(id)?;
    }
    if let Some(id) = &input.technician_id {
        crate::domain::physical_asset::identifier(id)?;
    }
    if let Some(id) = &input.optical_profile_id {
        crate::domain::physical_asset::identifier(id)?;
    }
    text(&input.operator, 256)?;
    text(&input.test_type, 128)?;
    if let Some(measurement) = &input.measurement {
        validate_measurement(measurement)?;
    }
    if let Some(notes) = &input.notes {
        text(notes, 2048)?;
    }
    Ok(())
}

pub fn validate_result(input: &OneTestResult) -> Result<(), &'static str> {
    for id in [&input.test_id, &input.work_order_id, &input.ont_id] {
        crate::domain::physical_asset::identifier(id)?;
    }
    crate::domain::field_ops::validate_serial(&input.ont_serial)?;
    text(&input.operator, 256)?;
    text(&input.one_result_code, 256)?;
    time(&input.completed_at)?;
    if input.one_status == OneStatus::OnePending {
        return Err("record a final real ONE result, not ONE_PENDING");
    }
    if input.one_status == OneStatus::OnePass {
        positive_closure_allowed(&input.one_status, input.one_test_code.as_deref())?;
    }
    if let Some(code) = &input.one_test_code {
        validate_test_code(code)?;
    }
    if matches!(input.one_status, OneStatus::OneFail | OneStatus::Error)
        && input
            .failure_reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    {
        return Err("ONE failure requires a reason");
    }
    for value in [&input.notes, &input.failure_reason, &input.raw_reference]
        .into_iter()
        .flatten()
    {
        text(value, 2048)?;
    }
    if let Some(measurement) = &input.measurement {
        validate_measurement(measurement)?;
    }
    Ok(())
}

pub fn validate_profile(input: &OpticalPrecheckProfile) -> Result<(), &'static str> {
    crate::domain::physical_asset::identifier(&input.profile_id)?;
    for field in [&input.operator, &input.vendor, &input.model] {
        text(field, 256)?;
    }
    if !input.rx_min_dbm.is_finite()
        || !input.rx_max_dbm.is_finite()
        || input.rx_min_dbm > input.rx_max_dbm
    {
        return Err("invalid RX precheck bounds");
    }
    Ok(())
}
fn time(value: &str) -> Result<(), &'static str> {
    chrono::DateTime::parse_from_rfc3339(value).map_err(|_| "test time must be RFC3339")?;
    Ok(())
}
fn text(value: &str, max: usize) -> Result<(), &'static str> {
    if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
        Err("invalid test text field")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_pass_requires_code_and_precheck_is_not_final() {
        assert!(positive_closure_allowed(&OneStatus::OnePass, Some("REAL-CODE-123")).is_ok());
        assert!(positive_closure_allowed(&OneStatus::OnePass, None).is_err());
        assert!(positive_closure_allowed(&OneStatus::OnePass, Some(" ")).is_err());
        assert!(positive_closure_allowed(&OneStatus::OneFail, Some("CODE-1")).is_err());
        assert_eq!(
            precheck(Some(-18.4), Some((-28.0, -8.0))),
            LocalPrecheckStatus::PrecheckPass
        );
        assert!(positive_closure_allowed(&OneStatus::OnePending, None).is_err());
        assert_eq!(precheck(Some(-18.4), None), LocalPrecheckStatus::Pending);
    }
}
