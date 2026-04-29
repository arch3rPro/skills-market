use anyhow::{anyhow, bail, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use tempfile::tempdir;
use tokio::sync::Mutex;

pub mod archive;

use archive::{
    backup_current_skills, restore_skills_from_backup, restore_skills_zip, zip_central_skills,
};

use crate::core::{skill_store::SkillStore, webdav};

pub const WEBDAV_SETTINGS_KEY: &str = "webdav_sync_settings";
pub const DEFAULT_REMOTE_ROOT: &str = "skills-manager-plus-sync";
pub const DEFAULT_PROFILE: &str = "default";
pub const PROTOCOL_FORMAT: &str = "skills-manager-plus-webdav-sync";
pub const PROTOCOL_VERSION: u32 = 1;
pub const REMOTE_DATA_SQL: &str = "data.sql";
pub const REMOTE_SKILLS_ZIP: &str = "skills.zip";
pub const REMOTE_MANIFEST: &str = "manifest.json";
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
pub const MAX_SYNC_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSyncStatus {
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub last_error_source: Option<String>,
    pub last_local_manifest_hash: Option<String>,
    pub last_remote_manifest_hash: Option<String>,
    pub last_remote_etag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSyncSettings {
    pub enabled: bool,
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub remote_root: String,
    pub profile: String,
    #[serde(default)]
    pub status: WebDavSyncStatus,
}

impl Default for WebDavSyncSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: String::new(),
            username: String::new(),
            password: String::new(),
            remote_root: DEFAULT_REMOTE_ROOT.to_string(),
            profile: DEFAULT_PROFILE.to_string(),
            status: WebDavSyncStatus::default(),
        }
    }
}

impl WebDavSyncSettings {
    pub fn normalize(&mut self) {
        self.base_url = self.base_url.trim().to_string();
        self.username = self.username.trim().to_string();
        self.remote_root = normalize_remote_segment(&self.remote_root, DEFAULT_REMOTE_ROOT);
        self.profile = normalize_remote_segment(&self.profile, DEFAULT_PROFILE);
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.base_url.trim().is_empty() {
            bail!("WebDAV base URL is required when WebDAV sync is enabled");
        }
        validate_safe_remote_segment("remote_root", &self.remote_root)?;
        validate_safe_remote_segment("profile", &self.profile)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncManifest {
    pub format: String,
    pub protocol_version: u32,
    pub app_id: String,
    pub app_name: String,
    pub app_version: String,
    pub db_compat_version: u32,
    pub device_name: String,
    pub created_at: String,
    pub snapshot_id: String,
    pub artifacts: BTreeMap<String, ArtifactMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactMeta {
    pub sha256: String,
    pub size: u64,
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote_name: Option<String>,
}

pub fn validate_manifest_compat(
    manifest: &SyncManifest,
    local_db_compat_version: u32,
) -> Result<()> {
    if manifest.format != PROTOCOL_FORMAT {
        bail!("Unsupported WebDAV sync manifest format");
    }
    if manifest.protocol_version != PROTOCOL_VERSION {
        bail!("Unsupported WebDAV sync protocol version");
    }
    if manifest.app_id != "com.agentskills.skillsmanagerplus" {
        bail!("Unsupported WebDAV sync app id");
    }
    if manifest.db_compat_version > local_db_compat_version {
        bail!("WebDAV sync manifest requires a newer database version");
    }
    if !manifest.artifacts.contains_key(REMOTE_DATA_SQL) {
        bail!("WebDAV sync manifest is missing data.sql");
    }
    if !manifest.artifacts.contains_key(REMOTE_SKILLS_ZIP) {
        bail!("WebDAV sync manifest is missing skills.zip");
    }
    Ok(())
}

pub fn compute_snapshot_id(artifacts: &BTreeMap<String, ArtifactMeta>) -> String {
    let mut hasher = Sha256::new();
    for (name, artifact) in artifacts {
        hasher.update(name.as_bytes());
        hasher.update(b":");
        hasher.update(artifact.sha256.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

pub fn resolve_password_for_save(
    mut incoming: WebDavSyncSettings,
    existing: Option<WebDavSyncSettings>,
    password_touched: bool,
) -> WebDavSyncSettings {
    if !password_touched && incoming.password.is_empty() {
        if let Some(existing) = existing {
            incoming.password = existing.password;
        }
    }
    incoming
}

pub fn redact_settings_for_export(raw: &str) -> String {
    match serde_json::from_str::<WebDavSyncSettings>(raw) {
        Ok(mut settings) => {
            settings.password.clear();
            serde_json::to_string(&settings)
                .unwrap_or_else(|_| serde_json::json!(WebDavSyncSettings::default()).to_string())
        }
        Err(_) => serde_json::json!(WebDavSyncSettings::default()).to_string(),
    }
}

pub fn sync_mutex() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub async fn run_with_sync_lock<T, Fut>(operation: Fut) -> Result<T>
where
    Fut: std::future::Future<Output = Result<T>>,
{
    let _guard = sync_mutex().lock().await;
    operation.await
}

pub fn get_settings(store: &SkillStore) -> Result<Option<WebDavSyncSettings>> {
    let Some(raw) = store.get_setting(WEBDAV_SETTINGS_KEY)? else {
        return Ok(None);
    };

    Ok(Some(serde_json::from_str(&raw)?))
}

pub fn save_settings(store: &SkillStore, mut settings: WebDavSyncSettings) -> Result<()> {
    settings.normalize();
    settings.validate()?;
    store.set_setting(WEBDAV_SETTINGS_KEY, &serde_json::to_string(&settings)?)?;
    Ok(())
}

pub async fn check_connection(settings: &WebDavSyncSettings) -> Result<()> {
    let settings = normalized_settings(settings)?;
    let auth = auth_for_settings(&settings);
    webdav::test_connection(&settings.base_url, auth.clone()).await?;
    let segments = remote_dir_segments(&settings, current_db_compat_version());
    webdav::ensure_remote_directories(&settings.base_url, &segments, auth).await
}

pub async fn upload_snapshot(
    store: &SkillStore,
    settings: &mut WebDavSyncSettings,
) -> Result<WebDavSyncResult> {
    let mut normalized = normalized_settings(settings)?;
    let auth = auth_for_settings(&normalized);
    let db_compat = current_db_compat_version();
    let segments = remote_dir_segments(&normalized, db_compat);
    webdav::ensure_remote_directories(&normalized.base_url, &segments, auth.clone()).await?;

    let snapshot = build_local_snapshot(store, db_compat)?;
    let artifact_segments =
        artifact_upload_dir_segments(&normalized, db_compat, &snapshot.upload_id);
    webdav::ensure_remote_directories(&normalized.base_url, &artifact_segments, auth.clone())
        .await?;
    let data_remote_name = snapshot_remote_name(&snapshot.manifest, REMOTE_DATA_SQL)?.to_string();
    let skills_remote_name =
        snapshot_remote_name(&snapshot.manifest, REMOTE_SKILLS_ZIP)?.to_string();
    webdav::put_bytes(
        &remote_file_url(&normalized, db_compat, &data_remote_name)?,
        auth.clone(),
        snapshot.data_sql,
        "application/sql",
    )
    .await?;
    webdav::put_bytes(
        &remote_file_url(&normalized, db_compat, &skills_remote_name)?,
        auth.clone(),
        snapshot.skills_zip,
        "application/zip",
    )
    .await?;

    let manifest_url = remote_file_url(&normalized, db_compat, REMOTE_MANIFEST)?;
    webdav::put_bytes(
        &manifest_url,
        auth.clone(),
        snapshot.manifest_bytes,
        "application/json",
    )
    .await?;
    let etag = webdav::head_etag(&manifest_url, auth).await.ok().flatten();

    normalized.status.last_sync_at = Some(Utc::now().timestamp());
    normalized.status.last_error = None;
    normalized.status.last_error_source = None;
    normalized.status.last_local_manifest_hash = Some(snapshot.manifest_hash.clone());
    normalized.status.last_remote_manifest_hash = Some(snapshot.manifest_hash);
    normalized.status.last_remote_etag = etag;
    save_settings(store, normalized.clone())?;
    *settings = normalized;

    Ok(WebDavSyncResult {
        status: "uploaded".to_string(),
        remote_path: remote_dir_display(settings, db_compat),
        warning: None,
    })
}

pub async fn download_snapshot(
    store: &SkillStore,
    settings: &mut WebDavSyncSettings,
) -> Result<WebDavSyncResult> {
    let mut normalized = normalized_settings(settings)?;
    let auth = auth_for_settings(&normalized);
    let db_compat = current_db_compat_version();
    let remote = fetch_remote_snapshot(&normalized, &auth, db_compat)
        .await?
        .ok_or_else(|| anyhow!("No downloadable WebDAV snapshot found"))?;

    validate_manifest_compat(&remote.manifest, db_compat)?;
    let data_sql = download_and_verify(
        &normalized,
        &auth,
        db_compat,
        REMOTE_DATA_SQL,
        &remote.manifest,
    )
    .await?;
    let skills_zip = download_and_verify(
        &normalized,
        &auth,
        db_compat,
        REMOTE_SKILLS_ZIP,
        &remote.manifest,
    )
    .await?;
    apply_snapshot(store, &data_sql, &skills_zip)?;

    let manifest_hash = sha256_hex(&remote.manifest_bytes);
    normalized.status.last_sync_at = Some(Utc::now().timestamp());
    normalized.status.last_error = None;
    normalized.status.last_error_source = None;
    normalized.status.last_local_manifest_hash = Some(manifest_hash.clone());
    normalized.status.last_remote_manifest_hash = Some(manifest_hash);
    normalized.status.last_remote_etag = remote.manifest_etag;
    save_settings(store, normalized.clone())?;
    *settings = normalized;

    Ok(WebDavSyncResult {
        status: "downloaded".to_string(),
        remote_path: remote_dir_display(settings, db_compat),
        warning: None,
    })
}

pub async fn fetch_remote_info(settings: &WebDavSyncSettings) -> Result<RemoteSnapshotInfo> {
    let settings = normalized_settings(settings)?;
    let auth = auth_for_settings(&settings);
    let db_compat = current_db_compat_version();
    let Some(remote) = fetch_remote_snapshot(&settings, &auth, db_compat).await? else {
        return Ok(RemoteSnapshotInfo {
            empty: true,
            compatible: false,
            device_name: None,
            created_at: None,
            protocol_version: None,
            db_compat_version: None,
            snapshot_id: None,
            remote_path: Some(remote_dir_display(&settings, db_compat)),
            artifacts: Vec::new(),
        });
    };

    let compatible = validate_manifest_compat(&remote.manifest, db_compat).is_ok();
    Ok(RemoteSnapshotInfo {
        empty: false,
        compatible,
        device_name: Some(remote.manifest.device_name),
        created_at: Some(remote.manifest.created_at),
        protocol_version: Some(remote.manifest.protocol_version),
        db_compat_version: Some(remote.manifest.db_compat_version),
        snapshot_id: Some(remote.manifest.snapshot_id),
        remote_path: Some(remote_dir_display(&settings, db_compat)),
        artifacts: remote.manifest.artifacts.keys().cloned().collect(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSyncResult {
    pub status: String,
    pub remote_path: String,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSnapshotInfo {
    pub empty: bool,
    pub compatible: bool,
    pub device_name: Option<String>,
    pub created_at: Option<String>,
    pub protocol_version: Option<u32>,
    pub db_compat_version: Option<u32>,
    pub snapshot_id: Option<String>,
    pub remote_path: Option<String>,
    pub artifacts: Vec<String>,
}

struct LocalSnapshot {
    data_sql: Vec<u8>,
    skills_zip: Vec<u8>,
    upload_id: String,
    manifest: SyncManifest,
    manifest_bytes: Vec<u8>,
    manifest_hash: String,
}

struct RemoteSnapshot {
    manifest: SyncManifest,
    manifest_bytes: Vec<u8>,
    manifest_etag: Option<String>,
}

pub fn current_db_compat_version() -> u32 {
    1
}

pub fn remote_dir_segments(settings: &WebDavSyncSettings, db_compat_version: u32) -> Vec<String> {
    let mut segments = Vec::new();
    segments.extend(webdav::path_segments(&settings.remote_root).map(str::to_string));
    segments.push(format!("v{PROTOCOL_VERSION}"));
    segments.push(format!("db-v{db_compat_version}"));
    segments.extend(webdav::path_segments(&settings.profile).map(str::to_string));
    segments
}

fn artifact_dir_segments(settings: &WebDavSyncSettings, db_compat_version: u32) -> Vec<String> {
    let mut segments = remote_dir_segments(settings, db_compat_version);
    segments.push("artifacts".to_string());
    segments
}

fn artifact_upload_dir_segments(
    settings: &WebDavSyncSettings,
    db_compat_version: u32,
    upload_id: &str,
) -> Vec<String> {
    let mut segments = artifact_dir_segments(settings, db_compat_version);
    segments.push(upload_id.to_string());
    segments
}

fn remote_file_url(
    settings: &WebDavSyncSettings,
    db_compat_version: u32,
    file_name: &str,
) -> Result<String> {
    let mut segments = remote_dir_segments(settings, db_compat_version);
    segments.extend(webdav::path_segments(file_name).map(str::to_string));
    webdav::build_remote_url(&settings.base_url, &segments)
}

fn remote_dir_display(settings: &WebDavSyncSettings, db_compat_version: u32) -> String {
    format!(
        "/{}",
        remote_dir_segments(settings, db_compat_version).join("/")
    )
}

fn validate_artifact_size_limit(name: &str, size: u64) -> Result<()> {
    if size > MAX_SYNC_ARTIFACT_BYTES {
        bail!("{name} exceeds WebDAV sync artifact size limit");
    }
    Ok(())
}

fn normalized_settings(settings: &WebDavSyncSettings) -> Result<WebDavSyncSettings> {
    let mut normalized = settings.clone();
    normalized.normalize();
    normalized.validate()?;
    Ok(normalized)
}

fn auth_for_settings(settings: &WebDavSyncSettings) -> webdav::WebDavAuth {
    let password = if settings.password.is_empty() {
        None
    } else {
        Some(settings.password.as_str())
    };
    webdav::auth_from_credentials(&settings.username, password)
}

fn artifact_remote_name(upload_id: &str, logical_name: &str, sha256: &str) -> String {
    format!("artifacts/{upload_id}/{sha256}-{logical_name}")
}

fn resolve_artifact_remote_name<'a>(logical_name: &'a str, meta: &'a ArtifactMeta) -> &'a str {
    meta.remote_name.as_deref().unwrap_or(logical_name)
}

fn snapshot_remote_name(manifest: &SyncManifest, logical_name: &str) -> Result<String> {
    let meta = manifest
        .artifacts
        .get(logical_name)
        .ok_or_else(|| anyhow!("Manifest missing artifact {logical_name}"))?;
    Ok(resolve_artifact_remote_name(logical_name, meta).to_string())
}

fn generate_upload_id() -> String {
    static UPLOAD_COUNTER: AtomicU64 = AtomicU64::new(0);
    let now_ms = Utc::now().timestamp_millis();
    let counter = UPLOAD_COUNTER.fetch_add(1, Ordering::Relaxed);
    let seed = format!("{now_ms}-{}-{counter}", std::process::id());
    let hash = sha256_hex(seed.as_bytes());
    format!("{now_ms}-{}", &hash[..12])
}

fn build_local_snapshot(store: &SkillStore, db_compat_version: u32) -> Result<LocalSnapshot> {
    let data_sql = store.export_data_sql_string()?.into_bytes();
    validate_artifact_size_limit(REMOTE_DATA_SQL, data_sql.len() as u64)?;

    let tmp = tempdir()?;
    let skills_zip_path = tmp.path().join(REMOTE_SKILLS_ZIP);
    zip_central_skills(&skills_zip_path)?;
    let skills_zip = std::fs::read(&skills_zip_path)?;
    validate_artifact_size_limit(REMOTE_SKILLS_ZIP, skills_zip.len() as u64)?;

    let upload_id = generate_upload_id();
    let data_hash = sha256_hex(&data_sql);
    let skills_hash = sha256_hex(&skills_zip);
    let mut artifacts = BTreeMap::new();
    artifacts.insert(
        REMOTE_DATA_SQL.to_string(),
        ArtifactMeta {
            sha256: data_hash.clone(),
            size: data_sql.len() as u64,
            content_type: "application/sql".to_string(),
            remote_name: Some(artifact_remote_name(
                &upload_id,
                REMOTE_DATA_SQL,
                &data_hash,
            )),
        },
    );
    artifacts.insert(
        REMOTE_SKILLS_ZIP.to_string(),
        ArtifactMeta {
            sha256: skills_hash.clone(),
            size: skills_zip.len() as u64,
            content_type: "application/zip".to_string(),
            remote_name: Some(artifact_remote_name(
                &upload_id,
                REMOTE_SKILLS_ZIP,
                &skills_hash,
            )),
        },
    );

    let manifest = SyncManifest {
        format: PROTOCOL_FORMAT.to_string(),
        protocol_version: PROTOCOL_VERSION,
        app_id: "com.agentskills.skillsmanagerplus".to_string(),
        app_name: "Skills-Manager-Plus".to_string(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        db_compat_version,
        device_name: detect_device_name(),
        created_at: Utc::now().to_rfc3339(),
        snapshot_id: compute_snapshot_id(&artifacts),
        artifacts,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let manifest_hash = sha256_hex(&manifest_bytes);

    Ok(LocalSnapshot {
        data_sql,
        skills_zip,
        upload_id,
        manifest,
        manifest_bytes,
        manifest_hash,
    })
}

async fn fetch_remote_snapshot(
    settings: &WebDavSyncSettings,
    auth: &webdav::WebDavAuth,
    db_compat_version: u32,
) -> Result<Option<RemoteSnapshot>> {
    let url = remote_file_url(settings, db_compat_version, REMOTE_MANIFEST)?;
    let Some((manifest_bytes, manifest_etag)) =
        webdav::get_bytes(&url, auth.clone(), MAX_MANIFEST_BYTES).await?
    else {
        return Ok(None);
    };

    let manifest = serde_json::from_slice(&manifest_bytes)?;
    Ok(Some(RemoteSnapshot {
        manifest,
        manifest_bytes,
        manifest_etag,
    }))
}

async fn download_and_verify(
    settings: &WebDavSyncSettings,
    auth: &webdav::WebDavAuth,
    db_compat_version: u32,
    artifact_name: &str,
    manifest: &SyncManifest,
) -> Result<Vec<u8>> {
    let meta = manifest
        .artifacts
        .get(artifact_name)
        .ok_or_else(|| anyhow!("Manifest missing artifact {artifact_name}"))?;
    validate_artifact_size_limit(artifact_name, meta.size)?;

    let remote_name = resolve_artifact_remote_name(artifact_name, meta);
    let url = remote_file_url(settings, db_compat_version, remote_name)?;
    let (bytes, _) = webdav::get_bytes(&url, auth.clone(), MAX_SYNC_ARTIFACT_BYTES as usize)
        .await?
        .ok_or_else(|| anyhow!("Remote missing artifact {artifact_name}"))?;

    if bytes.len() as u64 != meta.size {
        bail!("Artifact size mismatch for {artifact_name}");
    }
    if sha256_hex(&bytes) != meta.sha256 {
        bail!("Artifact hash mismatch for {artifact_name}");
    }

    Ok(bytes)
}

fn apply_snapshot(store: &SkillStore, data_sql: &[u8], skills_zip: &[u8]) -> Result<()> {
    let sql = std::str::from_utf8(data_sql)?;
    let skills_backup = backup_current_skills()?;

    if let Err(restore_error) = restore_skills_zip(skills_zip) {
        restore_skills_from_backup(&skills_backup)?;
        return Err(restore_error);
    }

    if let Err(db_error) = store.import_data_sql_string(sql) {
        restore_skills_from_backup(&skills_backup)?;
        return Err(db_error);
    }

    Ok(())
}

fn detect_device_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .map(|name| name.trim().to_string())
        .ok()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Unknown Device".to_string())
}

fn normalize_remote_segment(value: &str, default_value: &str) -> String {
    let normalized = value.trim().trim_matches('/').trim().to_string();
    if normalized.is_empty() {
        default_value.to_string()
    } else {
        normalized
    }
}

fn validate_safe_remote_segment(name: &str, value: &str) -> Result<()> {
    if value.contains("..") || value.contains('\\') || value.contains('\0') {
        bail!("WebDAV {name} contains an unsafe path segment");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_artifacts(data_hash: &str, skills_hash: &str) -> BTreeMap<String, ArtifactMeta> {
        BTreeMap::from([
            (
                REMOTE_DATA_SQL.to_string(),
                ArtifactMeta {
                    sha256: data_hash.to_string(),
                    size: 123,
                    content_type: "application/sql".to_string(),
                    remote_name: None,
                },
            ),
            (
                REMOTE_SKILLS_ZIP.to_string(),
                ArtifactMeta {
                    sha256: skills_hash.to_string(),
                    size: 456,
                    content_type: "application/zip".to_string(),
                    remote_name: None,
                },
            ),
        ])
    }

    fn test_manifest_with(
        format: &str,
        protocol_version: u32,
        db_compat_version: u32,
    ) -> SyncManifest {
        SyncManifest {
            format: format.to_string(),
            protocol_version,
            app_id: "com.agentskills.skillsmanagerplus".to_string(),
            app_name: "Skills Manager Plus".to_string(),
            app_version: "1.20.2".to_string(),
            db_compat_version,
            device_name: "test-device".to_string(),
            created_at: "2026-04-29T00:00:00Z".to_string(),
            snapshot_id: "snapshot".to_string(),
            artifacts: test_artifacts("data-hash", "skills-hash"),
        }
    }

    #[test]
    fn settings_normalize_defaults_remote_root_and_profile() {
        let mut settings = WebDavSyncSettings {
            enabled: false,
            base_url: " https://dav.example.com/remote.php/dav/files/me/ ".to_string(),
            username: " alice ".to_string(),
            password: String::new(),
            remote_root: " / ".to_string(),
            profile: " /// ".to_string(),
            status: WebDavSyncStatus::default(),
        };

        settings.normalize();

        assert_eq!(
            settings.base_url,
            "https://dav.example.com/remote.php/dav/files/me/"
        );
        assert_eq!(settings.username, "alice");
        assert_eq!(settings.remote_root, DEFAULT_REMOTE_ROOT);
        assert_eq!(settings.profile, DEFAULT_PROFILE);
    }

    #[test]
    fn resolve_password_preserves_existing_when_not_touched() {
        let incoming = WebDavSyncSettings {
            password: String::new(),
            ..WebDavSyncSettings::default()
        };
        let existing = WebDavSyncSettings {
            password: "stored-secret".to_string(),
            ..WebDavSyncSettings::default()
        };

        let settings = resolve_password_for_save(incoming, Some(existing), false);

        assert_eq!(settings.password, "stored-secret");
    }

    #[test]
    fn resolve_password_allows_explicit_clear_when_touched() {
        let incoming = WebDavSyncSettings {
            password: String::new(),
            ..WebDavSyncSettings::default()
        };
        let existing = WebDavSyncSettings {
            password: "stored-secret".to_string(),
            ..WebDavSyncSettings::default()
        };

        let settings = resolve_password_for_save(incoming, Some(existing), true);

        assert_eq!(settings.password, "");
    }

    #[test]
    fn redact_settings_for_export_clears_password_and_keeps_non_secret_config() {
        let settings = WebDavSyncSettings {
            enabled: true,
            base_url: "https://dav.example.com".to_string(),
            username: "alice".to_string(),
            password: "super-secret-password".to_string(),
            remote_root: "team-sync-root".to_string(),
            profile: DEFAULT_PROFILE.to_string(),
            status: WebDavSyncStatus::default(),
        };
        let redacted = redact_settings_for_export(&serde_json::to_string(&settings).unwrap());

        assert!(!redacted.contains("super-secret-password"));
        assert!(redacted.contains("https://dav.example.com"));
        assert!(redacted.contains("team-sync-root"));
    }

    #[test]
    fn validate_manifest_rejects_wrong_format() {
        let manifest = test_manifest_with("wrong-format", PROTOCOL_VERSION, 1);

        assert!(validate_manifest_compat(&manifest, 1).is_err());
    }

    #[test]
    fn validate_manifest_rejects_newer_db_version() {
        let manifest = test_manifest_with(PROTOCOL_FORMAT, PROTOCOL_VERSION, 2);

        assert!(validate_manifest_compat(&manifest, 1).is_err());
    }

    #[test]
    fn snapshot_id_changes_with_artifact_hash() {
        let original = test_artifacts("data-hash", "skills-hash");
        let changed = test_artifacts("data-hash", "changed-skills-hash");

        assert_ne!(
            compute_snapshot_id(&original),
            compute_snapshot_id(&changed)
        );
    }

    #[test]
    fn remote_dir_segments_include_protocol_db_and_profile() {
        let settings = WebDavSyncSettings {
            remote_root: "root".to_string(),
            profile: "work".to_string(),
            ..WebDavSyncSettings::default()
        };

        let segments = remote_dir_segments(&settings, 7);

        assert_eq!(segments, vec!["root", "v1", "db-v7", "work"]);
    }

    #[test]
    fn normalized_remote_dir_segments_default_blank_root_and_profile() {
        let settings = WebDavSyncSettings {
            remote_root: " / ".to_string(),
            profile: " /// ".to_string(),
            ..WebDavSyncSettings::default()
        };

        let normalized = normalized_settings(&settings).unwrap();
        let segments = remote_dir_segments(&normalized, 1);

        assert_eq!(
            segments,
            vec![DEFAULT_REMOTE_ROOT, "v1", "db-v1", DEFAULT_PROFILE]
        );
    }

    #[test]
    fn artifact_remote_name_uses_upload_id_hash_and_legacy_fallback() {
        let first = artifact_remote_name("upload-a", REMOTE_DATA_SQL, "abc123");
        let second = artifact_remote_name("upload-b", REMOTE_DATA_SQL, "abc123");
        let upload_scoped = ArtifactMeta {
            sha256: "abc123".to_string(),
            size: 7,
            content_type: "application/sql".to_string(),
            remote_name: Some(first.clone()),
        };
        let legacy = ArtifactMeta {
            sha256: "def456".to_string(),
            size: 8,
            content_type: "application/sql".to_string(),
            remote_name: None,
        };

        assert_eq!(first, "artifacts/upload-a/abc123-data.sql");
        assert_eq!(second, "artifacts/upload-b/abc123-data.sql");
        assert_ne!(first, second);
        assert_eq!(
            resolve_artifact_remote_name(REMOTE_DATA_SQL, &upload_scoped),
            "artifacts/upload-a/abc123-data.sql"
        );
        assert_eq!(
            resolve_artifact_remote_name(REMOTE_DATA_SQL, &legacy),
            REMOTE_DATA_SQL
        );
    }

    #[test]
    fn artifact_size_limit_rejects_too_large() {
        assert!(validate_artifact_size_limit("skills.zip", MAX_SYNC_ARTIFACT_BYTES + 1).is_err());
        assert!(validate_artifact_size_limit("skills.zip", MAX_SYNC_ARTIFACT_BYTES).is_ok());
    }
}
