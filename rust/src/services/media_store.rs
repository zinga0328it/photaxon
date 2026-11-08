use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{
    error::Error,
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_PHOTO_BYTES: usize = 20 * 1024 * 1024;

#[derive(Debug)]
pub enum MediaStoreError {
    InvalidMetadata,
    Filesystem,
    Database,
}

#[derive(Clone, Debug)]
pub struct StoredMedia {
    pub media_id: String,
    pub storage_key: String,
    pub path: PathBuf,
}

impl fmt::Display for MediaStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidMetadata => "photo metadata or size is invalid",
            Self::Filesystem => "private photo storage unavailable",
            Self::Database => "photo catalog write failed",
        })
    }
}

impl Error for MediaStoreError {}

pub struct PhotoMetadata<'a> {
    pub workflow_id: &'a str,
    pub work_order_id: &'a str,
    pub technician_id: &'a str,
    pub resource_type: &'a str,
    pub resource_ref: &'a str,
    pub purpose: &'a str,
    pub content_type: &'a str,
}

pub async fn list_work_order_photos(
    pool: &PgPool,
    work_order_id: &str,
) -> Result<Vec<serde_json::Value>, MediaStoreError> {
    if work_order_id.is_empty()
        || work_order_id.len() > 128
        || !work_order_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(MediaStoreError::InvalidMetadata);
    }
    let admin_catalog:bool=sqlx::query_scalar("SELECT to_regclass('public.admin_photo_catalog') IS NOT NULL")
        .fetch_one(pool).await.map_err(|_| MediaStoreError::Database)?;
    let query=if admin_catalog {
        "SELECT data FROM (SELECT captured_at,media_id,jsonb_build_object('media_id',media_id,'work_order_id',work_order_id,'resource_type',resource_type,'resource_ref',resource_ref,'purpose',purpose,'storage_key',storage_key,'content_type',content_type,'byte_size',byte_size,'sha256',sha256,'captured_at',captured_at,'author',captured_by_technician) AS data FROM field_operation_media WHERE work_order_id=$1 UNION ALL SELECT v.created_at,c.media_id,jsonb_build_object('media_id',c.media_id,'work_order_id',c.work_order_id,'resource_type',c.asset_type,'resource_ref',c.asset_id,'purpose',c.evidence_type,'storage_key',v.storage_key,'content_type',v.content_type,'byte_size',v.byte_size,'sha256',v.sha256,'captured_at',v.created_at,'author',v.created_by,'version_id',v.version_id,'notes',c.notes) FROM admin_photo_catalog c JOIN admin_photo_versions v ON v.media_id=c.media_id AND v.version_id=c.current_version_id WHERE c.work_order_id=$1) item ORDER BY captured_at,media_id LIMIT 200"
    } else {
        "SELECT jsonb_build_object('media_id',media_id,'work_order_id',work_order_id,'resource_type',resource_type,'resource_ref',resource_ref,'purpose',purpose,'storage_key',storage_key,'content_type',content_type,'byte_size',byte_size,'sha256',sha256,'captured_at',captured_at,'author',captured_by_technician) FROM field_operation_media WHERE work_order_id=$1 ORDER BY captured_at,media_id LIMIT 200"
    };
    sqlx::query_scalar::<_, serde_json::Value>(query)
    .bind(work_order_id)
    .fetch_all(pool)
    .await
    .map_err(|_| MediaStoreError::Database)
}

pub async fn store_telegram_photo(
    pool: &PgPool,
    metadata: PhotoMetadata<'_>,
    bytes: Vec<u8>,
) -> Result<StoredMedia, MediaStoreError> {
    let PhotoMetadata {
        workflow_id,
        work_order_id,
        technician_id,
        resource_type,
        resource_ref,
        purpose,
        content_type,
    } = metadata;
    if bytes.is_empty()
        || bytes.len() > MAX_PHOTO_BYTES
        || work_order_id.trim().is_empty()
        || work_order_id.len() > 128
        || !work_order_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || resource_ref.trim().is_empty()
        || !matches!(resource_type, "ROE" | "ONT" | "MODEM")
        || !matches!(purpose, "ROE_INSTALLATION" | "ONT_LABEL" | "MODEM_DELIVERY")
        || !matches!(content_type, "image/jpeg" | "image/png" | "image/webp")
    {
        return Err(MediaStoreError::InvalidMetadata);
    }
    let media_id = Uuid::new_v4().to_string();
    let storage_key = format!("{work_order_id}/{media_id}.jpg");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(MediaStoreError::Filesystem)?
        .join("secrets/field-media");
    let bytes_len = bytes.len() as i64;
    let digest = encode_hex(&Sha256::digest(&bytes));
    let stored_path = root.join(&storage_key);
    let path_for_write = stored_path.clone();
    tokio::task::spawn_blocking(move || write_private_file(&root, &path_for_write, &bytes))
        .await
        .map_err(|_| MediaStoreError::Filesystem)??;

    let result = async {
        let mut transaction = pool.begin().await.map_err(|_| MediaStoreError::Database)?;
        let inserted = sqlx::query("INSERT INTO field_operation_media (media_id,work_order_id,workflow_id,resource_type,resource_ref,purpose,storage_key,content_type,byte_size,sha256,captured_by_technician,captured_at) SELECT $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,now() WHERE EXISTS (SELECT 1 FROM telegram_workflows w JOIN field_technician_sessions s ON s.session_id=w.session_id WHERE w.workflow_id=$3 AND w.work_order_id=$2 AND w.technician_id=$11 AND s.status='ACTIVE' AND s.expires_at>now())")
            .bind(&media_id)
            .bind(work_order_id)
            .bind(workflow_id)
            .bind(resource_type)
            .bind(resource_ref)
            .bind(purpose)
            .bind(&storage_key)
            .bind(content_type)
            .bind(bytes_len)
            .bind(digest)
            .bind(technician_id)
            .execute(&mut *transaction)
            .await
            .map_err(|_| MediaStoreError::Database)?;
        if inserted.rows_affected() != 1 {
            return Err(MediaStoreError::Database);
        }
        transaction.commit().await.map_err(|_| MediaStoreError::Database)
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&stored_path);
    }
    result.map(|_| StoredMedia {
        media_id,
        storage_key,
        path: stored_path,
    })
}

pub async fn update_resource_ref(
    pool: &PgPool,
    work_order_id: &str,
    media_id: &str,
    resource_ref: &str,
) -> Result<(), MediaStoreError> {
    if resource_ref.trim().is_empty() || resource_ref.len() > 128 {
        return Err(MediaStoreError::InvalidMetadata);
    }
    let updated = sqlx::query(
        "UPDATE field_operation_media SET resource_ref=$3 WHERE media_id=$1 AND work_order_id=$2",
    )
    .bind(media_id)
    .bind(work_order_id)
    .bind(resource_ref)
    .execute(pool)
    .await
    .map_err(|_| MediaStoreError::Database)?;
    if updated.rows_affected() != 1 {
        return Err(MediaStoreError::Database);
    }
    Ok(())
}

fn write_private_file(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), MediaStoreError> {
    fs::create_dir_all(root).map_err(|_| MediaStoreError::Filesystem)?;
    let metadata = fs::symlink_metadata(root).map_err(|_| MediaStoreError::Filesystem)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MediaStoreError::Filesystem);
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))
        .map_err(|_| MediaStoreError::Filesystem)?;
    let parent = path.parent().ok_or(MediaStoreError::Filesystem)?;
    fs::create_dir_all(parent).map_err(|_| MediaStoreError::Filesystem)?;
    let parent_metadata = fs::symlink_metadata(parent).map_err(|_| MediaStoreError::Filesystem)?;
    if parent_metadata.file_type().is_symlink() || !parent_metadata.is_dir() {
        return Err(MediaStoreError::Filesystem);
    }
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|_| MediaStoreError::Filesystem)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| MediaStoreError::Filesystem)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| MediaStoreError::Filesystem)
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
