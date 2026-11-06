use crate::domain::{anomaly::ReportAnomaly, ont::ReportOntEvent};

pub fn validate_anomaly(input: &ReportAnomaly) -> Result<(), &'static str> {
    if input.anomaly_id.trim().is_empty()
        || input.code.trim().is_empty()
        || input.source_event.trim().is_empty()
        || input.description.trim().is_empty()
        || input.description.len() > 512
    {
        return Err("anomaly fields are required and description must be at most 512 bytes");
    }
    let description = input.description.to_ascii_lowercase();
    if ["password=", "token=", "api_key=", "bearer ", "secret="]
        .iter()
        .any(|marker| description.contains(marker))
    {
        return Err("anomaly description contains a prohibited credential marker");
    }
    if !matches!(
        input.severity.as_str(),
        "LOW" | "MEDIUM" | "HIGH" | "CRITICAL"
    ) {
        return Err("unsupported anomaly severity");
    }
    Ok(())
}

pub fn validate_ont_event(input: &ReportOntEvent) -> Result<(), &'static str> {
    if !matches!(
        input.event_type.as_str(),
        "ONT_DETECTED" | "MODEM_DETECTED" | "PORT_STATUS_CHANGED"
    ) {
        return Err("unsupported lab ONT event type");
    }
    if input.agent_id.trim().is_empty() || input.ont_serial.trim().is_empty() {
        return Err("agent and ONT identifiers are required");
    }
    if let Some(port_id) = &input.port_id
        && (port_id.len() > 64
            || !port_id
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_')))
    {
        return Err("invalid port identifier");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_anomaly, validate_ont_event};
    use crate::domain::{anomaly::ReportAnomaly, ont::ReportOntEvent};

    #[test]
    fn anomaly_rejects_credential_markers_and_unknown_severity() {
        let valid = ReportAnomaly {
            anomaly_id: "ANOM-1".into(),
            code: "LOS_SIMULATED".into(),
            severity: "LOW".into(),
            description: "simulated signal loss".into(),
            correlation_id: None,
            source_event: "LAB-EVENT-1".into(),
        };
        assert!(validate_anomaly(&valid).is_ok());
        let secret = ReportAnomaly {
            description: "token=do-not-log".into(),
            ..valid.clone()
        };
        assert!(validate_anomaly(&secret).is_err());
        let severity = ReportAnomaly {
            severity: "ROOT".into(),
            ..valid
        };
        assert!(validate_anomaly(&severity).is_err());
    }

    #[test]
    fn ont_events_are_limited_to_lab_event_types() {
        let valid = ReportOntEvent {
            agent_id: "AGENT-CAB-A".into(),
            event_type: "ONT_DETECTED".into(),
            ont_serial: "FS-ONT-TEST-1".into(),
            port_id: None,
        };
        assert!(validate_ont_event(&valid).is_ok());
        let invalid = ReportOntEvent {
            event_type: "SYSTEM_COMMAND".into(),
            ..valid
        };
        assert!(validate_ont_event(&invalid).is_err());
    }
}
