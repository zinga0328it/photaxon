use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{
    env,
    error::Error,
    ffi::OsString,
    fmt,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const ROE_HAS_LLM: bool = false;
pub const ONT_HAS_LLM: bool = false;
pub const MODEM_HAS_LLM: bool = false;
pub const SPLITTER_HAS_LLM: bool = false;
pub const CABINET_HAS_LLM: bool = true;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CodexDecision {
    Allow,
    AllowWithWarning,
    Deny,
    NeedsSupervisor,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionRisk {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetDecision {
    pub decision: CodexDecision,
    pub agent_id: String,
    pub cabinet_id: String,
    pub current_roe_id: Option<String>,
    pub candidate_roe_id: String,
    pub inventory_fingerprint: String,
    pub reason_code: String,
    pub risk: DecisionRisk,
}

#[derive(Clone, Debug, Serialize)]
pub struct RoeSelectionRequest {
    pub cabinet_id: String,
    pub current_roe_id: Option<String>,
    pub candidate_roe_id: String,
    pub candidate_splitter_id: String,
    pub distance_m: f64,
    pub available_fibers: Option<i32>,
    pub required_fibers: i32,
    pub inventory_fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OntLabel {
    pub serial: String,
    pub gpon_port_id: String,
    pub confidence_milli: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PortConflictAlert {
    pub decision: CodexDecision,
    pub agent_id: String,
    pub cabinet_id: String,
    pub gpon_port_id: String,
    pub reason_code: String,
    pub alert: String,
}

#[derive(Debug)]
pub enum CabinetAgentError {
    InvalidRequest,
    AgentUnavailable,
    WorkerUnavailable,
    InvalidResponse,
}

impl fmt::Display for CabinetAgentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidRequest => "ROE selection request is invalid",
            Self::AgentUnavailable => "selected cabinet agent is unavailable",
            Self::WorkerUnavailable => "cabinet agent worker is unavailable",
            Self::InvalidResponse => "cabinet agent returned an invalid decision",
        })
    }
}

impl Error for CabinetAgentError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerResponse {
    success: bool,
    decision: Option<CabinetDecision>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerOntLabelResponse {
    success: bool,
    ont_label: Option<OntLabelPayload>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerPortConflictResponse {
    success: bool,
    port_conflict: Option<PortConflictAlert>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OntLabelPayload {
    serial: String,
    gpon_port_id: String,
    confidence: f64,
}

#[derive(Serialize)]
struct WorkerRequest<'a> {
    operation: &'static str,
    agent_id: &'a str,
    cabinet_id: &'a str,
    context: Value,
}

pub fn snapshot_fingerprint(snapshot: &Value) -> Result<String, CabinetAgentError> {
    let bytes = serde_json::to_vec(snapshot).map_err(|_| CabinetAgentError::InvalidRequest)?;
    let digest = Sha256::digest(bytes);
    Ok(encode_hex(&digest))
}

pub async fn decide_roe_selection(
    pool: &PgPool,
    request: RoeSelectionRequest,
) -> Result<CabinetDecision, CabinetAgentError> {
    if request.cabinet_id.trim().is_empty()
        || request.candidate_roe_id.trim().is_empty()
        || request.candidate_splitter_id.trim().is_empty()
        || !request.distance_m.is_finite()
        || request.distance_m < 0.0
        || request.required_fibers <= 0
        || request.available_fibers.unwrap_or_default() < request.required_fibers
        || request.inventory_fingerprint.len() != 64
        || !request
            .inventory_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(CabinetAgentError::InvalidRequest);
    }
    let agent_id = select_agent(pool, &request.cabinet_id).await?;
    let expected = request.clone();
    let request_json = WorkerRequest {
        operation: "ROE_SELECTION_DECISION",
        agent_id: &agent_id,
        cabinet_id: &request.cabinet_id,
        context: json!({
            "candidate_cabinet_id": request.cabinet_id,
            "current_roe_id": request.current_roe_id,
            "candidate_roe_id": request.candidate_roe_id,
            "candidate_splitter_id": request.candidate_splitter_id,
            "distance_m": request.distance_m,
            "available_fibers": request.available_fibers,
            "required_fibers": request.required_fibers,
            "inventory_fingerprint": request.inventory_fingerprint,
        }),
    };
    let input = serde_json::to_vec(&request_json).map_err(|_| CabinetAgentError::InvalidRequest)?;
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(CabinetAgentError::WorkerUnavailable)?
        .to_path_buf();
    let worker_output = tokio::task::spawn_blocking(move || run_worker(&project_root, &input))
        .await
        .map_err(|_| CabinetAgentError::WorkerUnavailable)??;
    parse_worker_response(&worker_output, &agent_id, &expected)
}

pub async fn extract_ont_label(
    pool: &PgPool,
    roe_id: &str,
    image_path: &Path,
) -> Result<OntLabel, CabinetAgentError> {
    let cabinet_id = sqlx::query_scalar::<_, String>(
        "SELECT cabinet_id FROM roes WHERE roe_id=$1 AND status='ACTIVE'",
    )
    .bind(roe_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| CabinetAgentError::AgentUnavailable)?
    .ok_or(CabinetAgentError::InvalidRequest)?;
    let agent_id = select_agent(pool, &cabinet_id).await?;
    let request = WorkerRequest {
        operation: "EXTRACT_ONT_LABEL",
        agent_id: &agent_id,
        cabinet_id: &cabinet_id,
        context: json!({
            "cabinet_id": cabinet_id,
            "roe_id": roe_id,
            "image_path": image_path.to_string_lossy(),
        }),
    };
    let input = serde_json::to_vec(&request).map_err(|_| CabinetAgentError::InvalidRequest)?;
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(CabinetAgentError::WorkerUnavailable)?
        .to_path_buf();
    let worker_output = tokio::task::spawn_blocking(move || run_worker(&project_root, &input))
        .await
        .map_err(|_| CabinetAgentError::WorkerUnavailable)??;
    let response: WorkerOntLabelResponse =
        serde_json::from_str(&worker_output).map_err(|_| CabinetAgentError::InvalidResponse)?;
    if !response.success {
        let _ = response.error;
        return Err(CabinetAgentError::InvalidResponse);
    }
    let payload = response
        .ont_label
        .ok_or(CabinetAgentError::InvalidResponse)?;
    if payload.serial.trim().is_empty()
        || payload.serial.len() > 128
        || payload.serial.chars().any(char::is_control)
        || payload.gpon_port_id.trim().is_empty()
        || payload.gpon_port_id.len() > 64
        || !payload
            .gpon_port_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || !payload.confidence.is_finite()
        || !(0.85..=1.0).contains(&payload.confidence)
    {
        return Err(CabinetAgentError::InvalidResponse);
    }
    Ok(OntLabel {
        serial: payload.serial,
        gpon_port_id: payload.gpon_port_id,
        confidence_milli: (payload.confidence * 1000.0).round() as u16,
    })
}

pub async fn analyze_ont_port_conflict(
    pool: &PgPool,
    cabinet_id: &str,
    context: Value,
) -> Result<PortConflictAlert, CabinetAgentError> {
    if context.get("cabinet_id").and_then(Value::as_str) != Some(cabinet_id)
        || context
            .get("gpon_port_id")
            .and_then(Value::as_str)
            .is_none()
        || context
            .get("resource_state")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(CabinetAgentError::InvalidRequest);
    }
    let requested_port_id = context["gpon_port_id"]
        .as_str()
        .ok_or(CabinetAgentError::InvalidRequest)?
        .to_owned();
    let agent_id = select_agent(pool, cabinet_id).await?;
    let request = WorkerRequest {
        operation: "ONT_PORT_CONFLICT",
        agent_id: &agent_id,
        cabinet_id,
        context,
    };
    let input = serde_json::to_vec(&request).map_err(|_| CabinetAgentError::InvalidRequest)?;
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(CabinetAgentError::WorkerUnavailable)?
        .to_path_buf();
    let worker_output = tokio::task::spawn_blocking(move || run_worker(&project_root, &input))
        .await
        .map_err(|_| CabinetAgentError::WorkerUnavailable)??;
    let response: WorkerPortConflictResponse =
        serde_json::from_str(&worker_output).map_err(|_| CabinetAgentError::InvalidResponse)?;
    if !response.success {
        let _ = response.error;
        return Err(CabinetAgentError::InvalidResponse);
    }
    let alert = response
        .port_conflict
        .ok_or(CabinetAgentError::InvalidResponse)?;
    if alert.decision != CodexDecision::Deny
        || alert.agent_id != agent_id
        || alert.cabinet_id != cabinet_id
        || alert.gpon_port_id != requested_port_id
        || alert.reason_code.is_empty()
        || alert.reason_code.len() > 64
        || !alert
            .reason_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        || alert.alert.is_empty()
        || alert.alert.len() > 240
    {
        return Err(CabinetAgentError::InvalidResponse);
    }
    Ok(alert)
}

async fn select_agent(pool: &PgPool, cabinet_id: &str) -> Result<String, CabinetAgentError> {
    let rows = sqlx::query("SELECT agent_id FROM agents WHERE cabinet_id=$1 AND role='CABINET_AGENT' AND status='ACTIVE' ORDER BY agent_id")
        .bind(cabinet_id)
        .fetch_all(pool)
        .await
        .map_err(|_| CabinetAgentError::AgentUnavailable)?;
    if rows.len() != 1 {
        return Err(CabinetAgentError::AgentUnavailable);
    }
    rows[0]
        .try_get("agent_id")
        .map_err(|_| CabinetAgentError::AgentUnavailable)
}

fn run_worker(project_root: &Path, input: &[u8]) -> Result<String, CabinetAgentError> {
    run_worker_with_codex_path(project_root, input, env::var_os("FIBERSPIDER_CODEX_PATH"))
}

fn run_worker_with_codex_path(
    project_root: &Path,
    input: &[u8],
    codex_path: Option<OsString>,
) -> Result<String, CabinetAgentError> {
    let python = env::var_os("FIBERSPIDER_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.join(".venv/bin/python"));
    let path = env::var("PATH").unwrap_or_default();
    let home = env::var("HOME").unwrap_or_default();
    let mut command = Command::new(python);
    command
        .args(["-m", "agent.decision_worker"])
        .current_dir(project_root)
        .env_clear()
        .env("PATH", path)
        .env("HOME", home)
        .env("TMPDIR", env::temp_dir())
        .env("PYTHONPATH", ".")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(codex_path) = codex_path {
        command.env("FIBERSPIDER_CODEX_PATH", codex_path);
    }
    let mut child = command
        .spawn()
        .map_err(|_| CabinetAgentError::WorkerUnavailable)?;
    child
        .stdin
        .take()
        .ok_or(CabinetAgentError::WorkerUnavailable)?
        .write_all(input)
        .map_err(|_| CabinetAgentError::WorkerUnavailable)?;
    let output = child
        .wait_with_output()
        .map_err(|_| CabinetAgentError::WorkerUnavailable)?;
    if !output.status.success() {
        return Err(CabinetAgentError::WorkerUnavailable);
    }
    String::from_utf8(output.stdout).map_err(|_| CabinetAgentError::InvalidResponse)
}

fn parse_worker_response(
    output: &str,
    agent_id: &str,
    expected: &RoeSelectionRequest,
) -> Result<CabinetDecision, CabinetAgentError> {
    let response: WorkerResponse =
        serde_json::from_str(output).map_err(|_| CabinetAgentError::InvalidResponse)?;
    if !response.success {
        let _ = response.error;
        return Err(CabinetAgentError::InvalidResponse);
    }
    let decision = response
        .decision
        .ok_or(CabinetAgentError::InvalidResponse)?;
    if decision.agent_id != agent_id
        || decision.cabinet_id != expected.cabinet_id
        || decision.current_roe_id != expected.current_roe_id
        || decision.candidate_roe_id != expected.candidate_roe_id
        || decision.inventory_fingerprint != expected.inventory_fingerprint
        || decision.reason_code.is_empty()
        || decision.reason_code.len() > 64
        || !decision
            .reason_code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(CabinetAgentError::InvalidResponse);
    }
    if matches!(
        decision.decision,
        CodexDecision::Allow | CodexDecision::AllowWithWarning
    ) && expected.available_fibers.unwrap_or_default() < expected.required_fibers
    {
        return Err(CabinetAgentError::InvalidResponse);
    }
    Ok(decision)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        CabinetAgentError, CabinetDecision, CodexDecision, DecisionRisk, RoeSelectionRequest,
        parse_worker_response, run_worker_with_codex_path, snapshot_fingerprint,
    };
    use serde_json::json;
    use std::{fs, os::unix::fs::PermissionsExt, path::Path};
    use uuid::Uuid;

    fn request() -> RoeSelectionRequest {
        RoeSelectionRequest {
            cabinet_id: "CAB-D".into(),
            current_roe_id: Some("ROE-OLD".into()),
            candidate_roe_id: "ROE-NEW".into(),
            candidate_splitter_id: "SPL-D-01".into(),
            distance_m: 25.0,
            available_fibers: Some(4),
            required_fibers: 1,
            inventory_fingerprint: "a".repeat(64),
        }
    }

    fn worker_json(decision: CodexDecision) -> String {
        serde_json::to_string(&json!({
            "success": true,
            "decision": {
                "decision": decision,
                "agent_id": "AGENT-CAB-D",
                "cabinet_id": "CAB-D",
                "current_roe_id": "ROE-OLD",
                "candidate_roe_id": "ROE-NEW",
                "inventory_fingerprint": "a".repeat(64),
                "reason_code": "SAME_PATH_CAPACITY_OK",
                "risk": "LOW"
            },
            "error": null
        }))
        .unwrap()
    }

    #[test]
    fn accepts_allow_and_deny_as_typed_decisions() {
        for expected_decision in [
            CodexDecision::Allow,
            CodexDecision::AllowWithWarning,
            CodexDecision::Deny,
            CodexDecision::NeedsSupervisor,
        ] {
            let result = parse_worker_response(
                &worker_json(expected_decision.clone()),
                "AGENT-CAB-D",
                &request(),
            )
            .unwrap();
            assert_eq!(result.decision, expected_decision);
        }
    }

    #[test]
    fn rejects_wrong_cabinet_agent_identity_and_unknown_decisions() {
        let mut response: CabinetDecision = serde_json::from_value(json!({
            "decision":"ALLOW",
            "agent_id":"AGENT-CAB-A",
            "cabinet_id":"CAB-D",
            "current_roe_id":"ROE-OLD",
            "candidate_roe_id":"ROE-NEW",
            "inventory_fingerprint":"a".repeat(64),
            "reason_code":"SAME_PATH_CAPACITY_OK",
            "risk":"LOW"
        }))
        .unwrap();
        response.risk = DecisionRisk::Low;
        let envelope = json!({"success":true,"decision":response,"error":null});
        assert_eq!(
            parse_worker_response(&envelope.to_string(), "AGENT-CAB-D", &request())
                .unwrap_err()
                .to_string(),
            CabinetAgentError::InvalidResponse.to_string()
        );
    }

    #[test]
    fn rejects_allow_when_rust_capacity_validation_fails() {
        let mut request = request();
        request.available_fibers = Some(0);
        assert_eq!(
            parse_worker_response(&worker_json(CodexDecision::Allow), "AGENT-CAB-D", &request)
                .unwrap_err()
                .to_string(),
            CabinetAgentError::InvalidResponse.to_string()
        );
    }

    #[test]
    fn python_worker_returns_allow_and_deny_without_real_codex() {
        let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for expected_decision in [CodexDecision::Allow, CodexDecision::Deny] {
            let directory =
                std::env::temp_dir().join(format!("fiberspider-agent-worker-{}", Uuid::new_v4()));
            fs::create_dir(&directory).unwrap();
            let codex_mock = directory.join("codex-mock");
            let output = serde_json::to_string(&json!({
                "decision": expected_decision,
                "agent_id": "AGENT-CAB-D",
                "cabinet_id": "CAB-D",
                "current_roe_id": null,
                "candidate_roe_id": "ROE-NEW",
                "inventory_fingerprint": "a".repeat(64),
                "reason_code": "TEST_DECISION",
                "risk": "LOW"
            }))
            .unwrap();
            fs::write(
                &codex_mock,
                format!("#!/bin/sh\nprintf '%s\\n' '{output}'\n"),
            )
            .unwrap();
            fs::set_permissions(&codex_mock, fs::Permissions::from_mode(0o700)).unwrap();
            let worker_request = serde_json::to_vec(&json!({
                "agent_id": "AGENT-CAB-D",
                "cabinet_id": "CAB-D",
                "context": {
                    "candidate_cabinet_id": "CAB-D",
                    "current_roe_id": null,
                    "candidate_roe_id": "ROE-NEW",
                    "candidate_splitter_id": "SPL-D-01",
                    "distance_m": 20.0,
                    "available_fibers": 4,
                    "required_fibers": 1,
                    "inventory_fingerprint": "a".repeat(64)
                }
            }))
            .unwrap();
            let response = run_worker_with_codex_path(
                project_root,
                &worker_request,
                Some(codex_mock.as_os_str().to_owned()),
            )
            .unwrap();
            assert_eq!(
                parse_worker_response(&response, "AGENT-CAB-D", &nearby_request())
                    .unwrap_or_else(|error| {
                        panic!("worker output rejected: {error}; response={response}")
                    })
                    .decision,
                expected_decision
            );
            fs::remove_dir_all(directory).unwrap();
        }
    }

    fn nearby_request() -> RoeSelectionRequest {
        RoeSelectionRequest {
            cabinet_id: "CAB-D".into(),
            current_roe_id: None,
            candidate_roe_id: "ROE-NEW".into(),
            candidate_splitter_id: "SPL-D-01".into(),
            distance_m: 20.0,
            available_fibers: Some(4),
            required_fibers: 1,
            inventory_fingerprint: "a".repeat(64),
        }
    }

    #[test]
    fn fingerprints_are_stable_for_equivalent_json_objects() {
        let first = json!({"cabinet_id":"CAB-D","capacity":4});
        let second = json!({"capacity":4,"cabinet_id":"CAB-D"});
        assert_eq!(
            snapshot_fingerprint(&first).unwrap(),
            snapshot_fingerprint(&second).unwrap()
        );
    }
}
