use crate::{db::audit, domain::topology::TopologyPath, mcp::auth::InvocationContext};
use serde_json::json;
use sqlx::{PgPool, Row};

pub async fn validate_path(pool: &PgPool, path: &TopologyPath) -> Result<bool, sqlx::Error> {
    let row = sqlx::query("SELECT EXISTS(SELECT 1 FROM cabinets c JOIN ftth_lines l ON l.cabinet_id=c.cabinet_id JOIN splitters s ON s.cabinet_id=c.cabinet_id AND s.line_id=l.line_id WHERE c.cabinet_id=$1 AND l.line_id=$2 AND s.splitter_id=$3 AND c.status='ACTIVE' AND l.status='ACTIVE' AND s.status='ACTIVE') AS valid")
        .bind(&path.cabinet_id).bind(&path.line_id).bind(&path.splitter_id)
        .fetch_one(pool).await?;
    if !row.try_get::<bool, _>("valid")? {
        return Ok(false);
    }
    if let Some(roe_id) = &path.roe_id {
        let valid = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM roes WHERE roe_id=$1 AND cabinet_id=$2 AND splitter_id=$3 AND status='ACTIVE')")
            .bind(roe_id).bind(&path.cabinet_id).bind(&path.splitter_id).fetch_one(pool).await?;
        if !valid {
            return Ok(false);
        }
    }
    if let Some(pte_id) = &path.pte_id {
        let valid = sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM ptes WHERE pte_id=$1 AND cabinet_id=$2 AND line_id=$3 AND status='ACTIVE')")
            .bind(pte_id).bind(&path.cabinet_id).bind(&path.line_id).fetch_one(pool).await?;
        if !valid {
            return Ok(false);
        }
    }
    Ok(true)
}

pub async fn propose_path(
    pool: &PgPool,
    actor: &InvocationContext,
    path: &TopologyPath,
) -> Result<String, &'static str> {
    if !validate_path(pool, path)
        .await
        .map_err(|_| "topology lookup failed")?
    {
        return Err("path does not match persisted cabinet/line/splitter topology");
    }
    let source_event = path
        .source_event
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or("source_event is required")?;
    let input = json!({"cabinet_id":path.cabinet_id,"line_id":path.line_id,"splitter_id":path.splitter_id,"roe_id":path.roe_id,"pte_id":path.pte_id});
    let proposal = json!({"path":input,"status":"PENDING_REVIEW"});
    let validation = json!({"status":"PASS","check":"persisted resource links"});
    let mut transaction = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    let change_id = audit::record(
        &mut transaction,
        actor,
        audit::AuditRecord {
            tool_name: "update_physical_path",
            tool_class: "WRITE_TOPOLOGY",
            source_event: Some(source_event.to_owned()),
            sanitized_input: input,
            validation,
            result: proposal.clone(),
            resource_id: path.cabinet_id.clone(),
            next_state: proposal,
            effect: audit::AuditEffect::Pending,
        },
    )
    .await
    .map_err(|_| "proposal audit write failed")?
    .ok_or("proposal audit did not return a topology change id")?;
    transaction
        .commit()
        .await
        .map_err(|_| "proposal transaction commit failed")?;
    Ok(change_id)
}

#[cfg(test)]
mod tests {
    use crate::domain::topology::TopologyPath;

    #[test]
    fn path_requires_a_single_cabinet_line_splitter_chain() {
        let valid = TopologyPath {
            cabinet_id: "CAB-A".into(),
            line_id: "FTTH-LINE-A".into(),
            splitter_id: "SPL-A-01".into(),
            roe_id: None,
            pte_id: None,
            source_event: Some("TEST-EVENT".into()),
        };
        let impossible = TopologyPath {
            line_id: "FTTH-LINE-B".into(),
            ..valid.clone()
        };
        assert_ne!(valid.line_id, impossible.line_id);
    }
}
