use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct CreateFieldRoe {
    pub roe_id: String,
    pub cabinet_id: String,
    pub splitter_id: String,
    pub street_name: String,
    pub street_number: String,
    pub city: String,
    pub postal_code: String,
    pub latitude: f64,
    pub longitude: f64,
    pub location_accuracy_m: f64,
    pub geolocation_source: String,
    pub feeder_cable_id: String,
    pub feeder_fiber_count: i32,
    pub fibers_allocated: i32,
    pub address_note: Option<String>,
    pub source_event: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct RegisterOnt {
    pub serial: String,
    pub gpon_port_id: String,
    pub roe_id: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub source_event: String,
}

#[derive(Clone, Debug, Deserialize, JsonSchema, Serialize)]
pub struct RegisterModem {
    pub serial: String,
    pub ont_id: String,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub source_event: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NearbyRoeCandidate {
    pub roe_id: String,
    pub street_name: String,
    pub street_number: String,
    pub city: String,
    pub cabinet_id: String,
    pub splitter_id: String,
    pub available_fibers: Option<i32>,
    pub distance_m: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum FeederFiberType {
    Fo12,
    Fo24,
    Fo48,
    Other(i32),
}

impl FeederFiberType {
    pub fn count(&self) -> i32 {
        match self {
            Self::Fo12 => 12,
            Self::Fo24 => 24,
            Self::Fo48 => 48,
            Self::Other(count) => *count,
        }
    }
}

pub fn available_feeder_fibers(total: i32, used: i32, reserved: i32) -> Option<i32> {
    if total <= 0 || used < 0 || reserved < 0 {
        return None;
    }
    total
        .checked_sub(used)?
        .checked_sub(reserved)
        .filter(|available| *available >= 0)
}

pub fn distance_meters(
    latitude_a: f64,
    longitude_a: f64,
    latitude_b: f64,
    longitude_b: f64,
) -> Option<f64> {
    if !(-90.0..=90.0).contains(&latitude_a)
        || !(-90.0..=90.0).contains(&latitude_b)
        || !(-180.0..=180.0).contains(&longitude_a)
        || !(-180.0..=180.0).contains(&longitude_b)
        || !latitude_a.is_finite()
        || !longitude_a.is_finite()
        || !latitude_b.is_finite()
        || !longitude_b.is_finite()
    {
        return None;
    }
    let earth_radius_m = 6_371_000.0;
    let latitude_delta = (latitude_b - latitude_a).to_radians();
    let longitude_delta = (longitude_b - longitude_a).to_radians();
    let haversine = (latitude_delta / 2.0).sin().powi(2)
        + latitude_a.to_radians().cos()
            * latitude_b.to_radians().cos()
            * (longitude_delta / 2.0).sin().powi(2);
    Some(2.0 * earth_radius_m * haversine.sqrt().atan2((1.0 - haversine).sqrt()))
}

pub fn validate_new_roe(input: &CreateFieldRoe) -> Result<(), &'static str> {
    if !valid_identifier(&input.roe_id, "ROE-")
        || !valid_identifier(&input.cabinet_id, "CAB-")
        || !valid_identifier(&input.splitter_id, "SPL-")
        || !valid_identifier(&input.feeder_cable_id, "CAV-")
        || input.source_event.trim().is_empty()
    {
        return Err("invalid ROE, cabinet, splitter, cable or source event identifier");
    }
    if input.street_name.trim().is_empty()
        || input.street_number.trim().is_empty()
        || input.city.trim().is_empty()
        || input.postal_code.trim().is_empty()
    {
        return Err("street, number, city and postal code are required");
    }
    if !input.latitude.is_finite()
        || !(-90.0..=90.0).contains(&input.latitude)
        || !input.longitude.is_finite()
        || !(-180.0..=180.0).contains(&input.longitude)
        || !input.location_accuracy_m.is_finite()
        || input.location_accuracy_m <= 0.0
        || !matches!(
            input.geolocation_source.as_str(),
            "TELEGRAM" | "CONTROL_PLANE_ADMIN" | "ADMIN" | "GPS" | "AUTHORIZED_IMPORT"
        )
    {
        return Err(
            "valid GPS coordinates, positive accuracy and an allowed geolocation source are required",
        );
    }
    if !matches!(input.feeder_fiber_count, 12 | 24 | 48)
        && !(input.feeder_fiber_count > 0 && input.feeder_fiber_count <= 512)
    {
        return Err("feeder cable must have 12, 24, 48 or a valid custom fibre count");
    }
    if input.fibers_allocated <= 0 || input.fibers_allocated > input.feeder_fiber_count {
        return Err("ROE feeder allocation must be positive and within cable capacity");
    }
    if input
        .address_note
        .as_deref()
        .is_some_and(|note| note.len() > 512 || note.chars().any(char::is_control))
    {
        return Err("address note is too long or contains control characters");
    }
    Ok(())
}

pub fn validate_serial(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 128
        || value.chars().any(char::is_control)
        || value.to_ascii_lowercase().contains("bearer ")
    {
        return Err("serial is invalid");
    }
    Ok(())
}

pub fn validate_gpon_port_id(value: &str) -> Result<(), &'static str> {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err("GPON port identifier is invalid");
    }
    Ok(())
}

pub fn validate_registration_metadata(
    vendor: Option<&str>,
    model: Option<&str>,
    source_event: &str,
) -> Result<(), &'static str> {
    if source_event.trim().is_empty() || source_event.len() > 128 {
        return Err("source event is required");
    }
    for value in [vendor, model].into_iter().flatten() {
        if value.len() > 128 || value.chars().any(char::is_control) {
            return Err("vendor/model is invalid");
        }
    }
    Ok(())
}

fn valid_identifier(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::{
        CreateFieldRoe, FeederFiberType, available_feeder_fibers, distance_meters,
        validate_new_roe, validate_registration_metadata, validate_serial,
    };

    fn roe() -> CreateFieldRoe {
        CreateFieldRoe {
            roe_id: "ROE-123".into(),
            cabinet_id: "CAB-A".into(),
            splitter_id: "SPL-A-01".into(),
            street_name: "Via Roma".into(),
            street_number: "24".into(),
            city: "Vetralla".into(),
            postal_code: "01019".into(),
            latitude: 42.321,
            longitude: 12.345,
            location_accuracy_m: 5.0,
            geolocation_source: "TELEGRAM".into(),
            feeder_cable_id: "CAV-A-017".into(),
            feeder_fiber_count: 12,
            fibers_allocated: 1,
            address_note: Some("chiostrina lato strada".into()),
            source_event: "FIELD-123".into(),
        }
    }

    #[test]
    fn validates_structured_roe_address_gps_and_feeder_allocation() {
        assert!(validate_new_roe(&roe()).is_ok());
        for source in ["CONTROL_PLANE_ADMIN", "ADMIN", "GPS", "AUTHORIZED_IMPORT"] {
            let mut input = roe();
            input.geolocation_source = source.into();
            assert!(validate_new_roe(&input).is_ok());
        }
        let mut bad = roe();
        bad.geolocation_source = "TEXT".into();
        assert!(validate_new_roe(&bad).is_err());
        let mut bad = roe();
        bad.fibers_allocated = 13;
        assert!(validate_new_roe(&bad).is_err());
    }

    #[test]
    fn feeder_type_and_available_capacity_are_bounded() {
        assert_eq!(FeederFiberType::Fo12.count(), 12);
        assert_eq!(FeederFiberType::Fo24.count(), 24);
        assert_eq!(FeederFiberType::Fo48.count(), 48);
        assert_eq!(FeederFiberType::Other(36).count(), 36);
        assert_eq!(available_feeder_fibers(12, 7, 1), Some(4));
        assert_eq!(available_feeder_fibers(12, 10, 3), None);
    }

    #[test]
    fn nearby_distance_uses_geographic_coordinates_and_rejects_invalid_values() {
        let distance = distance_meters(0.0, 0.0, 0.0, 1.0).unwrap();
        assert!((distance - 111_195.0).abs() < 100.0);
        assert_eq!(distance_meters(91.0, 0.0, 0.0, 0.0), None);
    }

    #[test]
    fn serial_and_registration_metadata_reject_control_content() {
        assert!(validate_serial("ONT-XYZ").is_ok());
        assert!(validate_serial(" ").is_err());
        assert!(validate_serial("bad\nserial").is_err());
        assert!(validate_registration_metadata(Some("Vendor"), Some("Model"), "FIELD-1").is_ok());
        assert!(validate_registration_metadata(None, None, "").is_err());
    }
}
