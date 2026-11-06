use crate::{
    db::audit, domain::cabinet::CreateCabinet, mcp::auth::InvocationContext,
    policy::topology_validator,
};
use serde_json::json;
use sqlx::{PgPool, Row};

pub async fn create(
    pool: &PgPool,
    actor: &InvocationContext,
    input: CreateCabinet,
) -> Result<String, &'static str> {
    topology_validator::validate_cabinet(&input)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| "database transaction unavailable")?;
    let parent_exists = input.parent_node == "CENTRAL-01"
        || sqlx::query("SELECT EXISTS(SELECT 1 FROM cabinets WHERE cabinet_id=$1)")
            .bind(&input.parent_node)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "topology lookup failed")?
            .get::<bool, _>(0);
    if !parent_exists {
        return Err("parent node does not exist");
    }
    for (table, column, id) in [
        ("cabinets", "cabinet_id", input.cabinet_id.as_str()),
        ("ftth_lines", "line_id", input.line_id.as_str()),
        ("splitters", "splitter_id", input.splitter_id.as_str()),
    ] {
        if crate::db::topology::resource_exists(pool, table, column, id)
            .await
            .map_err(|_| "topology lookup failed")?
        {
            return Err("resource identifier already exists");
        }
    }
    sqlx::query("INSERT INTO cabinets (cabinet_id,status,display_name,location_reference,parent_node,notes) VALUES ($1,'ACTIVE',$2,$3,$4,$5)")
        .bind(&input.cabinet_id).bind(&input.display_name).bind(&input.location_reference)
        .bind(&input.parent_node).bind(&input.notes).execute(&mut *tx).await.map_err(|_| "cabinet insert failed")?;
    sqlx::query("INSERT INTO ftth_lines (line_id,cabinet_id,openstack_network_name,status) VALUES ($1,$2,$3,'ACTIVE')")
        .bind(&input.line_id).bind(&input.cabinet_id).bind(&input.network_name)
        .execute(&mut *tx).await.map_err(|_| "line insert failed")?;
    sqlx::query("INSERT INTO splitters (splitter_id,cabinet_id,line_id,ratio,capacity_total,secondary_splitter_allowed,status) VALUES ($1,$2,$3,$4,$5,FALSE,'ACTIVE')")
        .bind(&input.splitter_id).bind(&input.cabinet_id).bind(&input.line_id)
        .bind(input.splitter_ratio).bind(input.splitter_capacity)
        .execute(&mut *tx).await.map_err(|_| "splitter insert failed")?;
    let agent_id = format!("AGENT-{}", input.cabinet_id);
    sqlx::query("INSERT INTO agents (agent_id,cabinet_id,role,status) VALUES ($1,$2,'CABINET_AGENT','ACTIVE')")
        .bind(&agent_id).bind(&input.cabinet_id).execute(&mut *tx).await.map_err(|_| "cabinet agent insert failed")?;
    for position in 1..=input.splitter_capacity {
        let port_id = format!("{}-PORT-{position:02}", input.splitter_id);
        sqlx::query("INSERT INTO ports (port_id,splitter_id,position,resource_state) VALUES ($1,$2,$3,'FREE')")
            .bind(&port_id).bind(&input.splitter_id).bind(position)
            .execute(&mut *tx).await.map_err(|_| "splitter port insert failed")?;
        sqlx::query("INSERT INTO resource_states (resource_id,resource_kind,resource_state,metadata) VALUES ($1,'PORT','FREE',$2)")
            .bind(&port_id).bind(json!({"cabinet_id": input.cabinet_id, "splitter_id": input.splitter_id}))
            .execute(&mut *tx).await.map_err(|_| "port state insert failed")?;
    }
    let sanitized = json!({"cabinet_id":input.cabinet_id,"line_id":input.line_id,"splitter_id":input.splitter_id,"parent_node":input.parent_node,"location_reference":input.location_reference});
    let state = json!({"cabinet":"ACTIVE","line":"ACTIVE","splitter":"ACTIVE","agent":"ACTIVE"});
    audit::record(
        &mut tx,
        actor,
        audit::AuditRecord {
            tool_name: "create_cabinet",
            tool_class: "WRITE_TOPOLOGY",
            source_event: Some(input.source_event.clone()),
            sanitized_input: sanitized,
            validation: json!({"status":"PASS"}),
            result: state.clone(),
            resource_id: input.cabinet_id.clone(),
            next_state: state,
            effect: audit::AuditEffect::Applied,
        },
    )
    .await
    .map_err(|_| "audit write failed")?;
    tx.commit()
        .await
        .map_err(|_| "cabinet transaction commit failed")?;
    Ok(input.cabinet_id)
}
