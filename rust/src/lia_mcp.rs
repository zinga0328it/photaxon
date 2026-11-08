use crate::{
    domain::field_ops::{CreateFieldRoe, RegisterModem, RegisterOnt},
    mcp::auth::{CONTROL_PLANE_ADMIN_PRINCIPAL, ControlPlaneAdminContext},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::ServerConfig,
    schemars::JsonSchema,
    tool, tool_handler, tool_router,
    transport::stdio,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_LOG_TAIL_BYTES: u64 = 1024 * 1024;
const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_ORDER_BYTES: usize = 128 * 1024;
const MAX_LISTED_FILES: usize = 5000;

#[derive(Clone)]
pub struct LiaMcp {
    root: PathBuf,
    pub(crate) pool: PgPool,
    pub(crate) admin: ControlPlaneAdminContext,
    tool_router: ToolRouter<Self>,
}

#[derive(Deserialize, JsonSchema)]
pub struct DirectoryArgs {
    /// Optional repository-relative directory.
    pub directory: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReadPathArgs {
    /// Repository-relative source, documentation, configuration, or log path.
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct TailLogArgs {
    /// Repository-relative log path.
    pub path: String,
    /// Number of final lines to return, capped at 500.
    pub lines: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct WriteOrdersArgs {
    /// Complete Markdown contents for docs/ORDINI_LIA.md.
    pub content: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ImageListArgs {
    /// Optional work-order folder, containing only letters, digits, '-' and '_'.
    pub work_order_id: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct WorkOrderPhotosArgs {
    pub work_order_id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ImagePathArgs {
    /// Image path relative to secrets/field-media, in WORK_ORDER/filename form.
    pub path: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct UploadImageArgs {
    pub operation_id: String,
    pub association: crate::services::admin_photo_service::PhotoAssociation,
    /// One of image/jpeg, image/png, or image/webp.
    pub content_type: String,
    /// Image bytes encoded as standard Base64.
    pub image_base64: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReplaceImageArgs {
    pub operation_id: String,
    pub association: crate::services::admin_photo_service::PhotoAssociation,
    /// Existing image path relative to secrets/field-media.
    pub path: String,
    /// One of image/jpeg, image/png, or image/webp.
    pub content_type: String,
    /// Replacement image bytes encoded as standard Base64.
    pub image_base64: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct CatalogExistingImageArgs {
    pub operation_id: String,
    pub path: String,
    pub association: crate::services::admin_photo_service::PhotoAssociation,
}

#[derive(Deserialize, JsonSchema)]
pub struct AuditArgs {
    /// Number of most recent records to return, capped at 200.
    pub limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct RoeAdminArgs {
    pub input: CreateFieldRoe,
}

#[derive(Deserialize, JsonSchema)]
pub struct OntAdminArgs {
    pub input: RegisterOnt,
}

#[derive(Deserialize, JsonSchema)]
pub struct ReplaceOntArgs {
    pub operation_id: String,
    pub old_ont_id: String,
    pub input: RegisterOnt,
}

#[derive(Deserialize, JsonSchema)]
pub struct ModemAdminArgs {
    pub input: RegisterModem,
    /// Set when replacing the currently active modem on the same ONT.
    pub replace_modem_id: Option<String>,
}

impl LiaMcp {
    pub fn new(
        root: PathBuf,
        pool: PgPool,
        admin: ControlPlaneAdminContext,
    ) -> Result<Self, std::io::Error> {
        let mut tool_router = Self::lia_tools();
        tool_router.merge(Self::physical_tools());
        tool_router.merge(Self::resource_tools());
        tool_router.merge(Self::delivery_tools());
        tool_router.merge(Self::one_tools());
        tool_router.merge(Self::legacy_tools());
        Ok(Self {
            root: root.canonicalize()?,
            pool,
            admin,
            tool_router,
        })
    }

    pub async fn serve_stdio(self) -> Result<(), Box<dyn Error>> {
        self.serve(stdio()).await?.waiting().await?;
        Ok(())
    }

    fn resolve_readable_file(
        &self,
        relative: &str,
        max_bytes: u64,
    ) -> Result<PathBuf, &'static str> {
        let path = Path::new(relative);
        let components = normal_components(path)?;
        if components.iter().any(|part| is_env_name(part)) {
            return Err("environment files are not exposed through MCP");
        }
        let in_secrets = components.first().is_some_and(|part| *part == "secrets");
        if in_secrets {
            return Err("private directories are not exposed through file tools");
        }
        if components.iter().any(|part| is_excluded_component(part)) {
            return Err("path is excluded from MCP file access");
        }
        let file_name = components.last().ok_or("path is empty")?;
        if is_private_extension(file_name) {
            return Err("private file type is not readable");
        }
        let mut candidate = self.root.clone();
        for component in &components {
            candidate.push(component);
            let metadata = fs::symlink_metadata(&candidate).map_err(|_| "file not found")?;
            if metadata.file_type().is_symlink() {
                return Err("symbolic links are not readable");
            }
        }
        let resolved = candidate.canonicalize().map_err(|_| "file not found")?;
        if !resolved.starts_with(&self.root) {
            return Err("path escapes the repository");
        }
        let metadata = fs::metadata(&resolved).map_err(|_| "file metadata unavailable")?;
        if !metadata.is_file() || metadata.len() > max_bytes {
            return Err("file is not regular text or exceeds the size limit");
        }
        Ok(resolved)
    }

    fn resolve_directory(&self, relative: &str) -> Result<PathBuf, &'static str> {
        if relative.is_empty() {
            return Ok(self.root.clone());
        }
        let components = normal_components(Path::new(relative))?;
        if components.iter().any(|part| is_excluded_component(part))
            || components.first().is_some_and(|part| *part == "secrets")
        {
            return Err("directory is excluded from MCP file access");
        }
        let mut candidate = self.root.clone();
        for component in components {
            candidate.push(component);
            if fs::symlink_metadata(&candidate)
                .map_err(|_| "directory not found")?
                .file_type()
                .is_symlink()
            {
                return Err("symbolic links are not readable");
            }
        }
        let resolved = candidate
            .canonicalize()
            .map_err(|_| "directory not found")?;
        if !resolved.starts_with(&self.root) || !resolved.is_dir() {
            return Err("path is outside the repository");
        }
        Ok(resolved)
    }

    fn collect_files(
        &self,
        directory: &Path,
        results: &mut Vec<String>,
    ) -> Result<(), std::io::Error> {
        if results.len() >= MAX_LISTED_FILES {
            return Ok(());
        }
        let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            if results.len() >= MAX_LISTED_FILES {
                break;
            }
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(&self.root).unwrap_or(Path::new(""));
            let relative_text = relative.to_string_lossy();
            let components = match normal_components(relative) {
                Ok(components) => components,
                Err(_) => continue,
            };
            if components.iter().any(|part| is_excluded_component(part)) {
                continue;
            }
            if components.first().is_some_and(|part| *part == "secrets") {
                if components.len() == 1 && file_type.is_dir() {
                    self.collect_files(&path, results)?;
                    continue;
                }
                if components.len() == 2
                    && file_type.is_file()
                    && is_env_name(components[1])
                    && self
                        .resolve_readable_file(&relative_text, MAX_TEXT_BYTES)
                        .is_ok()
                {
                    results.push(relative_text.into_owned());
                }
                continue;
            }
            if file_type.is_dir() {
                self.collect_files(&path, results)?;
            } else if file_type.is_file()
                && self
                    .resolve_readable_file(&relative_text, MAX_TEXT_BYTES)
                    .is_ok()
            {
                results.push(relative_text.into_owned());
            }
        }
        Ok(())
    }

    fn media_root(&self, create: bool) -> Result<PathBuf, &'static str> {
        let secrets = self.root.join("secrets");
        let metadata =
            fs::symlink_metadata(&secrets).map_err(|_| "private media directory unavailable")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("private media directory is invalid");
        }
        let media = secrets.join("field-media");
        if create && !media.exists() {
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&media)
                .map_err(|_| "private media directory creation failed")?;
        }
        let metadata =
            fs::symlink_metadata(&media).map_err(|_| "private media directory unavailable")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("private media directory is invalid");
        }
        let resolved = media
            .canonicalize()
            .map_err(|_| "private media directory unavailable")?;
        if !resolved.starts_with(&self.root) {
            return Err("private media directory escapes the repository");
        }
        Ok(resolved)
    }

    fn resolve_image(&self, relative: &str) -> Result<PathBuf, &'static str> {
        let components = normal_components(Path::new(relative))?;
        if components.len() != 2
            || !valid_work_order_id(components[0])
            || !is_image_name(components[1])
        {
            return Err("image path must be WORK_ORDER/filename with a supported image extension");
        }
        let media_root = self.media_root(false)?;
        let mut path = media_root;
        for component in components {
            path.push(component);
            let metadata = fs::symlink_metadata(&path).map_err(|_| "image not found")?;
            if metadata.file_type().is_symlink() {
                return Err("symbolic links are not allowed in photo storage");
            }
        }
        let resolved = path.canonicalize().map_err(|_| "image not found")?;
        let metadata = fs::metadata(&resolved).map_err(|_| "image metadata unavailable")?;
        if !resolved.starts_with(self.media_root(false)?)
            || !metadata.is_file()
            || metadata.len() > MAX_IMAGE_BYTES as u64
        {
            return Err("image is outside private photo storage");
        }
        Ok(resolved)
    }

    fn append_audit(
        &self,
        event_id: &str,
        action: &str,
        path: &str,
        status: &str,
        old_sha256: Option<&str>,
        new_sha256: Option<&str>,
    ) -> Result<(), &'static str> {
        let secrets = self.root.join("secrets");
        let metadata = fs::symlink_metadata(&secrets).map_err(|_| "audit directory unavailable")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("audit directory is invalid");
        }
        let audit_path = secrets.join("mcp-audit.jsonl");
        match fs::symlink_metadata(&audit_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("audit file must be a regular file");
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("audit file unavailable"),
        }
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(&audit_path)
            .map_err(|_| "audit file unavailable")?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| "audit permissions could not be secured")?;
        let actor = std::env::var("USER").unwrap_or_else(|_| "local-stdio".to_owned());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let entry = serde_json::json!({
            "event_id": event_id,
            "timestamp_unix": timestamp,
            "actor": actor,
            "action": action,
            "path": path,
            "status": status,
            "old_sha256": old_sha256,
            "new_sha256": new_sha256,
        });
        writeln!(file, "{entry}")
            .and_then(|_| file.sync_all())
            .map_err(|_| "audit record could not be persisted")
    }

    fn begin_audit(
        &self,
        action: &str,
        path: &str,
        old_sha256: Option<&str>,
        new_sha256: Option<&str>,
    ) -> Result<String, &'static str> {
        let event_id = Uuid::new_v4().to_string();
        self.append_audit(&event_id, action, path, "PENDING", old_sha256, new_sha256)?;
        Ok(event_id)
    }

    fn finish_audit(
        &self,
        event_id: &str,
        action: &str,
        path: &str,
        status: &str,
        old_sha256: Option<&str>,
        new_sha256: Option<&str>,
    ) -> Result<(), &'static str> {
        self.append_audit(event_id, action, path, status, old_sha256, new_sha256)
    }

    fn ensure_admin(&self) -> Result<(), rmcp::ErrorData> {
        if self.admin.principal_id == CONTROL_PLANE_ADMIN_PRINCIPAL {
            Ok(())
        } else {
            Err(rmcp::ErrorData::internal_error(
                "control-plane principal is unavailable",
                None,
            ))
        }
    }
}

#[tool_router(router = lia_tools)]
impl LiaMcp {
    #[tool(
        description = "ADMIN WRITE: replace a failed active ONT on exactly the same occupied GPON port and ROE. Preserves customer and modem association, old device/link history, audit and idempotency; never frees the port."
    )]
    async fn replace_ont(
        &self,
        Parameters(args): Parameters<ReplaceOntArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        let result = crate::services::control_plane_admin_service::replace_ont(
            &self.pool,
            &self.admin,
            &args.operation_id,
            &args.old_ont_id,
            &args.input,
        )
        .await
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }
    #[tool(
        description = "ADMIN VALIDATE: validate the required ROE identifiers, full address, GPS, feeder cable and allocation without reading or writing AAA."
    )]
    async fn validate_roe(
        &self,
        Parameters(args): Parameters<RoeAdminArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let result = match crate::services::control_plane_admin_service::validate_roe(&args.input) {
            Ok(()) => serde_json::json!({"status":"PASS","persisted":false}),
            Err(reason) => serde_json::json!({"status":"DENY","persisted":false,"reason":reason}),
        };
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN DRY-RUN: check a proposed ROE against live AAA cabinet/splitter topology and feeder capacity; no rows are written."
    )]
    async fn dry_run_roe(
        &self,
        Parameters(args): Parameters<RoeAdminArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let result =
            crate::services::control_plane_admin_service::dry_run_roe(&self.pool, &args.input)
                .await
                .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN VALIDATE: validate ONT serial, GPON port identifier, metadata and source event without reading or writing AAA."
    )]
    async fn validate_ont(
        &self,
        Parameters(args): Parameters<OntAdminArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let result = match crate::services::control_plane_admin_service::validate_ont(&args.input) {
            Ok(()) => serde_json::json!({"status":"PASS","persisted":false}),
            Err(reason) => serde_json::json!({"status":"DENY","persisted":false,"reason":reason}),
        };
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN DRY-RUN: validate an ONT proposal against live AAA ROE, GPON port occupancy, customer assignment and serial uniqueness; no rows are written."
    )]
    async fn dry_run_ont(
        &self,
        Parameters(args): Parameters<OntAdminArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let result =
            crate::services::control_plane_admin_service::dry_run_ont(&self.pool, &args.input)
                .await
                .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN VALIDATE: validate modem serial, target ONT, metadata and source event without reading or writing AAA."
    )]
    async fn validate_modem(
        &self,
        Parameters(args): Parameters<ModemAdminArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let result = match crate::services::control_plane_admin_service::validate_modem(&args.input)
        {
            Ok(()) => serde_json::json!({"status":"PASS","persisted":false}),
            Err(reason) => serde_json::json!({"status":"DENY","persisted":false,"reason":reason}),
        };
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN DRY-RUN: check an ONT/modem association or same-ONT modem replacement against live AAA; no rows are written."
    )]
    async fn dry_run_modem(
        &self,
        Parameters(args): Parameters<ModemAdminArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let result = crate::services::control_plane_admin_service::dry_run_modem(
            &self.pool,
            &args.input,
            args.replace_modem_id.as_deref(),
        )
        .await
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN READ: read the bounded live cabinet, line, splitter, port, ROE, feeder, ONT, modem and assignment inventory from the configured AAA PostgreSQL database over Yggdrasil."
    )]
    async fn get_network_inventory(&self) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let inventory = crate::db::admin_inventory::get_network_inventory(&self.pool)
            .await
            .map_err(|_| rmcp::ErrorData::internal_error("AAA inventory read failed", None))?;
        serde_json::to_string(&inventory)
            .map_err(|_| rmcp::ErrorData::internal_error("AAA inventory encoding failed", None))
    }

    #[tool(
        description = "Read-only recursive list of repository source, documentation, configuration, and logs. Build/VCS folders and private files are excluded; only .env filenames under secrets/ are listed."
    )]
    async fn list_project_files(
        &self,
        Parameters(args): Parameters<DirectoryArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        let directory = args.directory.unwrap_or_default();
        let base = self
            .resolve_directory(&directory)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        let mut files = Vec::new();
        self.collect_files(&base, &mut files)
            .map_err(|_| rmcp::ErrorData::internal_error("project file listing failed", None))?;
        Ok(files.join("\n"))
    }

    #[tool(
        description = "Read repository UTF-8 text. All .env values are masked; sensitive assignment lines are redacted in other files. secrets/ permits only .env files."
    )]
    async fn read_project_file(
        &self,
        Parameters(args): Parameters<ReadPathArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        let path = self
            .resolve_readable_file(&args.path, MAX_TEXT_BYTES)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        let contents = fs::read_to_string(path)
            .map_err(|_| rmcp::ErrorData::invalid_params("file is not UTF-8 text", None))?;
        Ok(redact_text(&contents, is_env_path(&args.path)))
    }

    #[tool(
        description = "Read the final lines of a repository log, with sensitive assignment values redacted. Reads at most the final 1 MiB and 500 lines."
    )]
    async fn tail_project_log(
        &self,
        Parameters(args): Parameters<TailLogArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        if !args.path.ends_with(".log") && !args.path.starts_with("logs/") {
            return Err(rmcp::ErrorData::invalid_params(
                "path must identify a repository log",
                None,
            ));
        }
        let path = self
            .resolve_readable_file(&args.path, u64::MAX)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        let mut file = File::open(path)
            .map_err(|_| rmcp::ErrorData::internal_error("log unavailable", None))?;
        let length = file
            .metadata()
            .map_err(|_| rmcp::ErrorData::internal_error("log metadata unavailable", None))?
            .len();
        let start = length.saturating_sub(MAX_LOG_TAIL_BYTES);
        file.seek(SeekFrom::Start(start))
            .map_err(|_| rmcp::ErrorData::internal_error("log seek failed", None))?;
        let mut text = String::new();
        file.read_to_string(&mut text)
            .map_err(|_| rmcp::ErrorData::invalid_params("log is not UTF-8 text", None))?;
        let line_limit = args.lines.unwrap_or(200).clamp(1, 500);
        let lines: Vec<_> = text.lines().rev().take(line_limit).collect();
        Ok(lines
            .into_iter()
            .rev()
            .map(redact_line)
            .collect::<Vec<_>>()
            .join("\n"))
    }

    #[tool(description = "Read Lia's Markdown task queue at docs/ORDINI_LIA.md.")]
    async fn read_lia_orders(&self) -> Result<String, rmcp::ErrorData> {
        let path = self
            .resolve_readable_file("docs/ORDINI_LIA.md", MAX_ORDER_BYTES as u64)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        fs::read_to_string(path)
            .map_err(|_| rmcp::ErrorData::internal_error("Lia orders unavailable", None))
    }

    #[tool(
        description = "Replace docs/ORDINI_LIA.md with Lia's Markdown requests. This is the only general project file writable through this MCP; changes are audited."
    )]
    async fn write_lia_orders(
        &self,
        Parameters(args): Parameters<WriteOrdersArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        if args.content.len() > MAX_ORDER_BYTES {
            return Err(rmcp::ErrorData::invalid_params(
                "orders file exceeds 128 KiB",
                None,
            ));
        }
        let relative = "docs/ORDINI_LIA.md";
        let path = self
            .safe_orders_path()
            .map_err(|error| rmcp::ErrorData::internal_error(error, None))?;
        let old_hash = fs::read(&path).ok().map(|bytes| sha256(&bytes));
        let new_hash = sha256(args.content.as_bytes());
        let event = self
            .begin_audit(
                "write_lia_orders",
                relative,
                old_hash.as_deref(),
                Some(&new_hash),
            )
            .map_err(|error| rmcp::ErrorData::internal_error(error, None))?;
        let result = atomic_replace(&path, args.content.as_bytes());
        let status = if result.is_ok() {
            "COMMITTED"
        } else {
            "FAILED"
        };
        let audit_result = self.finish_audit(
            &event,
            "write_lia_orders",
            relative,
            status,
            old_hash.as_deref(),
            Some(&new_hash),
        );
        result.map_err(|_| rmcp::ErrorData::internal_error("orders write failed", None))?;
        audit_result.map_err(|error| rmcp::ErrorData::internal_error(error, None))?;
        Ok(serde_json::json!({"status":"COMMITTED","path":relative,"sha256":new_hash}).to_string())
    }

    #[tool(
        description = "List image filenames only inside secrets/field-media. An optional work-order folder narrows the listing."
    )]
    async fn list_ragno_images(
        &self,
        Parameters(args): Parameters<ImageListArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        let root = self
            .media_root(false)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        let (directory, work_order_filter) = if let Some(work_order_id) = args.work_order_id {
            if !valid_work_order_id(&work_order_id) {
                return Err(rmcp::ErrorData::invalid_params(
                    "invalid work-order id",
                    None,
                ));
            }
            let path = root.join(&work_order_id);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|_| rmcp::ErrorData::invalid_params("photo folder not found", None))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(rmcp::ErrorData::invalid_params(
                    "photo folder is invalid",
                    None,
                ));
            }
            (path, Some(work_order_id))
        } else {
            (root.clone(), None)
        };
        let mut names = Vec::new();
        let mut folders = Vec::new();
        if let Some(work_order_id) = work_order_filter {
            folders.push((work_order_id, directory));
        } else {
            folders.push((String::new(), directory.clone()));
            for entry in fs::read_dir(&directory)
                .map_err(|_| rmcp::ErrorData::internal_error("photo listing failed", None))?
            {
                let entry = entry
                    .map_err(|_| rmcp::ErrorData::internal_error("photo listing failed", None))?;
                let file_type = entry.file_type().map_err(|_| {
                    rmcp::ErrorData::internal_error("photo metadata unavailable", None)
                })?;
                let folder = entry.file_name().to_string_lossy().into_owned();
                if file_type.is_dir() && !file_type.is_symlink() && valid_work_order_id(&folder) {
                    folders.push((folder, entry.path()));
                }
            }
        }
        for (folder, directory) in folders {
            for entry in fs::read_dir(&directory)
                .map_err(|_| rmcp::ErrorData::internal_error("photo listing failed", None))?
            {
                let entry = entry
                    .map_err(|_| rmcp::ErrorData::internal_error("photo listing failed", None))?;
                let file_type = entry.file_type().map_err(|_| {
                    rmcp::ErrorData::internal_error("photo metadata unavailable", None)
                })?;
                if file_type.is_file() && !file_type.is_symlink() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if is_image_name(&name)
                        && entry
                            .metadata()
                            .map(|metadata| metadata.len() <= MAX_IMAGE_BYTES as u64)
                            .unwrap_or(false)
                    {
                        names.push(if folder.is_empty() {
                            name
                        } else {
                            format!("{folder}/{name}")
                        });
                    }
                }
            }
        }
        names.sort();
        let total = names.len();
        names.truncate(200);
        let mut images = Vec::new();
        for name in names {
            images.push(
                crate::services::admin_photo_service::catalog_status(&self.pool, &name)
                    .await
                    .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?,
            );
        }
        Ok(
            serde_json::json!({"images":images,"total_files":total,"truncated":total>200})
                .to_string(),
        )
    }

    #[tool(
        description = "ADMIN WRITE: catalog a photo file already present privately on RAGNO without rewriting its bytes. Requires explicit existing WR, matching asset type/ID and evidence metadata. Does not infer WR or technician from filenames. Conflicting catalog entries are denied; audit and idempotency are recorded on AAA."
    )]
    async fn catalog_existing_ragno_image(
        &self,
        Parameters(args): Parameters<CatalogExistingImageArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.admin
            .require_write_authorization()
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        let path = self
            .resolve_image(&args.path)
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        let bytes = fs::read(path)
            .map_err(|_| rmcp::ErrorData::internal_error("existing photo read failed", None))?;
        let mime = detect_image_mime(&bytes).ok_or_else(|| {
            rmcp::ErrorData::invalid_params("unsupported existing image bytes", None)
        })?;
        let result = crate::services::admin_photo_service::catalog_existing(
            &self.pool,
            &self.admin,
            crate::services::admin_photo_service::ExistingPhoto {
                operation_id: &args.operation_id,
                association: &args.association,
                storage_key: &args.path,
                content_type: mime,
                bytes: &bytes,
            },
        )
        .await
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }

    #[tool(
        description = "Read the AAA photo catalog for one WR: photo ID, asset association, purpose, private storage key, MIME type, byte size, hash and capture time. Returns only cataloged photos, not arbitrary folder contents."
    )]
    async fn list_work_order_photos(
        &self,
        Parameters(args): Parameters<WorkOrderPhotosArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.ensure_admin()?;
        let photos =
            crate::services::media_store::list_work_order_photos(&self.pool, &args.work_order_id)
                .await
                .map_err(|_| {
                    rmcp::ErrorData::invalid_params(
                        "WR photo catalog unavailable or invalid WR identifier",
                        None,
                    )
                })?;
        serde_json::to_string(&photos)
            .map_err(|_| rmcp::ErrorData::internal_error("WR photo response encoding failed", None))
    }

    #[tool(
        description = "Read an image from secrets/field-media as Base64 with its detected MIME type."
    )]
    async fn read_ragno_image(
        &self,
        Parameters(args): Parameters<ImagePathArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        let path = self
            .resolve_image(&args.path)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        let bytes = fs::read(path)
            .map_err(|_| rmcp::ErrorData::internal_error("image read failed", None))?;
        if bytes.len() > MAX_IMAGE_BYTES {
            return Err(rmcp::ErrorData::invalid_params(
                "image exceeds 20 MiB",
                None,
            ));
        }
        let mime = detect_image_mime(&bytes)
            .ok_or_else(|| rmcp::ErrorData::invalid_params("unsupported image data", None))?;
        crate::services::admin_photo_service::verify_catalog_bytes(&self.pool, &args.path, &bytes)
            .await
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(
            serde_json::json!({"content_type":mime,"image_base64":STANDARD.encode(bytes)})
                .to_string(),
        )
    }

    #[tool(
        description = "ADMIN WRITE: upload a JPEG, PNG or WebP up to 7 MiB. Requires an existing WR and asset. Catalog, version, idempotency and audit are persisted on AAA; image bytes stay private on RAGNO."
    )]
    async fn upload_ragno_image(
        &self,
        Parameters(args): Parameters<UploadImageArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.admin
            .require_write_authorization()
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        let bytes = decode_image(&args.image_base64, &args.content_type)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        let result = crate::services::admin_photo_service::write_photo(
            &self.pool,
            &self.admin,
            &self.root,
            crate::services::admin_photo_service::PhotoWrite {
                operation_id: &args.operation_id,
                association: &args.association,
                content_type: &args.content_type,
                bytes: &bytes,
                previous_storage_key: None,
            },
        )
        .await
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }

    #[tool(
        description = "ADMIN WRITE: replace the current AAA-cataloged photo with a new immutable version. Existing bytes are retained; WR and asset cannot change. Use the new storage key returned in the result."
    )]
    async fn replace_ragno_image(
        &self,
        Parameters(args): Parameters<ReplaceImageArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        self.admin
            .require_write_authorization()
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        let bytes = decode_image(&args.image_base64, &args.content_type)
            .map_err(|error| rmcp::ErrorData::invalid_params(error, None))?;
        self.resolve_image(&args.path)
            .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        let result = crate::services::admin_photo_service::write_photo(
            &self.pool,
            &self.admin,
            &self.root,
            crate::services::admin_photo_service::PhotoWrite {
                operation_id: &args.operation_id,
                association: &args.association,
                content_type: &args.content_type,
                bytes: &bytes,
                previous_storage_key: Some(&args.path),
            },
        )
        .await
        .map_err(|reason| rmcp::ErrorData::invalid_params(reason, None))?;
        Ok(result.to_string())
    }

    #[tool(
        description = "Read the most recent MCP mutation audit records. The private audit file is never exposed by generic project-file tools."
    )]
    async fn read_mcp_audit(
        &self,
        Parameters(args): Parameters<AuditArgs>,
    ) -> Result<String, rmcp::ErrorData> {
        let path = self.root.join("secrets/mcp-audit.jsonl");
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| rmcp::ErrorData::invalid_params("audit file not found", None))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(rmcp::ErrorData::invalid_params(
                "audit file is invalid",
                None,
            ));
        }
        let contents = fs::read_to_string(path)
            .map_err(|_| rmcp::ErrorData::internal_error("audit file unavailable", None))?;
        let limit = args.limit.unwrap_or(50).clamp(1, 200);
        Ok(contents
            .lines()
            .rev()
            .take(limit)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

impl LiaMcp {
    fn safe_orders_path(&self) -> Result<PathBuf, &'static str> {
        let directory = self.root.join("docs");
        let metadata =
            fs::symlink_metadata(&directory).map_err(|_| "docs directory unavailable")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("docs directory is invalid");
        }
        let path = directory.join("ORDINI_LIA.md");
        if path.exists() {
            let metadata = fs::symlink_metadata(&path).map_err(|_| "orders file unavailable")?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err("orders file is invalid");
            }
        }
        Ok(path)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for LiaMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            rmcp::model::ServerCapabilities::builder()
                .enable_tools()
                .build(),
        )
    }
}

fn normal_components(path: &Path) -> Result<Vec<&str>, &'static str> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("path must be repository-relative");
    }
    path.components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str().ok_or("path is not valid UTF-8"),
            _ => Err("path traversal is not allowed"),
        })
        .collect()
}

fn is_excluded_component(name: &str) -> bool {
    if is_env_name(name) || name == "secrets" {
        return true;
    }
    matches!(
        name,
        ".git"
            | ".venv"
            | ".vscode"
            | ".idea"
            | "venv"
            | "ENV"
            | "target"
            | "node_modules"
            | "__pycache__"
            | "backups"
            | "backup"
    )
}

fn is_private_extension(name: &str) -> bool {
    [
        ".key", ".pem", ".crt", ".age", ".db", ".sqlite3", ".dump", ".bak", ".zip", ".tar.gz",
        ".sql.gz",
    ]
    .iter()
    .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
}

fn is_env_name(name: &str) -> bool {
    name == ".env" || name.ends_with(".env") || name.starts_with(".env.")
}

fn is_env_path(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_env_name)
}

fn redact_text(text: &str, environment_file: bool) -> String {
    text.lines()
        .map(|line| {
            if environment_file {
                if line.trim().is_empty() {
                    String::new()
                } else if line.trim_start().starts_with('#') {
                    "# [comment redacted]".to_owned()
                } else if let Some((key, _)) = line.split_once('=') {
                    format!("{}=[REDACTED]", key.trim())
                } else {
                    "[REDACTED]".to_owned()
                }
            } else {
                redact_line(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn redact_line(line: &str) -> String {
    let sensitive = |key: &str| {
        let key = key.to_ascii_lowercase();
        [
            "secret",
            "token",
            "password",
            "passwd",
            "credential",
            "api_key",
            "private_key",
            "authorization",
            "database_url",
            "db_url",
        ]
        .iter()
        .any(|needle| key.contains(needle))
    };
    if let Some((key, _)) = line.split_once('=')
        && sensitive(key)
    {
        return format!("{}=[REDACTED]", key.trim());
    }
    if let Some((key, _)) = line.split_once(':')
        && sensitive(key.trim_matches([' ', '\'', '"']))
    {
        return format!("{}: [REDACTED]", key.trim());
    }
    let lower = line.to_ascii_lowercase();
    if let Some(index) = lower.find("bearer ") {
        return format!("{}Bearer [REDACTED]", &line[..index]);
    }
    line.split_whitespace()
        .map(|word| {
            let token_like = word.len() >= 48
                && word.bytes().any(|byte| byte.is_ascii_alphabetic())
                && word.bytes().any(|byte| byte.is_ascii_digit());
            if token_like { "[REDACTED]" } else { word }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn valid_work_order_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_image_name(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".webp"]
        .iter()
        .any(|extension| lower.ends_with(extension))
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn extension_for_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

fn decode_image(encoded: &str, mime: &str) -> Result<Vec<u8>, &'static str> {
    if extension_for_mime(mime).is_none() {
        return Err("content type must be image/jpeg, image/png, or image/webp");
    }
    if encoded.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 + 8 {
        return Err("image exceeds 20 MiB");
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| "image_base64 is invalid")?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES || detect_image_mime(&bytes) != Some(mime)
    {
        return Err("image bytes do not match the declared content type or size");
    }
    Ok(bytes)
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<(), std::io::Error> {
    if let Ok(metadata) = fs::symlink_metadata(path)
        && (metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(std::io::Error::other(
            "replacement target is not a regular file",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("replacement parent unavailable"))?;
    let temporary = parent.join(format!(".mcp-{}.tmp", Uuid::new_v4()));
    write_new_file(&temporary, bytes)?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(temporary);
        return Err(error);
    }
    File::open(parent)?.sync_all()
}

fn sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
