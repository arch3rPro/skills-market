use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

        assert_eq!(settings.base_url, "https://dav.example.com/remote.php/dav/files/me/");
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
}
