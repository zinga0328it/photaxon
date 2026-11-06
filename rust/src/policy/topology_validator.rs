use crate::domain::splitter::CreateSplitter;
use crate::domain::{cabinet::CreateCabinet, pte::CreatePte, roe::CreateRoe};

pub fn valid_identifier(value: &str, prefix: &str) -> bool {
    value.starts_with(prefix)
        && value.len() > prefix.len()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn safe_notes(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return true;
    };
    if value.len() > 1024 || value.chars().any(char::is_control) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    !["password=", "token=", "api_key=", "bearer ", "secret="]
        .iter()
        .any(|marker| lower.contains(marker))
}

pub fn validate_cabinet(input: &CreateCabinet) -> Result<(), &'static str> {
    if !valid_identifier(&input.cabinet_id, "CAB-")
        || !valid_identifier(&input.line_id, "FTTH-LINE-")
        || !valid_identifier(&input.splitter_id, "SPL-")
        || !valid_identifier(&input.network_name, "FS-")
        || !valid_identifier(&input.source_event, "FIELD-")
    {
        return Err("invalid cabinet, line, or splitter identifier");
    }
    if input.display_name.trim().is_empty()
        || input.location_reference.trim().is_empty()
        || input.source_event.trim().is_empty()
    {
        return Err("display name, location reference, and source event are required");
    }
    if !safe_notes(input.notes.as_deref()) {
        return Err(
            "notes are oversized, contain control characters, or include credential markers",
        );
    }
    if input.splitter_ratio <= 0
        || input.splitter_capacity < input.splitter_ratio
        || input.splitter_capacity > 256
    {
        return Err("splitter capacity is inconsistent or exceeds lab policy");
    }
    if input.parent_node != "CENTRAL-01" {
        return Err("new cabinet parent must be an explicitly known topology node");
    }
    Ok(())
}

pub fn validate_roe(input: &CreateRoe) -> Result<(), &'static str> {
    if !valid_identifier(&input.roe_id, "ROE-")
        || !valid_identifier(&input.cabinet_id, "CAB-")
        || !valid_identifier(&input.splitter_id, "SPL-")
        || input.address_reference.trim().is_empty()
        || input.upstream_resource.trim().is_empty()
        || input.source_event.trim().is_empty()
    {
        return Err("ROE identifiers, address, upstream resource, and source event are required");
    }
    if !safe_notes(input.notes.as_deref()) {
        return Err("notes contain prohibited or oversized content");
    }
    Ok(())
}

pub fn validate_pte(input: &CreatePte) -> Result<(), &'static str> {
    if !valid_identifier(&input.pte_id, "PTE-")
        || !valid_identifier(&input.cabinet_id, "CAB-")
        || !valid_identifier(&input.line_id, "FTTH-LINE-")
        || input.address_reference.trim().is_empty()
        || input.source_event.trim().is_empty()
    {
        return Err("PTE identifiers, address, and source event are required");
    }
    if !safe_notes(input.notes.as_deref()) {
        return Err("notes contain prohibited or oversized content");
    }
    Ok(())
}

pub fn validate_splitter(input: &CreateSplitter) -> Result<(), &'static str> {
    if !valid_identifier(&input.splitter_id, "SPL-")
        || !valid_identifier(&input.cabinet_id, "CAB-")
        || !valid_identifier(&input.line_id, "FTTH-LINE-")
        || input.source_event.trim().is_empty()
    {
        return Err("splitter identifiers and source event are required");
    }
    if input.ratio <= 0 || input.capacity_total < input.ratio || input.capacity_total > 256 {
        return Err("splitter capacity is inconsistent or exceeds lab policy");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_cabinet, validate_pte, validate_roe};
    use crate::domain::{cabinet::CreateCabinet, pte::CreatePte, roe::CreateRoe};

    #[test]
    fn accepts_valid_new_cabinet_and_rejects_bad_topology_or_capacity() {
        let valid = CreateCabinet {
            cabinet_id: "CAB-D".into(),
            display_name: "Cabinet D".into(),
            location_reference: "LAB-D".into(),
            parent_node: "CENTRAL-01".into(),
            line_id: "FTTH-LINE-D".into(),
            network_name: "FS-LINE-D".into(),
            splitter_id: "SPL-D-01".into(),
            splitter_ratio: 8,
            splitter_capacity: 8,
            source_event: "FIELD-001".into(),
            notes: None,
        };
        assert!(validate_cabinet(&valid).is_ok());
        let mut bad = valid;
        bad.parent_node = "MISSING-NODE".into();
        assert!(validate_cabinet(&bad).is_err());
    }

    #[test]
    fn rejects_invalid_roe_and_pte_inputs() {
        let roe = CreateRoe {
            roe_id: "ROE-D-1".into(),
            cabinet_id: "CAB-D".into(),
            splitter_id: "SPL-D-01".into(),
            address_reference: "LAB-D".into(),
            upstream_resource: "SPL-D-01".into(),
            source_event: "FIELD-001".into(),
            notes: None,
        };
        assert!(validate_roe(&roe).is_ok());
        let pte = CreatePte {
            pte_id: "PTE-D-1".into(),
            cabinet_id: "CAB-D".into(),
            line_id: "FTTH-LINE-D".into(),
            address_reference: "LAB-D".into(),
            source_event: "FIELD-001".into(),
            notes: None,
        };
        assert!(validate_pte(&pte).is_ok());
    }
}
