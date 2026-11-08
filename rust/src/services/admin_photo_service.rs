use crate::{mcp::auth::ControlPlaneAdminContext, services::control_plane_admin_service};
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PhotoAssetType {
    Cabinet,
    Splitter,
    Roe,
    Pte,
    Ont,
    Modem,
    Feeder,
    Closure,
    Cable,
    Tube,
    Fiber,
    TerminalBlock,
    Port,
    Riser,
    FloorDistribution,
    Termination,
    Oto,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
pub struct PhotoAssociation {
    pub work_order_id: String,
    pub asset_type: PhotoAssetType,
    pub asset_id: String,
    pub evidence_type: String,
    pub notes: Option<String>,
}

pub fn validate(
    association: &PhotoAssociation,
    content_type: &str,
    bytes: &[u8],
) -> Result<(), &'static str> {
    if association.work_order_id.is_empty()
        || association.work_order_id.len() > 128
        || !association
            .work_order_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || association.asset_id.is_empty()
        || association.asset_id.len() > 128
        || association.asset_id.chars().any(char::is_control)
        || association.evidence_type.is_empty()
        || association.evidence_type.len() > 128
        || association.evidence_type.chars().any(char::is_control)
        || association
            .notes
            .as_ref()
            .is_some_and(|notes| notes.len() > 2048 || notes.chars().any(char::is_control))
        || bytes.is_empty()
        || bytes.len() > 7 * 1024 * 1024
    {
        return Err("invalid WR photo metadata or size (maximum 7 MiB)");
    }
    let mime = if bytes.starts_with(&[255, 216, 255]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        return Err("unsupported photo bytes");
    };
    if mime != content_type {
        return Err("photo MIME does not match bytes");
    }
    Ok(())
}

pub struct PhotoWrite<'a> {
    pub operation_id: &'a str,
    pub association: &'a PhotoAssociation,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
    pub previous_storage_key: Option<&'a str>,
}

pub async fn write_photo(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    root: &Path,
    request: PhotoWrite<'_>,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let transaction = control_plane_admin_service::begin_transaction(pool).await?;
    let (transaction, result) = write_photo_transaction(transaction, actor, root, request).await?;
    transaction.commit().await.map_err(
        |_| "photo commit acknowledgement failed; private file retained for reconciliation",
    )?;
    Ok(result)
}

pub async fn write_photo_transaction<'tx>(
    mut transaction: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    root: &Path,
    request: PhotoWrite<'_>,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    let PhotoWrite {
        operation_id,
        association,
        content_type,
        bytes,
        previous_storage_key,
    } = request;
    actor.require_write_authorization()?;
    validate(association, content_type, bytes)?;
    let hash = format!("{:x}", Sha256::digest(bytes));
    let payload = json!({"association":association,"content_type":content_type,"sha256":hash,"previous_storage_key":previous_storage_key});
    let digest = control_plane_admin_service::input_digest(&payload)?;
    let tool = if previous_storage_key.is_some() {
        "replace_ragno_image"
    } else {
        "upload_ragno_image"
    };
    if let Some(result) = control_plane_admin_service::begin_operation(
        &mut transaction,
        actor,
        operation_id,
        tool,
        &digest,
    )
    .await?
    {
        return Ok((transaction, result));
    }
    let asset_type = validate_association(&mut transaction, association).await?;
    let media_id = if let Some(previous) = previous_storage_key {
        let row = sqlx::query("SELECT c.media_id,c.work_order_id,c.asset_type,c.asset_id FROM admin_photo_catalog c JOIN admin_photo_versions v ON v.version_id=c.current_version_id AND v.media_id=c.media_id WHERE v.storage_key=$1 FOR UPDATE OF c")
            .bind(previous).fetch_optional(&mut *transaction).await.map_err(|_| "photo current version lookup failed")?
            .ok_or("only the current cataloged photo version can be replaced")?;
        if row
            .try_get::<String, _>("work_order_id")
            .map_err(|_| "photo WR unavailable")?
            != association.work_order_id
            || row
                .try_get::<String, _>("asset_id")
                .map_err(|_| "photo asset unavailable")?
                != association.asset_id
            || row
                .try_get::<String, _>("asset_type")
                .map_err(|_| "photo asset unavailable")?
                != asset_type
        {
            return Err("replacement cannot change WR or asset association");
        }
        row.try_get::<String, _>("media_id")
            .map_err(|_| "photo identity unavailable")?
    } else {
        let media_id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO admin_photo_catalog(media_id,work_order_id,asset_type,asset_id,evidence_type,notes,created_by) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(&media_id).bind(&association.work_order_id).bind(&asset_type).bind(&association.asset_id)
            .bind(&association.evidence_type).bind(&association.notes).bind(actor.principal_id)
            .execute(&mut *transaction).await.map_err(|_| "photo catalog insert failed")?;
        media_id
    };
    let version_id = Uuid::new_v4().to_string();
    let extension = match content_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => return Err("invalid MIME"),
    };
    let storage_key = format!(
        "{}/{}-{}.{}",
        association.work_order_id, media_id, version_id, extension
    );
    sqlx::query("INSERT INTO admin_photo_versions(version_id,media_id,storage_key,content_type,byte_size,sha256,created_by) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(&version_id).bind(&media_id).bind(&storage_key).bind(content_type).bind(bytes.len() as i64).bind(&hash).bind(actor.principal_id)
        .execute(&mut *transaction).await.map_err(|_| "photo version insert failed")?;
    sqlx::query("UPDATE admin_photo_catalog SET current_version_id=$2,evidence_type=$3,notes=$4 WHERE media_id=$1")
        .bind(&media_id).bind(&version_id).bind(&association.evidence_type).bind(&association.notes)
        .execute(&mut *transaction).await.map_err(|_| "photo current version update failed")?;
    let result = json!({"status":"COMMITTED","media_id":media_id,"version_id":version_id,"work_order_id":association.work_order_id,
        "path":format!("secrets/field-media/{storage_key}"),"storage_key":storage_key,"sha256":hash,"previous_storage_key":previous_storage_key});
    control_plane_admin_service::finish_operation(
        &mut transaction,
        actor,
        operation_id,
        tool,
        "PHOTO",
        &media_id,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    write_immutable(root, &storage_key, bytes)?;
    Ok((transaction, result))
}

async fn validate_association(
    tx: &mut Transaction<'_, Postgres>,
    association: &PhotoAssociation,
) -> Result<String, &'static str> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM activation_requests WHERE wr=$1")
        .bind(&association.work_order_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(|_| "AAA WR lookup failed")?;
    if count != 1 {
        return Err("WR does not exist or is ambiguous");
    }
    let legacy_query = match association.asset_type {
        PhotoAssetType::Cabinet => {
            Some("SELECT EXISTS(SELECT 1 FROM cabinets WHERE cabinet_id=$1)")
        }
        PhotoAssetType::Splitter => {
            Some("SELECT EXISTS(SELECT 1 FROM splitters WHERE splitter_id=$1)")
        }
        PhotoAssetType::Roe => Some("SELECT EXISTS(SELECT 1 FROM roes WHERE roe_id=$1)"),
        PhotoAssetType::Pte => Some("SELECT EXISTS(SELECT 1 FROM ptes WHERE pte_id=$1)"),
        PhotoAssetType::Ont => Some("SELECT EXISTS(SELECT 1 FROM onts WHERE ont_id=$1)"),
        PhotoAssetType::Modem => Some("SELECT EXISTS(SELECT 1 FROM modems WHERE modem_id=$1)"),
        PhotoAssetType::Feeder => {
            Some("SELECT EXISTS(SELECT 1 FROM feeder_cables WHERE cable_id=$1)")
        }
        _ => None,
    };
    let legacy = if let Some(query) = legacy_query {
        sqlx::query_scalar::<_, bool>(query)
            .bind(&association.asset_id)
            .fetch_one(&mut **tx)
            .await
            .map_err(|_| "photo asset lookup failed")?
    } else {
        false
    };
    let asset_type = serde_json::to_value(&association.asset_type)
        .map_err(|_| "photo asset type invalid")?
        .as_str()
        .ok_or("photo asset type invalid")?
        .to_owned();
    if !legacy {
        let kind = if asset_type == "FEEDER" {
            "CABLE"
        } else {
            &asset_type
        };
        let physical:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM physical_asset_records WHERE asset_id=$1 AND asset_kind=$2)")
            .bind(&association.asset_id).bind(kind).fetch_one(&mut **tx).await.map_err(|_|"physical photo asset lookup failed")?;
        if !physical {
            return Err("photo asset does not exist or type does not match");
        }
    }
    Ok(asset_type)
}

pub struct ExistingPhoto<'a> {
    pub operation_id: &'a str,
    pub association: &'a PhotoAssociation,
    pub storage_key: &'a str,
    pub content_type: &'a str,
    pub bytes: &'a [u8],
}

pub async fn catalog_existing(
    pool: &PgPool,
    actor: &ControlPlaneAdminContext,
    input: ExistingPhoto<'_>,
) -> Result<Value, &'static str> {
    actor.require_write_authorization()?;
    let tx = control_plane_admin_service::begin_transaction(pool).await?;
    let (tx, result) = catalog_existing_transaction(tx, actor, input).await?;
    tx.commit()
        .await
        .map_err(|_| "existing photo catalog commit failed")?;
    Ok(result)
}

pub async fn catalog_existing_transaction<'tx>(
    mut tx: Transaction<'tx, Postgres>,
    actor: &ControlPlaneAdminContext,
    input: ExistingPhoto<'_>,
) -> Result<(Transaction<'tx, Postgres>, Value), &'static str> {
    let ExistingPhoto {
        operation_id,
        association,
        storage_key,
        content_type,
        bytes,
    } = input;
    validate(association, content_type, bytes)?;
    let path = Path::new(storage_key);
    if path.components().count() != 2
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("existing photo path must be folder/filename");
    }
    let hash = format!("{:x}", Sha256::digest(bytes));
    let payload = json!({"association":association,"storage_key":storage_key,"sha256":hash,"content_type":content_type});
    let digest = control_plane_admin_service::input_digest(&payload)?;
    if let Some(result) = control_plane_admin_service::begin_operation(
        &mut tx,
        actor,
        operation_id,
        "catalog_existing_ragno_image",
        &digest,
    )
    .await?
    {
        return Ok((tx, result));
    }
    let asset_type = validate_association(&mut tx, association).await?;
    let found=sqlx::query("SELECT v.media_id,c.work_order_id,c.asset_type,c.asset_id,v.sha256 FROM admin_photo_versions v JOIN admin_photo_catalog c ON c.media_id=v.media_id WHERE v.storage_key=$1 UNION ALL SELECT media_id,work_order_id,resource_type,resource_ref,sha256 FROM field_operation_media WHERE storage_key=$1")
        .bind(storage_key).fetch_all(&mut *tx).await.map_err(|_|"existing photo catalog lookup failed")?;
    if found.len() > 1 {
        return Err("photo already has ambiguous catalog entries; reconcile manually");
    }
    if let Some(existing) = found.first() {
        if existing
            .try_get::<String, _>("work_order_id")
            .map_err(|_| "photo WR unavailable")?
            != association.work_order_id
            || existing
                .try_get::<String, _>("asset_type")
                .map_err(|_| "photo asset unavailable")?
                != asset_type
            || existing
                .try_get::<String, _>("asset_id")
                .map_err(|_| "photo asset unavailable")?
                != association.asset_id
            || existing
                .try_get::<String, _>("sha256")
                .map_err(|_| "photo hash unavailable")?
                != hash
        {
            return Err("existing photo catalog differs from declared WR/asset/bytes");
        }
        let result = json!({"status":"ALREADY_CATALOGED","storage_key":storage_key,"media_id":existing.try_get::<String,_>("media_id").map_err(|_|"photo ID unavailable")?});
        control_plane_admin_service::finish_operation(
            &mut tx,
            actor,
            operation_id,
            "catalog_existing_ragno_image",
            "PHOTO",
            storage_key,
            &digest,
            payload,
            json!({"status":"PASS"}),
            result.clone(),
        )
        .await?;
        return Ok((tx, result));
    }
    let media = Uuid::new_v4().to_string();
    let version = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO admin_photo_catalog(media_id,work_order_id,asset_type,asset_id,evidence_type,notes,created_by) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(&media).bind(&association.work_order_id).bind(asset_type).bind(&association.asset_id).bind(&association.evidence_type).bind(&association.notes).bind(actor.principal_id)
        .execute(&mut *tx).await.map_err(|_|"existing photo association save failed")?;
    sqlx::query("INSERT INTO admin_photo_versions(version_id,media_id,storage_key,content_type,byte_size,sha256,created_by) VALUES ($1,$2,$3,$4,$5,$6,$7)")
        .bind(&version).bind(&media).bind(storage_key).bind(content_type).bind(bytes.len() as i64).bind(&hash).bind(actor.principal_id)
        .execute(&mut *tx).await.map_err(|_|"existing photo version save failed")?;
    sqlx::query("UPDATE admin_photo_catalog SET current_version_id=$2 WHERE media_id=$1")
        .bind(&media)
        .bind(&version)
        .execute(&mut *tx)
        .await
        .map_err(|_| "existing photo version update failed")?;
    let result = json!({"status":"CATALOGED","storage_key":storage_key,"media_id":media,"work_order_id":association.work_order_id,"sha256":hash,"existing_bytes_modified":false});
    control_plane_admin_service::finish_operation(
        &mut tx,
        actor,
        operation_id,
        "catalog_existing_ragno_image",
        "PHOTO",
        &media,
        &digest,
        payload,
        json!({"status":"PASS"}),
        result.clone(),
    )
    .await?;
    Ok((tx, result))
}

pub async fn catalog_status(pool: &PgPool, storage_key: &str) -> Result<Value, &'static str> {
    let rows:Vec<Value>=sqlx::query_scalar("SELECT jsonb_build_object('media_id',v.media_id,'work_order_id',c.work_order_id,'asset_type',c.asset_type,'asset_id',c.asset_id) FROM admin_photo_versions v JOIN admin_photo_catalog c ON c.media_id=v.media_id WHERE v.storage_key=$1 UNION ALL SELECT jsonb_build_object('media_id',media_id,'work_order_id',work_order_id,'asset_type',resource_type,'asset_id',resource_ref) FROM field_operation_media WHERE storage_key=$1")
        .bind(storage_key).fetch_all(pool).await.map_err(|_|"photo reconciliation lookup failed")?;
    Ok(
        json!({"storage_key":storage_key,"catalog_status":match rows.len(){0=>"UNASSOCIATED_REQUIRES_WR_AND_ASSET",1=>"CATALOGED",_=>"AMBIGUOUS"},"associations":rows}),
    )
}

fn write_immutable(root: &Path, storage_key: &str, bytes: &[u8]) -> Result<PathBuf, &'static str> {
    let mut parent = root.to_path_buf();
    for component in ["secrets", "field-media"] {
        parent.push(component);
        if !parent.exists() {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&parent)
                .map_err(|_| "private photo folder unavailable")?;
        }
        let metadata =
            fs::symlink_metadata(&parent).map_err(|_| "private photo folder unavailable")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("invalid private photo directory");
        }
    }
    let relative = Path::new(storage_key);
    if relative
        .components()
        .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err("invalid photo path");
    }
    let directory = parent.join(relative.parent().ok_or("photo WR folder missing")?);
    if !directory.exists() {
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(|_| "photo WR directory unavailable")?;
    }
    let metadata = fs::symlink_metadata(&directory).map_err(|_| "photo WR folder unavailable")?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("invalid photo WR directory");
    }
    let path = parent.join(relative);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|_| "photo file creation failed")?;
    if file.write_all(bytes).and_then(|_| file.sync_all()).is_err() {
        let _ = fs::remove_file(&path);
        return Err("photo bytes could not be persisted");
    }
    std::fs::File::open(&directory)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| "photo directory sync failed")?;
    Ok(path)
}

pub async fn verify_catalog_bytes(
    pool: &PgPool,
    storage_key: &str,
    bytes: &[u8],
) -> Result<(), &'static str> {
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.admin_photo_versions') IS NOT NULL")
            .fetch_one(pool)
            .await
            .map_err(|_| "photo catalog unavailable")?;
    let query = if exists {
        "SELECT sha256,byte_size FROM admin_photo_versions WHERE storage_key=$1 UNION ALL SELECT sha256,byte_size FROM field_operation_media WHERE storage_key=$1"
    } else {
        "SELECT sha256,byte_size FROM field_operation_media WHERE storage_key=$1"
    };
    let records = sqlx::query(query)
        .bind(storage_key)
        .fetch_all(pool)
        .await
        .map_err(|_| "photo catalog lookup failed")?;
    if records.len() != 1 {
        return Err("photo is not uniquely cataloged on AAA");
    }
    let record = &records[0];
    let hash: String = record
        .try_get("sha256")
        .map_err(|_| "photo hash unavailable")?;
    let size: i64 = record
        .try_get("byte_size")
        .map_err(|_| "photo size unavailable")?;
    if hash != format!("{:x}", Sha256::digest(bytes)) || size != bytes.len() as i64 {
        return Err("photo bytes do not match AAA catalog");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn photo_requires_wr_asset_and_consistent_mime() {
        let mut metadata = PhotoAssociation {
            work_order_id: "WR-1".into(),
            asset_type: PhotoAssetType::Roe,
            asset_id: "ROE-1".into(),
            evidence_type: "BEFORE".into(),
            notes: None,
        };
        assert!(validate(&metadata, "image/jpeg", &[255, 216, 255, 0]).is_ok());
        assert!(validate(&metadata, "image/png", &[255, 216, 255, 0]).is_err());
        metadata.work_order_id = "../private".into();
        assert!(validate(&metadata, "image/jpeg", &[255, 216, 255, 0]).is_err());
    }
}
