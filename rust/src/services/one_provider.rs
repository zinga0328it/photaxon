use crate::domain::ont_acceptance::{OneTestResult, validate_result};

pub trait OneProvider {
    fn validate_recorded_result(&self, result: &OneTestResult) -> Result<(), &'static str>;
    fn mode(&self) -> &'static str;
}

pub struct ManualOneProvider;

impl OneProvider for ManualOneProvider {
    fn validate_recorded_result(&self, result: &OneTestResult) -> Result<(), &'static str> {
        validate_result(result)
    }
    fn mode(&self) -> &'static str {
        "MANUAL_RECORDED"
    }
}
