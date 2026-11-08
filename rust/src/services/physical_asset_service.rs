use crate::{
    domain::physical_asset::{self, AssetDetails, PhysicalAsset},
    mcp::auth::ControlPlaneAdminContext,
    services::control_plane_admin_service as operations,
};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Row, Transaction};

pub async fn get(pool: &PgPool, id: &str) -> Result<Option<Value>, &'static str> {
    physical_asset::identifier(id)?;
    sqlx::query_scalar("SELECT jsonb_build_object('asset',attributes,'revision',revision,'metadata_status',metadata_status,'legacy_source_table',legacy_source_table,'legacy_source_id',legacy_source_id,'created_by',created_by,'updated_by',updated_by,'created_at',created_at,'updated_at',updated_at) FROM physical_asset_records WHERE asset_id=$1")
        .bind(id).fetch_optional(pool).await.map_err(|_| "physical asset read failed")
}

pub async fn list(
    pool: &PgPool,
    kind: Option<&str>,
    after: Option<&str>,
) -> Result<Vec<Value>, &'static str> {
    if let Some(id) = after {
        physical_asset::identifier(id)?;
    }
    sqlx::query_scalar("SELECT jsonb_build_object('asset',attributes,'revision',revision,'metadata_status',metadata_status,'legacy_source_table',legacy_source_table) FROM physical_asset_records WHERE ($1::text IS NULL OR asset_kind=$1) AND ($2::text IS NULL OR asset_id>$2) ORDER BY asset_id LIMIT 100")
        .bind(kind).bind(after).fetch_all(pool).await.map_err(|_| "physical asset inventory failed")
}

pub async fn validate_live(
    transaction: &mut Transaction<'_, Postgres>,
    asset: &PhysicalAsset,
    updating: bool,
) -> Result<(), &'static str> {
    physical_asset::validate(asset)?;
    let read_only: bool =
        sqlx::query_scalar("SELECT current_setting('transaction_read_only')='on'")
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| "transaction mode unavailable")?;
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM physical_asset_records WHERE asset_id=$1)")
            .bind(&asset.asset_id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| "asset identity check failed")?;
    if exists != updating {
        return Err(if updating {
            "physical asset does not exist"
        } else {
            "physical asset already exists"
        });
    }
    let duplicate:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM physical_asset_records WHERE asset_kind=$1 AND operational_code=$2 AND asset_id<>$3)")
        .bind(asset.asset.kind()).bind(&asset.operational_code).bind(&asset.asset_id).fetch_one(&mut **transaction).await.map_err(|_| "asset code check failed")?;
    if duplicate {
        return Err("operational code already used for this asset type");
    }
    for (target_id, kinds) in asset.asset.references() {
        let reference_query = if read_only {
            "SELECT asset_kind,state,attributes FROM physical_asset_records WHERE asset_id=$1"
        } else {
            "SELECT asset_kind,state,attributes FROM physical_asset_records WHERE asset_id=$1 FOR SHARE"
        };
        let row = sqlx::query(reference_query)
            .bind(target_id)
            .fetch_optional(&mut **transaction)
            .await
            .map_err(|_| "asset reference lookup failed")?
            .ok_or(
                "physical asset reference does not exist; register its measured metadata first",
            )?;
        let kind: String = row
            .try_get("asset_kind")
            .map_err(|_| "reference type unavailable")?;
        let state: String = row
            .try_get("state")
            .map_err(|_| "reference state unavailable")?;
        if !kinds.contains(&kind.as_str()) || state == "RETIRED" {
            return Err("reference type or state invalid");
        }
        let cyclic:bool=sqlx::query_scalar("WITH RECURSIVE upstream(id) AS (SELECT target_id FROM physical_asset_dependencies WHERE asset_id=$1 UNION SELECT d.target_id FROM physical_asset_dependencies d JOIN upstream u ON d.asset_id=u.id) SELECT EXISTS(SELECT 1 FROM upstream WHERE id=$2)")
            .bind(target_id).bind(&asset.asset_id).fetch_one(&mut **transaction).await.map_err(|_| "dependency cycle check failed")?;
        if cyclic {
            return Err("physical dependency cycle detected");
        }
    }
    match &asset.asset {
        AssetDetails::Tube {
            cable_id,
            number,
            fiber_count,
            ..
        } => {
            let capacity = read_capacity(transaction, cable_id, "fiber_count").await?;
            let used:i64=sqlx::query_scalar("SELECT COALESCE(sum((attributes->'asset'->'details'->>'fiber_count')::int),0) FROM physical_asset_records WHERE asset_kind='TUBE' AND attributes->'asset'->'details'->>'cable_id'=$1 AND asset_id<>$2 AND state<>'RETIRED'")
                .bind(cable_id).bind(&asset.asset_id).fetch_one(&mut **transaction).await.map_err(|_| "tube capacity check failed")?;
            if used + i64::from(*fiber_count) > i64::from(capacity) {
                return Err("cable fiber capacity exceeded");
            }
            unique_number(
                transaction,
                "TUBE",
                "cable_id",
                cable_id,
                *number,
                &asset.asset_id,
            )
            .await?;
        }
        AssetDetails::Fiber {
            cable_id,
            tube_id,
            number,
            ..
        } => {
            let tube = read_typed(transaction, tube_id).await?;
            let AssetDetails::Tube {
                cable_id: actual,
                fiber_count,
                ..
            } = tube.asset
            else {
                return Err("fiber requires tube");
            };
            if actual != *cable_id || *number > fiber_count {
                return Err("fiber cable/tube capacity mismatch");
            }
            unique_number(
                transaction,
                "FIBER",
                "tube_id",
                tube_id,
                *number,
                &asset.asset_id,
            )
            .await?;
        }
        AssetDetails::Port {
            owner_id, number, ..
        } => {
            let capacity = read_capacity(transaction, owner_id, "port_capacity").await?;
            if *number > capacity {
                return Err("port number exceeds owner capacity");
            }
            unique_number(
                transaction,
                "PORT",
                "owner_id",
                owner_id,
                *number,
                &asset.asset_id,
            )
            .await?;
        }
        AssetDetails::Oto {
            work_order_id,
            customer_id,
            ..
        }
        | AssetDetails::Ont {
            work_order_id,
            customer_id,
            ..
        }
        | AssetDetails::Modem {
            work_order_id,
            customer_id,
            ..
        } => {
            let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activation_requests WHERE wr=$1) AND EXISTS(SELECT 1 FROM customers WHERE customer_id=$2 AND status='ACTIVE')")
                .bind(work_order_id).bind(customer_id).fetch_one(&mut **transaction).await.map_err(|_| "WR/customer reference check failed")?;
            if !valid {
                return Err("WR or ACTIVE customer does not exist on AAA");
            }
        }
        _ => {}
    }
    Ok(())
}

async fn read_typed(
    transaction: &mut Transaction<'_, Postgres>,
    id: &str,
) -> Result<PhysicalAsset, &'static str> {
    let attributes: Value =
        sqlx::query_scalar("SELECT attributes FROM physical_asset_records WHERE asset_id=$1")
            .bind(id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| "asset metadata unavailable")?;
    serde_json::from_value(attributes).map_err(|_| "stored physical asset metadata invalid")
}

async fn read_capacity(
    transaction: &mut Transaction<'_, Postgres>,
    id: &str,
    field: &str,
) -> Result<u16, &'static str> {
    let attributes: Value =
        sqlx::query_scalar("SELECT attributes FROM physical_asset_records WHERE asset_id=$1")
            .bind(id)
            .fetch_one(&mut **transaction)
            .await
            .map_err(|_| "asset capacity unavailable")?;
    let details = &attributes["asset"]["details"];
    let capacity = if field == "port_capacity" {
        details
            .get("capacity_total")
            .or_else(|| details.get("ratio"))
            .or_else(|| details.get("capacity"))
    } else {
        details.get(field)
    };
    capacity
        .and_then(Value::as_u64)
        .and_then(|capacity| u16::try_from(capacity).ok())
        .filter(|capacity| *capacity > 0)
        .ok_or("measured legacy capacity is missing or invalid")
}

async fn unique_number(
    transaction: &mut Transaction<'_, Postgres>,
    kind: &str,
    parent_field: &str,
    parent: &str,
    number: u16,
    id: &str,
) -> Result<(), &'static str> {
    let occupied:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM physical_asset_records WHERE asset_kind=$1 AND attributes->'asset'->'details'->>$2=$3 AND (attributes->'asset'->'details'->>'number')::int=$4 AND asset_id<>$5 AND state<>'RETIRED')")
        .bind(kind).bind(parent_field).bind(parent).bind(i32::from(number)).bind(id).fetch_one(&mut **transaction).await.map_err(|_| "physical position lookup failed")?;
    if occupied {
        return Err("physical number is already used in this container");
    }
    Ok(())
}

pub async fn save(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    asset: &PhysicalAsset,
    expected_revision: Option<i64>,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = operations::begin_transaction(pool).await?;
    let (transaction, result) =
        save_transaction(transaction, actor, operation_id, asset, expected_revision).await?;
    transaction
        .commit()
        .await
        .map_err(|_| "physical asset commit failed")?;
    Ok(result)
}

pub async fn save_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    operation_id: &str,
    asset: &PhysicalAsset,
    expected_revision: Option<i64>,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    actor.require_write_authorization()?;
    physical_asset::validate(asset)?;
    let payload = json!({"asset":asset,"expected_revision":expected_revision});
    let digest = operations::input_digest(&payload)?;
    let tool = if expected_revision.is_some() {
        "update_physical_asset"
    } else {
        "create_physical_asset"
    };
    if let Some(result) =
        operations::begin_operation(&mut transaction, actor, operation_id, tool, &digest).await?
    {
        return Ok((transaction, result));
    }
    let prior=sqlx::query("SELECT revision,attributes,asset_kind FROM physical_asset_records WHERE asset_id=$1 FOR UPDATE")
        .bind(&asset.asset_id).fetch_optional(&mut *transaction).await.map_err(|_| "asset revision lock failed")?;
    let before = prior
        .as_ref()
        .map(|row| row.try_get::<Value, _>("attributes"))
        .transpose()
        .map_err(|_| "asset prior state unavailable")?;
    if let Some(expected) = expected_revision {
        let actual = prior
            .as_ref()
            .ok_or("asset does not exist")?
            .try_get::<i64, _>("revision")
            .map_err(|_| "asset revision unavailable")?;
        if actual != expected {
            return Err("asset revision changed; reload before editing");
        }
        let kind: String = prior
            .as_ref()
            .ok_or("asset does not exist")?
            .try_get("asset_kind")
            .map_err(|_| "asset type unavailable")?;
        if kind != asset.asset.kind() {
            return Err("asset type cannot be changed");
        }
        let assigned:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM physical_resource_assignments WHERE resource_id=$1 AND status<>'RELEASED')")
            .bind(&asset.asset_id).fetch_one(&mut *transaction).await.map_err(|_| "asset assignment check failed")?;
        let dependent: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM physical_asset_dependencies WHERE target_id=$1)",
        )
        .bind(&asset.asset_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| "asset dependency check failed")?;
        if assigned || dependent {
            return Err("assigned or referenced asset cannot be changed; use lifecycle operation");
        }
    }
    validate_live(&mut transaction, asset, expected_revision.is_some()).await?;
    let state = serde_json::to_value(&asset.state)
        .map_err(|_| "asset state encoding failed")?
        .as_str()
        .ok_or("invalid asset state")?
        .to_owned();
    let attributes = serde_json::to_value(asset).map_err(|_| "asset encoding failed")?;
    let revision: i64 = if expected_revision.is_some() {
        sqlx::query_scalar("UPDATE physical_asset_records SET operational_code=$2,state=$3,attributes=$4,revision=revision+1,updated_by=$5,updated_at=now() WHERE asset_id=$1 RETURNING revision")
            .bind(&asset.asset_id).bind(&asset.operational_code).bind(&state).bind(&attributes).bind(actor.principal_id)
            .fetch_one(&mut *transaction).await.map_err(|_| "physical asset update failed")?
    } else {
        sqlx::query_scalar("INSERT INTO physical_asset_records(asset_id,asset_kind,operational_code,state,attributes,created_by,updated_by) VALUES ($1,$2,$3,$4,$5,$6,$6) RETURNING revision")
            .bind(&asset.asset_id).bind(asset.asset.kind()).bind(&asset.operational_code).bind(&state).bind(&attributes).bind(actor.principal_id)
            .fetch_one(&mut *transaction).await.map_err(|_| "physical asset creation failed")?
    };
    sqlx::query("DELETE FROM physical_asset_dependencies WHERE asset_id=$1")
        .bind(&asset.asset_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| "asset dependencies update failed")?;
    for (target, _) in asset.asset.references() {
        sqlx::query("INSERT INTO physical_asset_dependencies(asset_id,target_id) VALUES ($1,$2) ON CONFLICT DO NOTHING")
            .bind(&asset.asset_id).bind(target).execute(&mut *transaction).await.map_err(|_| "asset dependency insert failed")?;
    }
    let result = json!({"asset_id":asset.asset_id,"revision":revision,"status":"APPLIED","before":before,"after":asset});
    operations::finish_operation(
        &mut transaction,
        actor,
        operation_id,
        tool,
        asset.asset.kind(),
        &asset.asset_id,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((transaction, result))
}

pub async fn dry_run(
    pool: &PgPool,
    asset: &PhysicalAsset,
    updating: bool,
) -> Result<Value, &'static str> {
    let mut transaction = operations::begin_transaction(pool).await?;
    sqlx::query("SET TRANSACTION READ ONLY")
        .execute(&mut *transaction)
        .await
        .map_err(|_| "read-only transaction unavailable")?;
    let result = validate_live(&mut transaction, asset, updating).await;
    transaction
        .rollback()
        .await
        .map_err(|_| "dry-run rollback failed")?;
    Ok(match result {
        Ok(()) => json!({"status":"PASS","persisted":false}),
        Err(reason) => json!({"status":"DENY","reason":reason,"persisted":false}),
    })
}
