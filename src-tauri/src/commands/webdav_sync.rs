use std::sync::Arc;

use tauri::State;

use crate::core::{
    error::AppError,
    skill_store::SkillStore,
    webdav_sync::{self, RemoteSnapshotInfo, WebDavSyncResult, WebDavSyncSettings},
};

fn require_enabled(settings: WebDavSyncSettings) -> Result<WebDavSyncSettings, AppError> {
    if !settings.enabled {
        return Err(AppError::invalid_input("WebDAV sync is disabled"));
    }
    Ok(settings)
}

fn validate_settings_for_save(settings: &mut WebDavSyncSettings) -> Result<(), AppError> {
    settings.normalize();
    settings
        .validate()
        .map_err(|error| AppError::invalid_input(error.to_string()))
}

fn classify_sync_error(error: anyhow::Error) -> AppError {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();

    if contains_any(
        &lower,
        &[
            "webdav",
            "http",
            "status ",
            "connection",
            "connect",
            "timed out",
            "timeout",
            "dns",
            "resolve host",
            "network",
            "request",
            "remote missing",
            "remote path",
        ],
    ) {
        AppError::network(message)
    } else if contains_any(
        &lower,
        &[
            "sqlite", "sql", "database", "db", "setting", "settings", "store", "export", "import",
        ],
    ) {
        AppError::db(message)
    } else if contains_any(
        &lower,
        &[
            "archive",
            "zip",
            "file",
            "path",
            "directory",
            "dir",
            "read",
            "write",
            "utf-8",
            "utf8",
            "backup",
            "restore",
        ],
    ) {
        AppError::io(message)
    } else {
        AppError::internal(message)
    }
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

#[tauri::command]
pub async fn webdav_sync_get_settings(
    store: State<'_, Arc<SkillStore>>,
) -> Result<Option<WebDavSyncSettings>, AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || webdav_sync::get_settings(&store).map_err(AppError::db))
        .await?
}

#[tauri::command]
pub async fn webdav_sync_save_settings(
    settings: WebDavSyncSettings,
    password_touched: bool,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || {
        let existing = webdav_sync::get_settings(&store).map_err(AppError::db)?;
        let mut resolved =
            webdav_sync::resolve_password_for_save(settings, existing, password_touched);
        validate_settings_for_save(&mut resolved)?;
        webdav_sync::save_settings(&store, resolved).map_err(AppError::db)
    })
    .await?
}

#[tauri::command]
pub async fn webdav_test_connection(
    settings: WebDavSyncSettings,
    preserve_empty_password: bool,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    let resolved = tokio::task::spawn_blocking(move || {
        let existing = webdav_sync::get_settings(&store).map_err(AppError::db)?;
        let mut resolved =
            webdav_sync::resolve_password_for_save(settings, existing, !preserve_empty_password);
        validate_settings_for_save(&mut resolved)?;
        Ok::<WebDavSyncSettings, AppError>(resolved)
    })
    .await??;

    webdav_sync::check_connection(&resolved)
        .await
        .map_err(AppError::network)
}

#[tauri::command]
pub async fn webdav_sync_fetch_remote_info(
    store: State<'_, Arc<SkillStore>>,
) -> Result<RemoteSnapshotInfo, AppError> {
    let store = store.inner().clone();
    let settings = tokio::task::spawn_blocking(move || {
        webdav_sync::get_settings(&store)
            .map_err(AppError::db)?
            .ok_or_else(|| AppError::invalid_input("WebDAV sync is not configured"))
    })
    .await??;
    let settings = require_enabled(settings)?;

    webdav_sync::fetch_remote_info(&settings)
        .await
        .map_err(AppError::network)
}

#[tauri::command]
pub async fn webdav_sync_upload(
    store: State<'_, Arc<SkillStore>>,
) -> Result<WebDavSyncResult, AppError> {
    let store = store.inner().clone();
    let mut settings = {
        let store = store.clone();
        tokio::task::spawn_blocking(move || {
            webdav_sync::get_settings(&store)
                .map_err(AppError::db)?
                .ok_or_else(|| AppError::invalid_input("WebDAV sync is not configured"))
        })
        .await??
    };
    require_enabled(settings.clone())?;

    webdav_sync::run_with_sync_lock(webdav_sync::upload_snapshot(&store, &mut settings))
        .await
        .map_err(classify_sync_error)
}

#[tauri::command]
pub async fn webdav_sync_download(
    store: State<'_, Arc<SkillStore>>,
) -> Result<WebDavSyncResult, AppError> {
    let store = store.inner().clone();
    let mut settings = {
        let store = store.clone();
        tokio::task::spawn_blocking(move || {
            webdav_sync::get_settings(&store)
                .map_err(AppError::db)?
                .ok_or_else(|| AppError::invalid_input("WebDAV sync is not configured"))
        })
        .await??
    };
    require_enabled(settings.clone())?;

    webdav_sync::run_with_sync_lock(webdav_sync::download_snapshot(&store, &mut settings))
        .await
        .map_err(classify_sync_error)
}

#[cfg(test)]
mod tests {
    use crate::core::error::ErrorKind;
    use crate::core::webdav_sync::WebDavSyncSettings;

    use super::*;

    #[test]
    fn disabled_settings_are_rejected() {
        let err = require_enabled(WebDavSyncSettings::default()).unwrap_err();

        assert_eq!(err.message, "WebDAV sync is disabled");
    }

    #[test]
    fn enabled_settings_are_accepted() {
        let settings = WebDavSyncSettings {
            enabled: true,
            ..WebDavSyncSettings::default()
        };

        assert!(require_enabled(settings).is_ok());
    }

    #[test]
    fn sync_errors_with_webdav_context_are_network_errors() {
        let err = classify_sync_error(anyhow::anyhow!(
            "WebDAV upload failed with status 503 Service Unavailable"
        ));

        assert!(matches!(err.kind, ErrorKind::Network));
    }

    #[test]
    fn sync_errors_with_sqlite_context_are_database_errors() {
        let err = classify_sync_error(anyhow::anyhow!("SQLite import failed: database is locked"));

        assert!(matches!(err.kind, ErrorKind::Database));
    }

    #[test]
    fn sync_errors_with_archive_context_are_io_errors() {
        let err = classify_sync_error(anyhow::anyhow!(
            "failed to read skills.zip from backup path"
        ));

        assert!(matches!(err.kind, ErrorKind::Io));
    }
}
