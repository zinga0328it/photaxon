use crate::domain::field_ops::NearbyRoeCandidate;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkflowKind {
    CreateRoe,
    RegisterOnt,
    RegisterModem,
    ReplaceModem,
    UnlinkModem,
    SearchAsset,
    NearbyRoe,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkflowStep {
    EnterWorkOrderId,
    SelectCabinet,
    SelectSplitter,
    EnterRoeId,
    EnterStreetName,
    EnterStreetNumber,
    EnterCity,
    EnterPostalCode,
    EnterAddressNote,
    SelectCableType,
    EnterFiberCount,
    EnterCableId,
    RequestLocation,
    RequestRoePhoto,
    SelectRoe,
    RequestOntPhoto,
    SelectOnt,
    SelectCurrentModem,
    SelectNearbyRoe,
    VerifyCabinetAgent,
    EnterSerial,
    EnterVendor,
    EnterModel,
    RequestModemPhoto,
    EnterUnlinkReason,
    EnterSearchQuery,
    RequestNearbyLocation,
    Review,
    Done,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum WorkflowStatus {
    Draft,
    Review,
    Confirmed,
    Cancelled,
    Expired,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WorkflowDraft {
    pub workflow_id: String,
    pub kind: WorkflowKind,
    pub step: WorkflowStep,
    pub status: WorkflowStatus,
    pub version: i32,
    pub fields: Map<String, Value>,
}

impl WorkflowDraft {
    pub fn new(workflow_id: impl Into<String>, kind: WorkflowKind) -> Self {
        let step = match kind {
            WorkflowKind::CreateRoe
            | WorkflowKind::RegisterOnt
            | WorkflowKind::RegisterModem
            | WorkflowKind::ReplaceModem
            | WorkflowKind::UnlinkModem
            | WorkflowKind::NearbyRoe => WorkflowStep::EnterWorkOrderId,
            WorkflowKind::SearchAsset => WorkflowStep::EnterSearchQuery,
        };
        Self {
            workflow_id: workflow_id.into(),
            kind,
            step,
            status: WorkflowStatus::Draft,
            version: 1,
            fields: Map::new(),
        }
    }

    pub fn accept_text(&mut self, input: &str) -> Result<WorkflowStep, &'static str> {
        if self.status != WorkflowStatus::Draft {
            return Err("workflow is not accepting input");
        }
        let value = input.trim();
        if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
            return Err("input is empty, oversized, or contains control characters");
        }

        let (field, next, normalized) = match self.step {
            WorkflowStep::EnterWorkOrderId => {
                let next = match self.kind {
                    WorkflowKind::CreateRoe => WorkflowStep::SelectCabinet,
                    WorkflowKind::RegisterOnt => WorkflowStep::SelectRoe,
                    WorkflowKind::RegisterModem
                    | WorkflowKind::ReplaceModem
                    | WorkflowKind::UnlinkModem => WorkflowStep::SelectOnt,
                    WorkflowKind::NearbyRoe => WorkflowStep::RequestNearbyLocation,
                    WorkflowKind::SearchAsset => WorkflowStep::EnterSearchQuery,
                };
                ("work_order_id", next, value.to_owned())
            }
            WorkflowStep::SelectCabinet => {
                ("cabinet_id", WorkflowStep::SelectSplitter, value.to_owned())
            }
            WorkflowStep::SelectSplitter => {
                ("splitter_id", WorkflowStep::EnterRoeId, value.to_owned())
            }
            WorkflowStep::EnterRoeId => ("roe_id", WorkflowStep::EnterStreetName, value.to_owned()),
            WorkflowStep::EnterStreetName => (
                "street_name",
                WorkflowStep::EnterStreetNumber,
                value.to_owned(),
            ),
            WorkflowStep::EnterStreetNumber => {
                ("street_number", WorkflowStep::EnterCity, value.to_owned())
            }
            WorkflowStep::EnterCity => ("city", WorkflowStep::EnterPostalCode, value.to_owned()),
            WorkflowStep::EnterPostalCode => (
                "postal_code",
                WorkflowStep::EnterAddressNote,
                value.to_owned(),
            ),
            WorkflowStep::EnterAddressNote => (
                "address_note",
                WorkflowStep::EnterCableId,
                optional_value(value),
            ),
            WorkflowStep::SelectCableType => match value.to_ascii_uppercase().as_str() {
                "12 FO" | "12" => (
                    "feeder_fiber_count",
                    WorkflowStep::RequestLocation,
                    "12".to_owned(),
                ),
                "24 FO" | "24" => (
                    "feeder_fiber_count",
                    WorkflowStep::RequestLocation,
                    "24".to_owned(),
                ),
                "48 FO" | "48" => (
                    "feeder_fiber_count",
                    WorkflowStep::RequestLocation,
                    "48".to_owned(),
                ),
                "ALTRO" => ("_next", WorkflowStep::EnterFiberCount, String::new()),
                _ => return Err("choose 12 FO, 24 FO, 48 FO, or Altro"),
            },
            WorkflowStep::EnterFiberCount => {
                let count = value
                    .parse::<i32>()
                    .map_err(|_| "fibre count must be numeric")?;
                if !(1..=512).contains(&count) {
                    return Err("fibre count must be between 1 and 512");
                }
                (
                    "feeder_fiber_count",
                    WorkflowStep::RequestLocation,
                    count.to_string(),
                )
            }
            WorkflowStep::EnterCableId => (
                "feeder_cable_id",
                WorkflowStep::SelectCableType,
                value.to_owned(),
            ),
            WorkflowStep::SelectRoe => {
                let next = if self.kind == WorkflowKind::RegisterOnt {
                    WorkflowStep::RequestOntPhoto
                } else {
                    WorkflowStep::EnterSerial
                };
                ("roe_id", next, value.to_owned())
            }
            WorkflowStep::SelectOnt => {
                let next = if matches!(
                    self.kind,
                    WorkflowKind::UnlinkModem | WorkflowKind::ReplaceModem
                ) {
                    WorkflowStep::SelectCurrentModem
                } else {
                    WorkflowStep::EnterSerial
                };
                ("ont_id", next, value.to_owned())
            }
            WorkflowStep::SelectCurrentModem => {
                let next = if self.kind == WorkflowKind::UnlinkModem {
                    WorkflowStep::EnterUnlinkReason
                } else {
                    WorkflowStep::EnterSerial
                };
                ("current_modem_id", next, value.to_owned())
            }
            WorkflowStep::SelectNearbyRoe => (
                "candidate_roe_id",
                WorkflowStep::VerifyCabinetAgent,
                value.to_owned(),
            ),
            WorkflowStep::EnterSerial => ("serial", WorkflowStep::EnterVendor, value.to_owned()),
            WorkflowStep::EnterVendor => {
                let vendor = optional_value(value);
                ("vendor", WorkflowStep::EnterModel, vendor)
            }
            WorkflowStep::EnterModel => {
                let model = optional_value(value);
                let next = if matches!(
                    self.kind,
                    WorkflowKind::RegisterModem | WorkflowKind::ReplaceModem
                ) {
                    WorkflowStep::RequestModemPhoto
                } else {
                    WorkflowStep::Review
                };
                ("model", next, model)
            }
            WorkflowStep::EnterUnlinkReason => {
                if value.len() > 512 {
                    return Err("unlink reason is too long");
                }
                ("reason", WorkflowStep::Review, value.to_owned())
            }
            WorkflowStep::EnterSearchQuery => {
                ("search_query", WorkflowStep::Review, value.to_owned())
            }
            WorkflowStep::RequestOntPhoto => {
                return Err("send an ONT label photo for Codex extraction");
            }
            WorkflowStep::RequestLocation
            | WorkflowStep::RequestRoePhoto
            | WorkflowStep::RequestModemPhoto
            | WorkflowStep::RequestNearbyLocation
            | WorkflowStep::VerifyCabinetAgent
            | WorkflowStep::Review
            | WorkflowStep::Done
            | WorkflowStep::Cancelled => {
                return Err("this workflow step requires a location or explicit action");
            }
        };
        if field != "_next" {
            self.fields
                .insert(field.to_owned(), Value::String(normalized));
        }
        if self.kind == WorkflowKind::CreateRoe && next == WorkflowStep::RequestLocation {
            self.fields
                .insert("fibers_allocated".into(), Value::String("1".into()));
        }
        self.step = next;
        self.version += 1;
        if self.step == WorkflowStep::Review {
            self.status = WorkflowStatus::Review;
        }
        Ok(self.step)
    }

    pub fn accept_location(
        &mut self,
        latitude: f64,
        longitude: f64,
        accuracy_m: Option<f64>,
    ) -> Result<WorkflowStep, &'static str> {
        let is_roe_location =
            self.kind == WorkflowKind::CreateRoe && self.step == WorkflowStep::RequestLocation;
        let is_technician_location = self.kind == WorkflowKind::NearbyRoe
            && self.step == WorkflowStep::RequestNearbyLocation;
        if !is_roe_location && !is_technician_location {
            return Err("location is not expected in this workflow step");
        }
        if !latitude.is_finite()
            || !(-90.0..=90.0).contains(&latitude)
            || !longitude.is_finite()
            || !(-180.0..=180.0).contains(&longitude)
        {
            return Err("latitude or longitude is invalid");
        }
        let accuracy = accuracy_m.ok_or("Telegram location accuracy is required")?;
        if !accuracy.is_finite() || accuracy <= 0.0 {
            return Err("location accuracy must be positive and finite");
        }
        if is_roe_location {
            self.fields
                .insert("latitude".into(), json_number(latitude)?);
            self.fields
                .insert("longitude".into(), json_number(longitude)?);
            self.fields
                .insert("location_accuracy_m".into(), json_number(accuracy)?);
            self.fields.insert(
                "geolocation_source".into(),
                Value::String("TELEGRAM".into()),
            );
            self.step = WorkflowStep::RequestRoePhoto;
        } else {
            self.fields
                .insert("technician_latitude".into(), json_number(latitude)?);
            self.fields
                .insert("technician_longitude".into(), json_number(longitude)?);
            self.fields.insert(
                "technician_location_accuracy_m".into(),
                json_number(accuracy)?,
            );
            self.step = WorkflowStep::SelectNearbyRoe;
        }
        self.version += 1;
        Ok(self.step)
    }

    pub fn accept_media(&mut self, media_id: &str) -> Result<WorkflowStep, &'static str> {
        if media_id.trim().is_empty() || media_id.len() > 64 {
            return Err("media id is invalid");
        }
        let field = match (self.kind, self.step) {
            (WorkflowKind::CreateRoe, WorkflowStep::RequestRoePhoto) => "roe_photo_media_id",
            (
                WorkflowKind::RegisterModem | WorkflowKind::ReplaceModem,
                WorkflowStep::RequestModemPhoto,
            ) => "modem_photo_media_id",
            _ => return Err("photo is not expected in this workflow step"),
        };
        self.fields
            .insert(field.into(), Value::String(media_id.into()));
        self.step = WorkflowStep::Review;
        self.status = WorkflowStatus::Review;
        self.version += 1;
        Ok(self.step)
    }

    pub fn accept_ont_label(
        &mut self,
        serial: &str,
        gpon_port_id: &str,
        confidence_milli: u16,
    ) -> Result<WorkflowStep, &'static str> {
        if self.kind != WorkflowKind::RegisterOnt || self.step != WorkflowStep::RequestOntPhoto {
            return Err("ONT label photo is not expected in this workflow step");
        }
        crate::domain::field_ops::validate_serial(serial)?;
        crate::domain::field_ops::validate_gpon_port_id(gpon_port_id)?;
        if !(850..=1000).contains(&confidence_milli) {
            return Err("ONT label confidence is too low");
        }
        self.fields
            .insert("serial".into(), Value::String(serial.trim().to_owned()));
        self.fields.insert(
            "gpon_port_id".into(),
            Value::String(gpon_port_id.trim().to_owned()),
        );
        self.fields.insert(
            "ont_label_confidence_milli".into(),
            Value::Number(confidence_milli.into()),
        );
        self.step = WorkflowStep::EnterVendor;
        self.version += 1;
        Ok(self.step)
    }

    pub fn set_nearby_candidates(
        &mut self,
        candidates: &[NearbyRoeCandidate],
    ) -> Result<(), &'static str> {
        if self.kind != WorkflowKind::NearbyRoe || self.step != WorkflowStep::SelectNearbyRoe {
            return Err("nearby candidates are not expected in this workflow step");
        }
        let candidate_snapshot =
            serde_json::to_value(candidates).map_err(|_| "candidate serialization failed")?;
        let inventory_fingerprint =
            crate::llm::cabinet_agent::snapshot_fingerprint(&candidate_snapshot)
                .map_err(|_| "candidate fingerprint failed")?;
        self.fields
            .insert("nearby_candidates".into(), candidate_snapshot);
        self.fields.insert(
            "nearby_inventory_fingerprint".into(),
            Value::String(inventory_fingerprint),
        );
        Ok(())
    }

    pub fn record_cabinet_decision(
        &mut self,
        proposal_id: &str,
        decision: Value,
    ) -> Result<(), &'static str> {
        if self.kind != WorkflowKind::NearbyRoe || self.step != WorkflowStep::VerifyCabinetAgent {
            return Err("cabinet decision is not expected in this workflow step");
        }
        let decision_name = decision
            .get("decision")
            .and_then(Value::as_str)
            .ok_or("cabinet decision is invalid")?
            .to_owned();
        self.fields.insert(
            "roe_proposal_id".into(),
            Value::String(proposal_id.to_owned()),
        );
        self.fields.insert("codex_decision".into(), decision);
        match decision_name.as_str() {
            "ALLOW" | "ALLOW_WITH_WARNING" => {
                self.step = WorkflowStep::Review;
                self.status = WorkflowStatus::Review;
            }
            "DENY" | "NEEDS_SUPERVISOR" => {
                self.step = WorkflowStep::Done;
                self.status = WorkflowStatus::Failed;
            }
            _ => return Err("cabinet decision is invalid"),
        }
        self.version += 1;
        Ok(())
    }

    pub fn confirm(&mut self) -> Result<(), &'static str> {
        if self.status != WorkflowStatus::Review || self.step != WorkflowStep::Review {
            return Err("only a reviewed workflow can be confirmed");
        }
        self.status = WorkflowStatus::Confirmed;
        self.step = WorkflowStep::Done;
        self.version += 1;
        Ok(())
    }

    pub fn cancel(&mut self) -> Result<(), &'static str> {
        if matches!(
            self.status,
            WorkflowStatus::Confirmed | WorkflowStatus::Cancelled
        ) {
            return Err("workflow is already closed");
        }
        self.status = WorkflowStatus::Cancelled;
        self.step = WorkflowStep::Cancelled;
        self.version += 1;
        Ok(())
    }

    pub fn home(&mut self) -> Result<(), &'static str> {
        if self.status == WorkflowStatus::Draft || self.status == WorkflowStatus::Review {
            self.cancel()?;
        }
        Ok(())
    }
}

fn optional_value(value: &str) -> String {
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "nessuno" | "skip" | "-"
    ) {
        String::new()
    } else {
        value.to_owned()
    }
}

fn json_number(value: f64) -> Result<Value, &'static str> {
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .ok_or("coordinate is not a valid JSON number")
}

#[cfg(test)]
mod tests {
    use super::{WorkflowDraft, WorkflowKind, WorkflowStatus, WorkflowStep};

    #[test]
    fn ont_photo_flow_resumes_with_serial_and_port_preserved() {
        let mut draft = WorkflowDraft::new("wf-ont-photo", WorkflowKind::RegisterOnt);
        draft.accept_text("WR-ONT").unwrap();
        draft.accept_text("ROE-A").unwrap();
        assert_eq!(draft.step, WorkflowStep::RequestOntPhoto);
        assert!(draft.accept_text("manual-serial").is_err());
        assert!(draft.accept_ont_label("ONT-1", "PORT-A", 400).is_err());
        draft.accept_ont_label("ONT-1", "PORT-A", 950).unwrap();
        let payload = serde_json::to_vec(&draft).unwrap();
        let mut resumed: WorkflowDraft = serde_json::from_slice(&payload).unwrap();
        assert_eq!(resumed.version, draft.version);
        assert_eq!(resumed.fields["gpon_port_id"], "PORT-A");
        resumed.accept_text("Vendor").unwrap();
        resumed.accept_text("Model").unwrap();
        assert_eq!(resumed.status, WorkflowStatus::Review);
        resumed.confirm().unwrap();
    }

    #[test]
    fn roe_flow_requires_structured_steps_and_native_location_before_review() {
        let mut draft = WorkflowDraft::new("wf-1", WorkflowKind::CreateRoe);
        assert_eq!(draft.step, WorkflowStep::EnterWorkOrderId);
        for (input, expected) in [
            ("WR-1", WorkflowStep::SelectCabinet),
            ("CAB-A", WorkflowStep::SelectSplitter),
            ("SPL-A-01", WorkflowStep::EnterRoeId),
            ("ROE-123", WorkflowStep::EnterStreetName),
            ("Via Roma", WorkflowStep::EnterStreetNumber),
            ("24", WorkflowStep::EnterCity),
            ("Vetralla", WorkflowStep::EnterPostalCode),
            ("01019", WorkflowStep::EnterAddressNote),
            ("chiostrina lato strada", WorkflowStep::EnterCableId),
            ("CAV-A-017", WorkflowStep::SelectCableType),
            ("12 FO", WorkflowStep::RequestLocation),
        ] {
            assert_eq!(draft.accept_text(input).unwrap(), expected);
        }
        assert!(draft.accept_location(42.0, 12.0, Some(4.0)).is_ok());
        assert_eq!(draft.step, WorkflowStep::RequestRoePhoto);
        assert!(draft.confirm().is_err());
        draft.accept_media("photo-1").unwrap();
        assert_eq!(draft.status, WorkflowStatus::Review);
        assert!(draft.confirm().is_ok());
        assert_eq!(draft.status, WorkflowStatus::Confirmed);
    }

    #[test]
    fn other_cable_type_requires_an_explicit_positive_count() {
        let mut draft = WorkflowDraft::new("wf-2", WorkflowKind::CreateRoe);
        for input in [
            "WR-2", "CAB-A", "SPL-A-01", "ROE-2", "Via A", "1", "Town", "00100", "nessuno",
            "CAV-A-2",
        ] {
            draft.accept_text(input).unwrap();
        }
        assert_eq!(
            draft.accept_text("Altro").unwrap(),
            WorkflowStep::EnterFiberCount
        );
        assert!(draft.accept_text("0").is_err());
        assert_eq!(
            draft.accept_text("36").unwrap(),
            WorkflowStep::RequestLocation
        );
    }

    #[test]
    fn home_cancels_the_draft_and_confirm_is_separate() {
        let mut draft = WorkflowDraft::new("wf-3", WorkflowKind::RegisterOnt);
        draft.accept_text("WR-3").unwrap();
        draft.accept_text("ROE-1").unwrap();
        draft.home().unwrap();
        assert_eq!(draft.status, WorkflowStatus::Cancelled);
        assert!(draft.confirm().is_err());
    }

    #[test]
    fn invalid_or_missing_location_cannot_be_confirmed() {
        let mut draft = WorkflowDraft::new("wf-4", WorkflowKind::CreateRoe);
        for input in [
            "WR-4", "CAB-A", "SPL-A-01", "ROE-4", "Via A", "1", "Town", "00100", "-", "CAV-A",
            "24 FO",
        ] {
            draft.accept_text(input).unwrap();
        }
        assert!(draft.accept_location(100.0, 12.0, Some(2.0)).is_err());
        assert!(draft.accept_location(42.0, 12.0, None).is_err());
        assert!(draft.confirm().is_err());
    }

    #[test]
    fn nearby_requires_location_candidate_and_structured_allow_before_review() {
        let mut draft = WorkflowDraft::new("wf-nearby", WorkflowKind::NearbyRoe);
        draft.accept_text("WR-5").unwrap();
        assert_eq!(draft.step, WorkflowStep::RequestNearbyLocation);
        assert_eq!(
            draft.accept_location(42.0, 12.0, Some(5.0)).unwrap(),
            WorkflowStep::SelectNearbyRoe
        );
        draft
            .accept_text("ROE-NEAR")
            .expect("candidate selection should reach the agent gate");
        assert_eq!(draft.step, WorkflowStep::VerifyCabinetAgent);
        assert_eq!(draft.status, WorkflowStatus::Draft);
        draft
            .record_cabinet_decision(
                "proposal-1",
                serde_json::json!({"decision":"ALLOW","risk":"LOW"}),
            )
            .unwrap();
        assert_eq!(draft.status, WorkflowStatus::Review);
        assert!(draft.confirm().is_ok());
    }

    #[test]
    fn nearby_deny_never_reaches_technician_confirmation() {
        for decision in ["DENY", "NEEDS_SUPERVISOR"] {
            let mut draft = WorkflowDraft::new("wf-nearby-deny", WorkflowKind::NearbyRoe);
            draft.accept_text("WR-6").unwrap();
            draft.accept_location(42.0, 12.0, Some(5.0)).unwrap();
            draft.accept_text("ROE-NEAR").unwrap();
            draft
                .record_cabinet_decision(
                    "proposal-2",
                    serde_json::json!({"decision":decision,"risk":"HIGH"}),
                )
                .unwrap();
            assert_eq!(draft.status, WorkflowStatus::Failed);
            assert!(draft.confirm().is_err());
        }
    }
}
