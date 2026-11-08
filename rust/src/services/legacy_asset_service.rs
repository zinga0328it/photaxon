use crate::{
    mcp::auth::ControlPlaneAdminContext, services::control_plane_admin_service as operations,
};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LegacyAssetKind {
    Cabinet,
    Splitter,
    Port,
    Roe,
    Pte,
    Feeder,
    Ont,
    Modem,
}

impl LegacyAssetKind {
    fn source(&self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Cabinet => ("cabinets", "cabinet_id", "CABINET"),
            Self::Splitter => ("splitters", "splitter_id", "SPLITTER"),
            Self::Port => ("ports", "port_id", "PORT"),
            Self::Roe => ("roes", "roe_id", "ROE"),
            Self::Pte => ("ptes", "pte_id", "PTE"),
            Self::Feeder => ("feeder_cables", "cable_id", "CABLE"),
            Self::Ont => ("onts", "ont_id", "ONT"),
            Self::Modem => ("modems", "modem_id", "MODEM"),
        }
    }
    fn query(&self) -> &'static str {
        match self {
            Self::Cabinet => {
                "SELECT jsonb_build_object('cabinet_id',cabinet_id,'status',status,'display_name',display_name,'location_reference',location_reference,'parent_node',parent_node) FROM cabinets WHERE cabinet_id=$1"
            }
            Self::Splitter => {
                "SELECT jsonb_build_object('splitter_id',splitter_id,'status',status,'cabinet_id',cabinet_id,'line_id',line_id,'ratio',ratio,'capacity_total',capacity_total) FROM splitters WHERE splitter_id=$1"
            }
            Self::Port => {
                "SELECT jsonb_build_object('port_id',port_id,'splitter_id',splitter_id,'position',position,'resource_state',resource_state) FROM ports WHERE port_id=$1"
            }
            Self::Roe => {
                "SELECT jsonb_build_object('roe_id',roe_id,'status',status,'cabinet_id',cabinet_id,'splitter_id',splitter_id,'street_name',street_name,'street_number',street_number,'postal_code',postal_code,'city',city,'latitude',latitude,'longitude',longitude,'location_accuracy_m',location_accuracy_m,'geolocation_source',geolocation_source,'feeder_cable_id',feeder_cable_id) FROM roes WHERE roe_id=$1"
            }
            Self::Pte => {
                "SELECT jsonb_build_object('pte_id',pte_id,'status',status,'cabinet_id',cabinet_id,'line_id',line_id,'address_reference',address_reference) FROM ptes WHERE pte_id=$1"
            }
            Self::Feeder => {
                "SELECT jsonb_build_object('cable_id',cable_id,'status',status,'cabinet_id',cabinet_id,'fiber_count',fiber_count,'fibers_used',fibers_used,'fibers_reserved',fibers_reserved) FROM feeder_cables WHERE cable_id=$1"
            }
            Self::Ont => {
                "SELECT jsonb_build_object('ont_id',ont_id,'status',status,'serial_number',serial_number,'roe_id',roe_id,'current_port_id',current_port_id,'customer_id',customer_id,'vendor',vendor,'model',model) FROM onts WHERE ont_id=$1"
            }
            Self::Modem => {
                "SELECT jsonb_build_object('modem_id',modem_id,'status',status,'serial',serial,'vendor',vendor,'model',model) FROM modems WHERE modem_id=$1"
            }
        }
    }
}

pub async fn preview(
    pool: &PgPool,
    kind: &LegacyAssetKind,
    id: &str,
) -> Result<Value, &'static str> {
    crate::domain::physical_asset::identifier(id)?;
    let source: Value = sqlx::query_scalar(kind.query())
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|_| "legacy source unavailable")?
        .ok_or("legacy asset does not exist")?;
    Ok(
        json!({"status":"PASS","persisted":false,"source_table":kind.source().0,"source_id":id,"source":source,"metadata_status":"LEGACY_INCOMPLETE","measured_address_or_gps_invented":false}),
    )
}

pub async fn import(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    kind: &LegacyAssetKind,
    id: &str,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let tx = operations::begin_transaction(pool).await?;
    let (tx, result) = import_transaction(tx, actor, operation_id, kind, id).await?;
    tx.commit()
        .await
        .map_err(|_| "legacy import commit failed")?;
    Ok(result)
}

pub async fn import_transaction<'tx>(
    mut tx: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    kind: &LegacyAssetKind,
    id: &str,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    crate::domain::physical_asset::identifier(id)?;
    let payload = json!({"kind":kind,"source_id":id});
    let digest = operations::input_digest(&payload)?;
    if let Some(result) =
        operations::begin_operation(&mut tx, actor, operation_id, "import_legacy_asset", &digest)
            .await?
    {
        return Ok((tx, result));
    }
    let source: Value = sqlx::query_scalar(kind.query())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| "legacy source unavailable")?
        .ok_or("legacy asset does not exist")?;
    let (table, _, asset_kind) = kind.source();
    let prior=sqlx::query("SELECT asset_kind,legacy_source_table,legacy_source_id,metadata_status,revision FROM physical_asset_records WHERE asset_id=$1 FOR UPDATE")
        .bind(id).fetch_optional(&mut *tx).await.map_err(|_|"legacy destination lookup failed")?;
    if let Some(prior) = prior {
        if prior
            .try_get::<String, _>("asset_kind")
            .map_err(|_| "destination type unavailable")?
            != asset_kind
        {
            return Err("legacy ID conflicts with a different physical asset type");
        }
        let bound: Option<String> = prior
            .try_get("legacy_source_table")
            .map_err(|_| "legacy provenance unavailable")?;
        let source_id: Option<String> = prior
            .try_get("legacy_source_id")
            .map_err(|_| "legacy source ID unavailable")?;
        if bound.as_deref() != Some(table) || source_id.as_deref() != Some(id) {
            return Err(
                "existing measured asset must be explicitly linked, not overwritten by import",
            );
        }
        let result = json!({"asset_id":id,"status":"ALREADY_IMPORTED","metadata_status":prior.try_get::<String,_>("metadata_status").map_err(|_|"metadata status unavailable")?,"source_table":table});
        operations::finish_operation(
            &mut tx,
            actor,
            operation_id,
            "import_legacy_asset",
            asset_kind,
            id,
            &digest,
            payload,
            json!({"status":"PASS"}),
            result.clone(),
        )
        .await?;
        return Ok((tx, result));
    }
    let state = source.get("status").and_then(Value::as_str).unwrap_or(
        if source.get("resource_state").and_then(Value::as_str) == Some("FAULT") {
            "FAULT"
        } else {
            "PLANNED"
        },
    );
    if !matches!(
        state,
        "ACTIVE" | "INACTIVE" | "FAULT" | "RETIRED" | "PLANNED"
    ) {
        return Err("legacy state requires explicit mapping");
    }
    let details = match kind {
        LegacyAssetKind::Cabinet => {
            json!({"cabinet_type":null,"central_area":source.get("parent_node")})
        }
        LegacyAssetKind::Splitter => {
            json!({"ratio":source.get("ratio"),"capacity_total":source.get("capacity_total"),"container_id":source.get("cabinet_id"),"level":null,"location_type":null})
        }
        LegacyAssetKind::Port => {
            json!({"owner_id":source.get("splitter_id"),"number":source.get("position"),"direction":null,"fiber_id":null})
        }
        LegacyAssetKind::Roe | LegacyAssetKind::Pte => {
            json!({"cabinet_id":source.get("cabinet_id"),"building":null,"capacity":null})
        }
        LegacyAssetKind::Feeder => {
            json!({"fiber_count":source.get("fiber_count"),"cable_type":"FEEDER","origin_id":source.get("cabinet_id"),"destination_id":null,"length_m":null,"route":null})
        }
        LegacyAssetKind::Ont => {
            json!({"serial":source.get("serial_number"),"vendor":source.get("vendor"),"model":source.get("model"),"roe_port_id":source.get("current_port_id"),"oto_id":null})
        }
        LegacyAssetKind::Modem => {
            json!({"serial":source.get("serial"),"vendor":source.get("vendor"),"model":source.get("model"),"ont_id":null})
        }
    };
    let attributes = json!({"asset_id":id,"operational_code":id,"state":state,"address":null,"geolocation":null,
        "asset":{"kind":asset_kind,"details":details},"notes":null,"legacy_source":source,"metadata_missing":["measured_address","gps","operational_code_confirmation","physical_details"]});
    sqlx::query("INSERT INTO physical_asset_records(asset_id,asset_kind,operational_code,state,attributes,created_by,updated_by,metadata_status,legacy_source_table,legacy_source_id,legacy_observed_at) VALUES ($1,$2,$1,$3,$4,$5,$5,'LEGACY_INCOMPLETE',$6,$1,now())")
        .bind(id).bind(asset_kind).bind(state).bind(attributes).bind(actor.principal_id).bind(table).execute(&mut *tx).await.map_err(|_|"legacy physical registry insert failed")?;
    for field in ["cabinet_id", "splitter_id", "roe_id", "current_port_id"] {
        if let Some(target) = source.get(field).and_then(Value::as_str)
            && target != id
        {
            let registered: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM physical_asset_records WHERE asset_id=$1)",
            )
            .bind(target)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| "legacy parent lookup failed")?;
            if !registered {
                return Err("import the referenced legacy parent before its child");
            }
            sqlx::query("INSERT INTO physical_asset_dependencies(asset_id,target_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
                    .bind(id).bind(target).execute(&mut *tx).await.map_err(|_|"legacy dependency insert failed")?;
        }
    }
    let result = json!({"asset_id":id,"status":"IMPORTED","metadata_status":"LEGACY_INCOMPLETE","source_table":table,"measured_address_or_gps_invented":false});
    operations::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "import_legacy_asset",
        asset_kind,
        id,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((tx, result))
}

pub async fn sync_inventory(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    crate::domain::physical_asset::identifier(operation_id)?;
    let mut tx = operations::begin_transaction(pool).await?;
    let rows=sqlx::query("SELECT kind,id FROM (SELECT 0 AS ordering,'CABINET' AS kind,cabinet_id AS id FROM cabinets UNION ALL SELECT 1,'SPLITTER',splitter_id FROM splitters UNION ALL SELECT 2,'PORT',port_id FROM ports UNION ALL SELECT 3,'FEEDER',cable_id FROM feeder_cables UNION ALL SELECT 4,'ROE',roe_id FROM roes UNION ALL SELECT 5,'PTE',pte_id FROM ptes UNION ALL SELECT 6,'ONT',ont_id FROM onts UNION ALL SELECT 7,'MODEM',modem_id FROM modems) source ORDER BY ordering,id LIMIT 1001")
        .fetch_all(&mut *tx).await.map_err(|_|"legacy inventory unavailable")?;
    if rows.len() > 1000 {
        return Err("legacy inventory exceeds safe batch limit; import explicitly by ID");
    }
    let mut results = Vec::new();
    for row in rows {
        let id: String = row.try_get("id").map_err(|_| "legacy ID invalid")?;
        let kind: LegacyAssetKind = serde_json::from_value(json!(
            row.try_get::<String, _>("kind")
                .map_err(|_| "legacy type invalid")?
        ))
        .map_err(|_| "legacy type unsupported")?;
        let child_digest =
            operations::input_digest(&json!({"operation_id":operation_id,"kind":kind,"id":id}))?;
        let child_operation = format!("IMPORT-{}", &child_digest[..48]);
        let (transaction, result) =
            import_transaction(tx, actor, &child_operation, &kind, &id).await?;
        tx = transaction;
        results.push(result);
    }
    tx.commit()
        .await
        .map_err(|_| "legacy inventory sync commit failed")?;
    Ok(
        json!({"status":"IMPORTED","count":results.len(),"results":results,"measured_address_or_gps_invented":false}),
    )
}
