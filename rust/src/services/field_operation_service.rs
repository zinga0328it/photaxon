use crate::{
    domain::field_ops::NearbyRoeCandidate,
    domain::field_ops::{
        CreateFieldRoe, RegisterModem, RegisterOnt, available_feeder_fibers, validate_new_roe,
        validate_registration_metadata, validate_serial,
    },
    llm::cabinet_agent::{
        CabinetDecision, CodexDecision, DecisionRisk, RoeSelectionRequest, decide_roe_selection,
        snapshot_fingerprint,
    },
    services::field_query_service,
    telegram::workflow::{WorkflowDraft, WorkflowKind, WorkflowStatus},
};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub async fn evaluate_nearby_selection(
    pool: &PgPool,
    session_id: &str,
    expected_version: i32,
    mut draft: WorkflowDraft,
) -> Result<(WorkflowDraft, CabinetDecision), &'static str> {
    if draft.kind != WorkflowKind::NearbyRoe
        || draft.step != crate::telegram::workflow::WorkflowStep::VerifyCabinetAgent
        || draft.status != WorkflowStatus::Draft
        || draft.version != expected_version
    {
        return Err("nearby workflow state is invalid");
    }
    let latitude = number_field(&draft, "technician_latitude")?;
    let longitude = number_field(&draft, "technician_longitude")?;
    let selected_roe_id = text_field(&draft, "candidate_roe_id")?.to_owned();
    let stored_candidates: Vec<NearbyRoeCandidate> = serde_json::from_value(
        draft
            .fields
            .get("nearby_candidates")
            .cloned()
            .ok_or("nearby candidates are missing")?,
    )
    .map_err(|_| "nearby candidates are invalid")?;
    if !stored_candidates
        .iter()
        .any(|candidate| candidate.roe_id == selected_roe_id)
    {
        return Err("selected ROE was not in the displayed candidate list");
    }
    let stored_fingerprint = text_field(&draft, "nearby_inventory_fingerprint")?.to_owned();
    let candidates = field_query_service::nearby_roes(pool, latitude, longitude, 10_000.0)
        .await
        .map_err(|_| "nearby inventory lookup failed")?;
    let live_snapshot =
        serde_json::to_value(&candidates).map_err(|_| "nearby inventory serialization failed")?;
    let live_fingerprint =
        snapshot_fingerprint(&live_snapshot).map_err(|_| "nearby inventory fingerprint failed")?;
    if live_fingerprint != stored_fingerprint {
        return Err("nearby inventory changed; restart search");
    }
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.roe_id == selected_roe_id)
        .ok_or("selected ROE is no longer a nearby candidate")?;
    let model_decision = decide_roe_selection(
        pool,
        RoeSelectionRequest {
            cabinet_id: candidate.cabinet_id.clone(),
            current_roe_id: None,
            candidate_roe_id: candidate.roe_id.clone(),
            candidate_splitter_id: candidate.splitter_id.clone(),
            distance_m: candidate.distance_m,
            available_fibers: candidate.available_fibers,
            required_fibers: 1,
            inventory_fingerprint: live_fingerprint.clone(),
        },
    )
    .await
    .map_err(|_| "selected cabinet agent could not return a valid decision")?;
    let after_codex_candidates =
        field_query_service::nearby_roes(pool, latitude, longitude, 10_000.0)
            .await
            .map_err(|_| "nearby inventory recheck failed")?;
    let after_codex_snapshot = serde_json::to_value(after_codex_candidates)
        .map_err(|_| "nearby inventory serialization failed")?;
    let snapshot_changed = snapshot_fingerprint(&after_codex_snapshot)
        .map_err(|_| "nearby inventory fingerprint failed")?
        != live_fingerprint;
    let (decision, validation_status) =
        validate_nearby_snapshot(model_decision.clone(), snapshot_changed);
    let model_decision_json = serde_json::to_value(&model_decision)
        .map_err(|_| "cabinet decision serialization failed")?;
    let effective_decision_json =
        serde_json::to_value(&decision).map_err(|_| "cabinet decision serialization failed")?;
    if decision.agent_id != model_decision.agent_id
        || decision.candidate_roe_id != model_decision.candidate_roe_id
        || decision.inventory_fingerprint != live_fingerprint
    {
        return Err("cabinet decision identity changed");
    }
    let proposal_id = Uuid::new_v4().to_string();
    draft
        .record_cabinet_decision(&proposal_id, effective_decision_json.clone())
        .map_err(|_| "cabinet decision cannot be recorded in this workflow step")?;
    let payload = serde_json::to_value(&draft).map_err(|_| "workflow serialization failed")?;
    let database_status = match draft.status {
        WorkflowStatus::Review => "REVIEW",
        WorkflowStatus::Failed => "FAILED",
        _ => return Err("cabinet decision produced an invalid workflow state"),
    };
    let mut transaction = pool.begin().await.map_err(|_| "database unavailable")?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "nearby transaction isolation unavailable")?;
    let state = sqlx::query("SELECT w.version,w.status,(w.expires_at<=now()) AS workflow_expired,s.status AS session_status,(s.expires_at<=now()) AS session_expired,t.status AS technician_status,w.technician_id FROM telegram_workflows w JOIN field_technician_sessions s ON s.session_id=w.session_id JOIN technicians t ON t.technician_id=w.technician_id WHERE w.workflow_id=$1 AND w.session_id=$2 FOR UPDATE OF w,s,t")
        .bind(&draft.workflow_id)
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "workflow lock failed")?
        .ok_or("workflow is unavailable")?;
    let stored_version: i32 = state
        .try_get("version")
        .map_err(|_| "workflow version invalid")?;
    let stored_status: String = state
        .try_get("status")
        .map_err(|_| "workflow status invalid")?;
    let workflow_expired: bool = state
        .try_get("workflow_expired")
        .map_err(|_| "workflow expiry invalid")?;
    let session_status: String = state
        .try_get("session_status")
        .map_err(|_| "session status invalid")?;
    let session_expired: bool = state
        .try_get("session_expired")
        .map_err(|_| "session expiry invalid")?;
    let technician_status: String = state
        .try_get("technician_status")
        .map_err(|_| "technician status invalid")?;
    let technician_id: String = state
        .try_get("technician_id")
        .map_err(|_| "technician identity invalid")?;
    if stored_version != expected_version
        || stored_status != "DRAFT"
        || workflow_expired
        || session_status != "ACTIVE"
        || session_expired
        || technician_status != "ACTIVE"
    {
        return Err("workflow or technician state changed during Codex decision");
    }
    let proposal_status = match decision.decision {
        CodexDecision::Allow | CodexDecision::AllowWithWarning => "PENDING",
        CodexDecision::Deny => "REJECTED",
        CodexDecision::NeedsSupervisor => "PENDING",
    };
    sqlx::query("INSERT INTO roe_selection_proposals (proposal_id,workflow_id,cabinet_id,agent_id,current_roe_id,candidate_roe_id,inventory_fingerprint,decision,reason_code,risk,status,expires_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,now()+interval '10 minutes')")
        .bind(&proposal_id)
        .bind(&draft.workflow_id)
        .bind(&decision.cabinet_id)
        .bind(&decision.agent_id)
        .bind(&decision.current_roe_id)
        .bind(&decision.candidate_roe_id)
        .bind(&decision.inventory_fingerprint)
        .bind(decision_name(&decision.decision))
        .bind(&decision.reason_code)
        .bind(risk_name(&decision.risk))
        .bind(proposal_status)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "ROE proposal write failed")?;
    let result_code = format!("CODEX_{}", decision_name(&decision.decision));
    sqlx::query("INSERT INTO field_operation_audit (event_id,workflow_id,correlation_id,idempotency_key,actor_type,actor_id,operation_type,resource_type,resource_id,sanitized_input,before_state,after_state,validator_result,agent_decision,result_code) VALUES ($1,$2,$2,$3,'TECHNICIAN',$4,'CODEX_DECISION','ROE',$5,$6,NULL,$7,$8,$9,$10)")
        .bind(Uuid::new_v4().to_string())
        .bind(&draft.workflow_id)
        .bind(format!("{}:CODEX_DECISION", draft.workflow_id))
        .bind(technician_id)
        .bind(&decision.candidate_roe_id)
        .bind(json!({"cabinet_id":decision.cabinet_id,"candidate_roe_id":decision.candidate_roe_id,"inventory_fingerprint":decision.inventory_fingerprint}))
        .bind(json!({"proposal_id":proposal_id,"decision":decision_name(&decision.decision),"risk":risk_name(&decision.risk)}))
        .bind(json!({"status":validation_status,"validator":"rust-nearby-snapshot","snapshot_fingerprint":live_fingerprint}))
        .bind(model_decision_json)
        .bind(result_code)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "Codex decision audit write failed")?;
    sqlx::query("UPDATE telegram_workflows SET state=$3,status=$4,draft_payload=$5,version=$6,updated_at=now() WHERE workflow_id=$1 AND session_id=$2 AND version=$7")
        .bind(&draft.workflow_id)
        .bind(session_id)
        .bind(format!("{:?}", draft.step))
        .bind(database_status)
        .bind(payload)
        .bind(draft.version)
        .bind(expected_version)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "workflow decision persistence failed")?;
    transaction
        .commit()
        .await
        .map_err(|_| "decision transaction failed")?;
    Ok((draft, decision))
}

pub async fn check_ont_gpon_port(
    pool: &PgPool,
    session_id: &str,
    workflow_id: &str,
    roe_id: &str,
    gpon_port_id: &str,
    serial: &str,
) -> Result<(), &'static str> {
    let mut transaction = pool.begin().await.map_err(|_| "database unavailable")?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "transaction isolation unavailable")?;
    let row = sqlx::query("SELECT p.resource_state,s.cabinet_id AS port_cabinet,(p.splitter_id=r.splitter_id AND s.cabinet_id=r.cabinet_id AND s.status='ACTIVE' AND r.status='ACTIVE') AS topology_valid FROM ports p JOIN splitters s ON s.splitter_id=p.splitter_id JOIN roes r ON r.roe_id=$1 WHERE p.port_id=$2 FOR UPDATE OF p")
        .bind(roe_id)
        .bind(gpon_port_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "GPON port lookup failed")?
        .ok_or("GPON port does not belong to an existing splitter/ROE")?;
    let port_state: String = row
        .try_get("resource_state")
        .map_err(|_| "GPON state invalid")?;
    let port_cabinet: String = row.try_get("port_cabinet").map_err(|_| "cabinet invalid")?;
    let topology_valid: bool = row
        .try_get("topology_valid")
        .map_err(|_| "GPON topology invalid")?;
    let assigned = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM onts WHERE current_port_id=$1 AND (status='ACTIVE' OR customer_id IS NOT NULL))",
    )
    .bind(gpon_port_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(|_| "GPON assignment lookup failed")?;
    if !gpon_port_available(&port_state, assigned, topology_valid) {
        let technician_id = sqlx::query_scalar::<_, String>(
            "SELECT technician_id FROM field_technician_sessions WHERE session_id=$1 AND status='ACTIVE' AND expires_at>now()",
        )
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "technician session lookup failed")?
        .ok_or("technician session is not active")?;
        sqlx::query("INSERT INTO field_operation_audit (event_id,workflow_id,correlation_id,idempotency_key,actor_type,actor_id,operation_type,resource_type,resource_id,sanitized_input,before_state,after_state,validator_result,agent_decision,result_code) VALUES ($1,$2,$2,$3,'TECHNICIAN',$4,'ONT_PROVISIONING_DENIED','GPON_PORT',$5,$6,$7,NULL,$8,NULL,'DENIED_PORT_OCCUPIED') ON CONFLICT (idempotency_key) DO NOTHING")
            .bind(Uuid::new_v4().to_string())
            .bind(workflow_id)
            .bind(format!("{}:GPON_PORT_OCCUPIED", workflow_id))
            .bind(technician_id)
            .bind(gpon_port_id)
            .bind(json!({"roe_id":roe_id,"serial":serial}))
            .bind(json!({"resource_state":port_state,"cabinet_id":port_cabinet}))
            .bind(json!({"status":"DENY","validator":"rust-gpon-port-guard"}))
            .execute(&mut *transaction)
            .await
            .map_err(|_| "GPON conflict audit failed")?;
        transaction
            .commit()
            .await
            .map_err(|_| "GPON conflict audit commit failed")?;
        return Err("GPON_PORT_OCCUPIED");
    }
    transaction
        .commit()
        .await
        .map_err(|_| "GPON port preflight commit failed")?;
    Ok(())
}

pub async fn analyze_ont_port_conflict(
    pool: &PgPool,
    session_id: &str,
    workflow_id: &str,
    work_order_id: &str,
    roe_id: &str,
    gpon_port_id: &str,
    requested_serial: &str,
) -> Result<crate::llm::cabinet_agent::PortConflictAlert, &'static str> {
    let row = sqlx::query("SELECT p.resource_state,s.cabinet_id,EXISTS(SELECT 1 FROM onts o WHERE o.current_port_id=p.port_id AND o.status='ACTIVE') AS active_ont_assigned,EXISTS(SELECT 1 FROM onts o WHERE o.current_port_id=p.port_id AND o.customer_id IS NOT NULL) AS customer_assignment_present FROM ports p JOIN splitters s ON s.splitter_id=p.splitter_id JOIN roes r ON r.cabinet_id=s.cabinet_id WHERE r.roe_id=$1 AND p.port_id=$2")
        .bind(roe_id)
        .bind(gpon_port_id)
        .fetch_optional(pool)
        .await
        .map_err(|_| "GPON conflict context lookup failed")?
        .ok_or("GPON conflict context is unavailable")?;
    let resource_state: String = row
        .try_get("resource_state")
        .map_err(|_| "GPON state unavailable")?;
    let cabinet_id: String = row
        .try_get("cabinet_id")
        .map_err(|_| "GPON cabinet unavailable")?;
    let active_ont_assigned: bool = row
        .try_get("active_ont_assigned")
        .map_err(|_| "ONT assignment state unavailable")?;
    let customer_assignment_present: bool = row
        .try_get("customer_assignment_present")
        .map_err(|_| "customer assignment state unavailable")?;
    let context = json!({
        "cabinet_id": cabinet_id,
        "gpon_port_id": gpon_port_id,
        "resource_state": resource_state,
        "active_ont_assigned": active_ont_assigned,
        "customer_assignment_present": customer_assignment_present,
        "requested_serial": requested_serial,
        "work_order_id": work_order_id,
    });
    let alert =
        crate::llm::cabinet_agent::analyze_ont_port_conflict(pool, &cabinet_id, context.clone())
            .await
            .map_err(|_| "cabinet agent conflict analysis unavailable")?;
    let mut transaction = pool.begin().await.map_err(|_| "database unavailable")?;
    sqlx::query("INSERT INTO field_operation_audit (event_id,workflow_id,correlation_id,idempotency_key,actor_type,actor_id,operation_type,resource_type,resource_id,sanitized_input,before_state,after_state,validator_result,agent_decision,result_code) SELECT $1,$2,$2,$3,'TECHNICIAN',s.technician_id,'CODEX_PORT_CONFLICT','GPON_PORT',$4,$5,$6,$7,$8,$7,'CODEX_DENY' FROM field_technician_sessions s WHERE s.session_id=$9 AND s.status='ACTIVE' ON CONFLICT (idempotency_key) DO NOTHING")
        .bind(Uuid::new_v4().to_string())
        .bind(workflow_id)
        .bind(format!("{}:CODEX_PORT_CONFLICT", workflow_id))
        .bind(gpon_port_id)
        .bind(json!({"work_order_id":work_order_id,"roe_id":roe_id,"requested_serial":requested_serial}))
        .bind(json!({"resource_state":resource_state,"active_ont_assigned":active_ont_assigned,"customer_assignment_present":customer_assignment_present}))
        .bind(serde_json::to_value(&alert).map_err(|_| "Codex conflict serialization failed")?)
        .bind(json!({"status":"PASS","validator":"rust-gpon-port-guard","decision":"DENY"}))
        .bind(session_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "Codex conflict audit failed")?;
    transaction
        .commit()
        .await
        .map_err(|_| "Codex conflict audit commit failed")?;
    Ok(alert)
}

fn gpon_port_available(port_state: &str, assigned: bool, topology_valid: bool) -> bool {
    port_state == "FREE" && !assigned && topology_valid
}

fn decision_name(decision: &CodexDecision) -> &'static str {
    match decision {
        CodexDecision::Allow => "ALLOW",
        CodexDecision::AllowWithWarning => "ALLOW_WITH_WARNING",
        CodexDecision::Deny => "DENY",
        CodexDecision::NeedsSupervisor => "NEEDS_SUPERVISOR",
    }
}

fn risk_name(risk: &DecisionRisk) -> &'static str {
    match risk {
        DecisionRisk::Low => "LOW",
        DecisionRisk::Medium => "MEDIUM",
        DecisionRisk::High => "HIGH",
    }
}

fn validate_nearby_snapshot(
    mut model_decision: CabinetDecision,
    snapshot_changed: bool,
) -> (CabinetDecision, &'static str) {
    if snapshot_changed {
        model_decision.decision = CodexDecision::Deny;
        model_decision.reason_code = "RUST_SNAPSHOT_CHANGED".into();
        model_decision.risk = DecisionRisk::High;
        (model_decision, "DENY_SNAPSHOT_CHANGED")
    } else {
        (model_decision, "PASS")
    }
}

pub async fn confirm_workflow(
    pool: &PgPool,
    session_id: &str,
    workflow_id: &str,
) -> Result<String, &'static str> {
    let mut transaction = pool.begin().await.map_err(|_| "database unavailable")?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL SERIALIZABLE")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "transaction isolation unavailable")?;
    let row = sqlx::query("SELECT w.status,w.draft_payload,w.technician_id,(w.expires_at<=now()) AS workflow_expired,s.status AS session_status,(s.expires_at<=now()) AS session_expired,t.status AS technician_status FROM telegram_workflows w JOIN field_technician_sessions s ON s.session_id=w.session_id JOIN technicians t ON t.technician_id=w.technician_id WHERE w.workflow_id=$1 AND w.session_id=$2 FOR UPDATE OF w,s,t")
        .bind(workflow_id)
        .bind(session_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "workflow lookup failed")?
        .ok_or("workflow is not available to this session")?;
    let status: String = row
        .try_get("status")
        .map_err(|_| "workflow state is invalid")?;
    let technician_id: String = row
        .try_get("technician_id")
        .map_err(|_| "workflow actor is invalid")?;
    let session_status: String = row
        .try_get("session_status")
        .map_err(|_| "technician session is invalid")?;
    let technician_status: String = row
        .try_get("technician_status")
        .map_err(|_| "technician status is unavailable")?;
    let workflow_expired: bool = row
        .try_get("workflow_expired")
        .map_err(|_| "workflow expiry is invalid")?;
    let session_expired: bool = row
        .try_get("session_expired")
        .map_err(|_| "session expiry is invalid")?;
    if status == "CONFIRMED" {
        let resource_id = sqlx::query_scalar::<_, String>(
            "SELECT resource_id FROM field_operation_audit WHERE workflow_id=$1 AND result_code='APPLIED'",
        )
        .bind(workflow_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| "audit lookup failed")?
        .ok_or("confirmed workflow has no applied audit")?;
        transaction
            .commit()
            .await
            .map_err(|_| "database commit failed")?;
        return Ok(resource_id);
    }
    if status != "REVIEW" {
        return Err("only a reviewed workflow can be confirmed");
    }
    if session_status != "ACTIVE" || technician_status != "ACTIVE" || session_expired {
        return Err("technician session is not active");
    }
    if workflow_expired {
        sqlx::query(
            "UPDATE telegram_workflows SET status='EXPIRED',updated_at=now() WHERE workflow_id=$1",
        )
        .bind(workflow_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "workflow expiry update failed")?;
        transaction
            .commit()
            .await
            .map_err(|_| "database commit failed")?;
        return Err("workflow expired");
    }
    let payload: Value = row
        .try_get("draft_payload")
        .map_err(|_| "workflow payload is invalid")?;
    let mut draft: WorkflowDraft =
        serde_json::from_value(payload).map_err(|_| "workflow payload is invalid")?;
    if draft.status != WorkflowStatus::Review {
        return Err("workflow draft is not in review");
    }
    if draft.kind == WorkflowKind::UnlinkModem {
        return Err("technician cannot disconnect a customer modem");
    }

    let (operation_type, resource_type, resource_id, after_state) = match draft.kind {
        WorkflowKind::CreateRoe => {
            let input = roe_input(&draft)?;
            let id = create_roe(&mut transaction, session_id, &technician_id, &input).await?;
            ("CREATE_ROE", "ROE", id, json!({"status":"ACTIVE"}))
        }
        WorkflowKind::RegisterOnt => {
            let input = ont_input(&draft)?;
            match register_ont(&mut transaction, session_id, &technician_id, &input).await {
                Ok(id) => ("REGISTER_ONT", "ONT", id, json!({"status":"ACTIVE"})),
                Err("GPON_PORT_OCCUPIED") => {
                    sqlx::query("INSERT INTO field_operation_audit (event_id,workflow_id,correlation_id,idempotency_key,actor_type,actor_id,operation_type,resource_type,resource_id,sanitized_input,before_state,after_state,validator_result,agent_decision,result_code) VALUES ($1,$2,$2,$3,'TECHNICIAN',$4,'ONT_PROVISIONING_DENIED','GPON_PORT',$5,$6,$7,NULL,$8,NULL,'DENIED_PORT_OCCUPIED') ON CONFLICT (idempotency_key) DO NOTHING")
                        .bind(Uuid::new_v4().to_string())
                        .bind(workflow_id)
                        .bind(format!("{}:GPON_PORT_OCCUPIED", workflow_id))
                        .bind(&technician_id)
                        .bind(&input.gpon_port_id)
                        .bind(json!({"roe_id":input.roe_id,"serial":input.serial}))
                        .bind(json!({"resource_state":"OCCUPIED_OR_NOT_FREE"}))
                        .bind(json!({"status":"DENY","validator":"rust-gpon-port-guard"}))
                        .execute(&mut *transaction)
                        .await
                        .map_err(|_| "GPON conflict audit failed")?;
                    draft.status = WorkflowStatus::Failed;
                    draft.step = crate::telegram::workflow::WorkflowStep::Done;
                    draft.version += 1;
                    let failed_payload = serde_json::to_value(&draft)
                        .map_err(|_| "workflow serialization failed")?;
                    sqlx::query("UPDATE telegram_workflows SET status='FAILED',state='Done',draft_payload=$2,version=$3,updated_at=now() WHERE workflow_id=$1")
                        .bind(workflow_id)
                        .bind(failed_payload)
                        .bind(draft.version)
                        .execute(&mut *transaction)
                        .await
                        .map_err(|_| "failed GPON workflow persistence failed")?;
                    transaction
                        .commit()
                        .await
                        .map_err(|_| "GPON conflict commit failed")?;
                    return Err("GPON_PORT_OCCUPIED");
                }
                Err(error) => return Err(error),
            }
        }
        WorkflowKind::RegisterModem => {
            let input = modem_input(&draft)?;
            let id = register_modem(&mut transaction, session_id, &technician_id, &input).await?;
            (
                "REGISTER_MODEM",
                "MODEM",
                id,
                json!({"status":"ACTIVE","ont_id":input.ont_id}),
            )
        }
        WorkflowKind::ReplaceModem => {
            let input = modem_input(&draft)?;
            let old_modem_id = text_field(&draft, "current_modem_id")?;
            let id = replace_modem(&mut transaction, &technician_id, old_modem_id, &input).await?;
            (
                "REPLACE_MODEM",
                "MODEM",
                id,
                json!({"status":"ACTIVE","replaces":old_modem_id,"ont_id":input.ont_id}),
            )
        }
        WorkflowKind::UnlinkModem => {
            let ont_id = text_field(&draft, "ont_id")?;
            let modem_id = text_field(&draft, "current_modem_id")?;
            let reason = text_field(&draft, "reason")?;
            unlink_modem(&mut transaction, ont_id, modem_id, reason).await?;
            (
                "UNLINK_MODEM",
                "MODEM",
                modem_id.to_owned(),
                json!({"status":"UNLINKED","ont_id":ont_id,"reason":reason}),
            )
        }
        WorkflowKind::SearchAsset => return Err("search workflow is not a write operation"),
        WorkflowKind::NearbyRoe => {
            let candidate_roe_id = match confirm_nearby_selection(&mut transaction, &draft).await {
                Ok(candidate_roe_id) => candidate_roe_id,
                Err("nearby proposal expired") => {
                    sqlx::query("UPDATE telegram_workflows SET status='EXPIRED',updated_at=now() WHERE workflow_id=$1")
                        .bind(workflow_id)
                        .execute(&mut *transaction)
                        .await
                        .map_err(|_| "workflow expiry update failed")?;
                    transaction
                        .commit()
                        .await
                        .map_err(|_| "database commit failed")?;
                    return Err("nearby proposal expired");
                }
                Err(error) => return Err(error),
            };
            (
                "SELECT_ALTERNATIVE_ROE",
                "ROE",
                candidate_roe_id.clone(),
                json!({"status":"CONFIRMED","roe_proposal_id":draft.fields.get("roe_proposal_id")}),
            )
        }
    };

    draft
        .confirm()
        .map_err(|_| "workflow confirmation state is invalid")?;
    let after_payload =
        serde_json::to_value(&draft).map_err(|_| "workflow serialization failed")?;
    let agent_decision = draft
        .fields
        .get("codex_decision")
        .cloned()
        .unwrap_or(Value::Null);
    sqlx::query("INSERT INTO field_operation_audit (event_id,workflow_id,correlation_id,idempotency_key,actor_type,actor_id,operation_type,resource_type,resource_id,sanitized_input,before_state,after_state,validator_result,agent_decision,result_code) VALUES ($1,$2,$2,$2,'TECHNICIAN',$3,$4,$5,$6,$7,NULL,$8,$9,$10,'APPLIED')")
        .bind(Uuid::new_v4().to_string())
        .bind(workflow_id)
        .bind(&technician_id)
        .bind(operation_type)
        .bind(resource_type)
        .bind(&resource_id)
        .bind(Value::Object(draft.fields.clone()))
        .bind(after_state)
        .bind(json!({"status":"PASS","validator":"rust-field-ops"}))
        .bind(agent_decision)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "field operation audit write failed")?;
    sqlx::query("UPDATE telegram_workflows SET status='CONFIRMED',state='Done',draft_payload=$2,version=$3,confirmed_at=now(),updated_at=now() WHERE workflow_id=$1")
        .bind(workflow_id)
        .bind(after_payload)
        .bind(draft.version)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "workflow confirmation write failed")?;
    transaction
        .commit()
        .await
        .map_err(|_| "database commit failed")?;
    Ok(resource_id)
}

async fn confirm_nearby_selection(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    draft: &WorkflowDraft,
) -> Result<String, &'static str> {
    let proposal_id = text_field(draft, "roe_proposal_id")?;
    let candidate_roe_id = text_field(draft, "candidate_roe_id")?;
    let latitude = number_field(draft, "technician_latitude")?;
    let longitude = number_field(draft, "technician_longitude")?;
    let expected_fingerprint = text_field(draft, "nearby_inventory_fingerprint")?;
    let codex_decision = draft
        .fields
        .get("codex_decision")
        .and_then(Value::as_object)
        .ok_or("cabinet decision is missing")?;
    let decision_name = codex_decision
        .get("decision")
        .and_then(Value::as_str)
        .ok_or("cabinet decision is invalid")?;
    if !matches!(decision_name, "ALLOW" | "ALLOW_WITH_WARNING") {
        return Err("cabinet decision does not permit confirmation");
    }
    let row = sqlx::query("SELECT cabinet_id,agent_id,candidate_roe_id,inventory_fingerprint,decision,status,(expires_at<=now()) AS proposal_expired FROM roe_selection_proposals WHERE proposal_id=$1 AND workflow_id=$2 FOR UPDATE")
        .bind(proposal_id)
        .bind(&draft.workflow_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| "ROE proposal lookup failed")?
        .ok_or("ROE proposal is unavailable")?;
    let cabinet_id: String = row
        .try_get("cabinet_id")
        .map_err(|_| "proposal cabinet invalid")?;
    let agent_id: String = row
        .try_get("agent_id")
        .map_err(|_| "proposal agent invalid")?;
    let proposal_roe_id: String = row
        .try_get("candidate_roe_id")
        .map_err(|_| "proposal candidate invalid")?;
    let proposal_fingerprint: String = row
        .try_get("inventory_fingerprint")
        .map_err(|_| "proposal snapshot invalid")?;
    let proposal_decision: String = row
        .try_get("decision")
        .map_err(|_| "proposal decision invalid")?;
    let proposal_status: String = row
        .try_get("status")
        .map_err(|_| "proposal status invalid")?;
    let proposal_expired: bool = row
        .try_get("proposal_expired")
        .map_err(|_| "proposal expiry invalid")?;
    if proposal_expired || proposal_fingerprint != expected_fingerprint {
        sqlx::query("UPDATE roe_selection_proposals SET status='EXPIRED' WHERE proposal_id=$1")
            .bind(proposal_id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| "proposal expiry update failed")?;
        return Err("nearby proposal expired");
    }
    if proposal_status != "PENDING"
        || proposal_roe_id != candidate_roe_id
        || proposal_decision != decision_name
    {
        return Err("ROE proposal changed or expired");
    }
    let agent_active = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM agents WHERE agent_id=$1 AND cabinet_id=$2 AND role='CABINET_AGENT' AND status='ACTIVE')")
        .bind(&agent_id)
        .bind(&cabinet_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| "cabinet agent lookup failed")?;
    if !agent_active {
        return Err("selected cabinet agent is no longer active");
    }
    let candidates =
        field_query_service::nearby_roes(&mut **transaction, latitude, longitude, 10_000.0)
            .await
            .map_err(|_| "nearby inventory lookup failed")?;
    let snapshot =
        serde_json::to_value(&candidates).map_err(|_| "nearby inventory serialization failed")?;
    let current_fingerprint =
        snapshot_fingerprint(&snapshot).map_err(|_| "nearby inventory fingerprint failed")?;
    if current_fingerprint != expected_fingerprint
        || !candidates
            .iter()
            .any(|candidate| candidate.roe_id == candidate_roe_id)
    {
        sqlx::query("UPDATE roe_selection_proposals SET status='EXPIRED' WHERE proposal_id=$1")
            .bind(proposal_id)
            .execute(&mut **transaction)
            .await
            .map_err(|_| "proposal expiry update failed")?;
        return Err("nearby proposal expired");
    }
    sqlx::query("UPDATE roe_selection_proposals SET status='CONFIRMED',confirmed_at=now() WHERE proposal_id=$1 AND status='PENDING'")
        .bind(proposal_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "proposal confirmation failed")?;
    Ok(candidate_roe_id.to_owned())
}

async fn create_roe(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    session_id: &str,
    technician_id: &str,
    input: &CreateFieldRoe,
) -> Result<String, &'static str> {
    validate_new_roe(input)?;
    if input.geolocation_source != "TELEGRAM" {
        return Err("technician workflow requires Telegram geolocation");
    }
    let topology_ok = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM cabinets c JOIN splitters s ON s.cabinet_id=c.cabinet_id WHERE c.cabinet_id=$1 AND c.status='ACTIVE' AND s.splitter_id=$2 AND s.status='ACTIVE')")
        .bind(&input.cabinet_id)
        .bind(&input.splitter_id)
        .fetch_one(&mut **transaction)
        .await
        .map_err(|_| "cabinet or splitter lookup failed")?;
    if !topology_ok {
        return Err("active splitter does not belong to the selected cabinet");
    }
    sqlx::query("INSERT INTO feeder_cables (cable_id,cabinet_id,fiber_count) VALUES ($1,$2,$3) ON CONFLICT (cable_id) DO NOTHING")
        .bind(&input.feeder_cable_id)
        .bind(&input.cabinet_id)
        .bind(input.feeder_fiber_count)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "feeder cable creation failed")?;
    let cable = sqlx::query("SELECT cabinet_id,fiber_count,fibers_used,fibers_reserved,status FROM feeder_cables WHERE cable_id=$1 FOR UPDATE")
        .bind(&input.feeder_cable_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| "feeder cable lookup failed")?
        .ok_or("feeder cable is unavailable")?;
    let cable_cabinet: String = cable
        .try_get("cabinet_id")
        .map_err(|_| "feeder cable state is invalid")?;
    let fiber_count: i32 = cable
        .try_get("fiber_count")
        .map_err(|_| "feeder cable state is invalid")?;
    let fibers_used: i32 = cable
        .try_get("fibers_used")
        .map_err(|_| "feeder cable state is invalid")?;
    let fibers_reserved: i32 = cable
        .try_get("fibers_reserved")
        .map_err(|_| "feeder cable state is invalid")?;
    let cable_status: String = cable
        .try_get("status")
        .map_err(|_| "feeder cable state is invalid")?;
    if cable_cabinet != input.cabinet_id
        || fiber_count != input.feeder_fiber_count
        || cable_status != "ACTIVE"
    {
        return Err("feeder cable does not match the selected cabinet or capacity");
    }
    let available = available_feeder_fibers(fiber_count, fibers_used, fibers_reserved)
        .ok_or("feeder cable capacity state is invalid")?;
    if input.fibers_allocated > available {
        return Err("insufficient free feeder fibers");
    }
    let address = format!(
        "{} {}, {} {}",
        input.street_name, input.street_number, input.postal_code, input.city
    );
    sqlx::query("INSERT INTO roes (roe_id,cabinet_id,splitter_id,address_reference,upstream_resource,notes,address_note,source_event,created_by_session,street_name,street_number,city,postal_code,latitude,longitude,location_accuracy_m,geolocated_at,geolocation_source,feeder_cable_id,created_by_technician) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,now(),$17,$18,$19)")
        .bind(&input.roe_id)
        .bind(&input.cabinet_id)
        .bind(&input.splitter_id)
        .bind(address)
        .bind(&input.splitter_id)
        .bind(&input.address_note)
        .bind(&input.address_note)
        .bind(&input.source_event)
        .bind(session_id)
        .bind(&input.street_name)
        .bind(&input.street_number)
        .bind(&input.city)
        .bind(&input.postal_code)
        .bind(input.latitude)
        .bind(input.longitude)
        .bind(input.location_accuracy_m)
        .bind(&input.geolocation_source)
        .bind(&input.feeder_cable_id)
        .bind(technician_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "ROE creation failed")?;
    sqlx::query("UPDATE feeder_cables SET fibers_reserved=fibers_reserved+$2,updated_at=now() WHERE cable_id=$1")
        .bind(&input.feeder_cable_id)
        .bind(input.fibers_allocated)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "feeder capacity reservation failed")?;
    sqlx::query("INSERT INTO feeder_cable_allocations (allocation_id,cable_id,roe_id,fibers_allocated,fibers_reserved) VALUES ($1,$2,$3,$4,$4)")
        .bind(Uuid::new_v4().to_string())
        .bind(&input.feeder_cable_id)
        .bind(&input.roe_id)
        .bind(input.fibers_allocated)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "feeder allocation creation failed")?;
    Ok(input.roe_id.clone())
}

async fn register_ont(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    _session_id: &str,
    technician_id: &str,
    input: &RegisterOnt,
) -> Result<String, &'static str> {
    validate_serial(&input.serial)?;
    validate_registration_metadata(
        input.vendor.as_deref(),
        input.model.as_deref(),
        &input.source_event,
    )?;
    let port = sqlx::query("SELECT p.resource_state,(p.splitter_id=r.splitter_id AND s.cabinet_id=r.cabinet_id AND s.status='ACTIVE' AND r.status='ACTIVE') AS topology_valid FROM ports p JOIN splitters s ON s.splitter_id=p.splitter_id JOIN roes r ON r.roe_id=$1 WHERE p.port_id=$2 FOR UPDATE OF p")
        .bind(&input.roe_id)
        .bind(&input.gpon_port_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| "GPON port lookup failed")?
        .ok_or("GPON port does not belong to an existing splitter/ROE")?;
    let port_state: String = port
        .try_get("resource_state")
        .map_err(|_| "GPON port state is invalid")?;
    let topology_valid: bool = port
        .try_get("topology_valid")
        .map_err(|_| "GPON topology is invalid")?;
    let ont_already_assigned = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM onts WHERE current_port_id=$1 AND (status='ACTIVE' OR customer_id IS NOT NULL))",
    )
    .bind(&input.gpon_port_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| "GPON assignment lookup failed")?;
    if !gpon_port_available(&port_state, ont_already_assigned, topology_valid) {
        return Err("GPON_PORT_OCCUPIED");
    }
    let roe_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM roes WHERE roe_id=$1 AND status='ACTIVE')",
    )
    .bind(&input.roe_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| "ROE lookup failed")?;
    if !roe_active {
        return Err("active ROE does not exist");
    }
    let ont_id = format!("ONT-{}", Uuid::new_v4());
    sqlx::query("INSERT INTO onts (ont_id,serial_number,roe_id,current_port_id,vendor,model,created_by_technician) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(&ont_id)
        .bind(&input.serial)
        .bind(&input.roe_id)
        .bind(&input.gpon_port_id)
        .bind(&input.vendor)
        .bind(&input.model)
        .bind(technician_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "ONT registration failed")?;
    sqlx::query("UPDATE ports SET resource_state='OCCUPIED',updated_at=now() WHERE port_id=$1 AND resource_state='FREE'")
        .bind(&input.gpon_port_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "GPON port assignment failed")?;
    Ok(ont_id)
}

async fn register_modem(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    _session_id: &str,
    technician_id: &str,
    input: &RegisterModem,
) -> Result<String, &'static str> {
    validate_serial(&input.serial)?;
    validate_registration_metadata(
        input.vendor.as_deref(),
        input.model.as_deref(),
        &input.source_event,
    )?;
    let ont_active = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM onts WHERE ont_id=$1 AND status='ACTIVE')",
    )
    .bind(&input.ont_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| "ONT lookup failed")?;
    if !ont_active {
        return Err("active ONT does not exist");
    }
    let active_link = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ont_modem_links WHERE ont_id=$1 AND status='ACTIVE')",
    )
    .bind(&input.ont_id)
    .fetch_one(&mut **transaction)
    .await
    .map_err(|_| "ONT modem link lookup failed")?;
    if active_link {
        return Err("ONT already has an active modem; use replacement workflow");
    }
    let modem_id = format!("MODEM-{}", Uuid::new_v4());
    let link_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO modems (modem_id,serial,vendor,model,created_by_technician) VALUES ($1,$2,$3,$4,$5)")
        .bind(&modem_id)
        .bind(&input.serial)
        .bind(&input.vendor)
        .bind(&input.model)
        .bind(technician_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "modem registration failed")?;
    sqlx::query("INSERT INTO ont_modem_links (link_id,ont_id,modem_id,status,created_by_technician,source_event) VALUES ($1,$2,$3,'ACTIVE',$4,$5)")
        .bind(link_id)
        .bind(&input.ont_id)
        .bind(&modem_id)
        .bind(technician_id)
        .bind(&input.source_event)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "ONT modem link creation failed")?;
    Ok(modem_id)
}

async fn replace_modem(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    technician_id: &str,
    old_modem_id: &str,
    input: &RegisterModem,
) -> Result<String, &'static str> {
    validate_serial(&input.serial)?;
    validate_registration_metadata(
        input.vendor.as_deref(),
        input.model.as_deref(),
        &input.source_event,
    )?;
    let ont_status =
        sqlx::query_scalar::<_, String>("SELECT status FROM onts WHERE ont_id=$1 FOR UPDATE")
            .bind(&input.ont_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|_| "replacement ONT lookup failed")?;
    if ont_status.as_deref() != Some("ACTIVE") {
        return Err("replacement requires the existing active ONT association");
    }
    let active_link = sqlx::query("SELECT link_id FROM ont_modem_links WHERE ont_id=$1 AND modem_id=$2 AND status='ACTIVE' FOR UPDATE")
        .bind(&input.ont_id)
        .bind(old_modem_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| "active modem link lookup failed")?
        .ok_or("selected modem is not active on this ONT")?;
    let old_link_id: String = active_link
        .try_get("link_id")
        .map_err(|_| "active modem link is invalid")?;
    sqlx::query("UPDATE ont_modem_links SET status='REPLACED',ended_at=now() WHERE link_id=$1")
        .bind(&old_link_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "old modem link update failed")?;
    sqlx::query("UPDATE modems SET status='INACTIVE',updated_at=now() WHERE modem_id=$1 AND status='ACTIVE'")
        .bind(old_modem_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "old modem update failed")?;
    let modem_id = format!("MODEM-{}", Uuid::new_v4());
    let new_link_id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO modems (modem_id,serial,vendor,model,created_by_technician) VALUES ($1,$2,$3,$4,$5)")
        .bind(&modem_id)
        .bind(&input.serial)
        .bind(&input.vendor)
        .bind(&input.model)
        .bind(technician_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "replacement modem registration failed")?;
    sqlx::query("INSERT INTO ont_modem_links (link_id,ont_id,modem_id,status,created_by_technician,source_event) VALUES ($1,$2,$3,'ACTIVE',$4,$5)")
        .bind(&new_link_id)
        .bind(&input.ont_id)
        .bind(&modem_id)
        .bind(technician_id)
        .bind(&input.source_event)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "replacement link creation failed")?;
    sqlx::query("UPDATE ont_modem_links SET replaced_by_link_id=$2 WHERE link_id=$1")
        .bind(old_link_id)
        .bind(new_link_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "replacement history update failed")?;
    Ok(modem_id)
}

async fn unlink_modem(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ont_id: &str,
    modem_id: &str,
    reason: &str,
) -> Result<(), &'static str> {
    if reason.trim().is_empty() || reason.len() > 512 || reason.chars().any(char::is_control) {
        return Err("unlink reason is invalid");
    }
    let link = sqlx::query("SELECT link_id FROM ont_modem_links WHERE ont_id=$1 AND modem_id=$2 AND status='ACTIVE' FOR UPDATE")
        .bind(ont_id)
        .bind(modem_id)
        .fetch_optional(&mut **transaction)
        .await
        .map_err(|_| "active modem link lookup failed")?
        .ok_or("selected modem is not active on this ONT")?;
    let link_id: String = link
        .try_get("link_id")
        .map_err(|_| "active modem link is invalid")?;
    sqlx::query("UPDATE ont_modem_links SET status='UNLINKED',ended_at=now() WHERE link_id=$1")
        .bind(link_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "modem unlink failed")?;
    sqlx::query("UPDATE modems SET status='INACTIVE',updated_at=now() WHERE modem_id=$1 AND status='ACTIVE'")
        .bind(modem_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| "modem status update failed")?;
    Ok(())
}

fn roe_input(draft: &WorkflowDraft) -> Result<CreateFieldRoe, &'static str> {
    Ok(CreateFieldRoe {
        roe_id: text_field(draft, "roe_id")?.to_owned(),
        cabinet_id: text_field(draft, "cabinet_id")?.to_owned(),
        splitter_id: text_field(draft, "splitter_id")?.to_owned(),
        street_name: text_field(draft, "street_name")?.to_owned(),
        street_number: text_field(draft, "street_number")?.to_owned(),
        city: text_field(draft, "city")?.to_owned(),
        postal_code: text_field(draft, "postal_code")?.to_owned(),
        latitude: number_field(draft, "latitude")?,
        longitude: number_field(draft, "longitude")?,
        location_accuracy_m: number_field(draft, "location_accuracy_m")?,
        geolocation_source: text_field(draft, "geolocation_source")?.to_owned(),
        feeder_cable_id: text_field(draft, "feeder_cable_id")?.to_owned(),
        feeder_fiber_count: integer_field(draft, "feeder_fiber_count")?,
        fibers_allocated: integer_field(draft, "fibers_allocated")?,
        address_note: draft
            .fields
            .get("address_note")
            .and_then(Value::as_str)
            .map(str::to_owned),
        source_event: draft.workflow_id.clone(),
    })
}

fn ont_input(draft: &WorkflowDraft) -> Result<RegisterOnt, &'static str> {
    Ok(RegisterOnt {
        serial: text_field(draft, "serial")?.to_owned(),
        gpon_port_id: text_field(draft, "gpon_port_id")?.to_owned(),
        roe_id: text_field(draft, "roe_id")?.to_owned(),
        vendor: optional_field(draft, "vendor"),
        model: optional_field(draft, "model"),
        source_event: draft.workflow_id.clone(),
    })
}

fn modem_input(draft: &WorkflowDraft) -> Result<RegisterModem, &'static str> {
    Ok(RegisterModem {
        serial: text_field(draft, "serial")?.to_owned(),
        ont_id: text_field(draft, "ont_id")?.to_owned(),
        vendor: optional_field(draft, "vendor"),
        model: optional_field(draft, "model"),
        source_event: draft.workflow_id.clone(),
    })
}

fn text_field<'a>(draft: &'a WorkflowDraft, key: &str) -> Result<&'a str, &'static str> {
    draft
        .fields
        .get(key)
        .and_then(Value::as_str)
        .ok_or("required workflow field is missing")
}

fn optional_field(draft: &WorkflowDraft, key: &str) -> Option<String> {
    draft
        .fields
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn number_field(draft: &WorkflowDraft, key: &str) -> Result<f64, &'static str> {
    draft
        .fields
        .get(key)
        .and_then(Value::as_f64)
        .ok_or("required numeric workflow field is missing")
}

fn integer_field(draft: &WorkflowDraft, key: &str) -> Result<i32, &'static str> {
    text_field(draft, key)?
        .parse()
        .map_err(|_| "required integer workflow field is invalid")
}

#[cfg(test)]
mod tests {
    use super::{gpon_port_available, validate_nearby_snapshot};
    use crate::llm::cabinet_agent::{CabinetDecision, CodexDecision, DecisionRisk};

    fn allow_decision() -> CabinetDecision {
        CabinetDecision {
            decision: CodexDecision::Allow,
            agent_id: "AGENT-CAB-D".into(),
            cabinet_id: "CAB-D".into(),
            current_roe_id: None,
            candidate_roe_id: "ROE-D-1".into(),
            inventory_fingerprint: "a".repeat(64),
            reason_code: "SAME_PATH_CAPACITY_OK".into(),
            risk: DecisionRisk::Low,
        }
    }

    #[test]
    fn gpon_requires_free_unassigned_port_on_correct_topology() {
        assert!(gpon_port_available("FREE", false, true));
        assert!(!gpon_port_available("OCCUPIED", false, true));
        assert!(!gpon_port_available("FREE", true, true));
        assert!(!gpon_port_available("FREE", false, false));
        assert!(!gpon_port_available("RESERVED", false, true));
    }

    #[test]
    fn unchanged_snapshot_preserves_allow() {
        let (decision, validation) = validate_nearby_snapshot(allow_decision(), false);
        assert_eq!(decision.decision, CodexDecision::Allow);
        assert_eq!(validation, "PASS");
    }

    #[test]
    fn changed_snapshot_overrides_allow_with_rust_deny() {
        let (decision, validation) = validate_nearby_snapshot(allow_decision(), true);
        assert_eq!(decision.decision, CodexDecision::Deny);
        assert_eq!(decision.reason_code, "RUST_SNAPSHOT_CHANGED");
        assert_eq!(decision.risk, DecisionRisk::High);
        assert_eq!(validation, "DENY_SNAPSHOT_CHANGED");
    }
}
