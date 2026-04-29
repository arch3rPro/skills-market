use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub mod archive;

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
                },
            ),
            (
                REMOTE_SKILLS_ZIP.to_string(),
                ArtifactMeta {
                    sha256: skills_hash.to_string(),
                    size: 456,
                    content_type: "application/zip".to_string(),
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
}
