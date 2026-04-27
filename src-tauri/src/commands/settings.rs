use semver::Version;
use std::process::Command;
use std::sync::Arc;
use tauri::State;

use crate::core::{central_repo, error::AppError, skill_store::SkillStore, skillssh_api};

#[derive(serde::Serialize)]
pub struct AppUpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
}

#[tauri::command]
pub async fn get_settings(
    key: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<Option<String>, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.get_setting(&key).map_err(AppError::db))
        .await?
}

#[tauri::command]
pub async fn set_settings(
    app: tauri::AppHandle,
    key: String,
    value: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    let key_for_store = key.clone();
    let value_for_store = value.clone();
    tauri::async_runtime::spawn_blocking(move || {
        store
            .set_setting(&key_for_store, &value_for_store)
            .map_err(AppError::db)?;
        if key_for_store == "show_tray_icon" {
            let tray_enabled = matches!(
                value_for_store.trim().to_ascii_lowercase().as_str(),
                "true" | "1" | "yes" | "on"
            );
            if !tray_enabled {
                store
                    .set_setting("close_action", "close")
                    .map_err(AppError::db)?;
            }
        }
        Ok::<(), AppError>(())
    })
    .await??;

    if key == "show_tray_icon" {
        let enabled = matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "true" | "1" | "yes" | "on"
        );
        crate::set_tray_icon_enabled(&app, enabled).map_err(AppError::io)?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_central_repo_path() -> String {
    central_repo::base_dir().to_string_lossy().to_string()
}

#[tauri::command]
pub fn get_central_repo_path_override() -> Option<String> {
    central_repo::configured_base_dir().map(|path| path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn set_central_repo_path(path: Option<String>) -> Result<String, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        central_repo::set_base_dir_override(path)
            .map(|resolved| resolved.to_string_lossy().to_string())
            .map_err(AppError::io)
    })
    .await?
}

#[tauri::command]
pub async fn open_central_repo_folder() -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(|| {
        let repo_path = central_repo::base_dir();

        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("open");
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = Command::new("explorer");
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000); // CREATE_NO_WINDOW
            c
        };
        #[cfg(target_os = "linux")]
        let mut cmd = Command::new("xdg-open");

        let status = cmd
            .arg(&repo_path)
            .status()
            .map_err(|e| AppError::io(format!("Failed to open folder: {e}")))?;

        // Windows explorer.exe returns exit code 1 even on success
        #[cfg(not(target_os = "windows"))]
        if !status.success() {
            return Err(AppError::io(format!(
                "File manager exited with status: {status}"
            )));
        }

        let _ = status;
        Ok(())
    })
    .await?
}

#[tauri::command]
pub async fn open_folder(path: String) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        let mut cmd = Command::new("open");
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = Command::new("explorer");
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x08000000);
            c
        };
        #[cfg(target_os = "linux")]
        let mut cmd = Command::new("xdg-open");

        let status = cmd
            .arg(&path)
            .status()
            .map_err(|e| AppError::io(format!("Failed to open folder: {e}")))?;

        #[cfg(not(target_os = "windows"))]
        if !status.success() {
            return Err(AppError::io(format!(
                "File manager exited with status: {status}"
            )));
        }

        let _ = status;
        Ok(())
    })
    .await?
}

#[tauri::command]
pub async fn check_app_update(
    app: tauri::AppHandle,
    store: State<'_, Arc<SkillStore>>,
) -> Result<AppUpdateInfo, AppError> {
    let current_version = app.config().version.clone().unwrap_or_default();
    let proxy_url = store.proxy_url();
    tauri::async_runtime::spawn_blocking(move || {
        let client = skillssh_api::build_http_client(proxy_url.as_deref(), 15);

        let resp: serde_json::Value = client
            .get("https://api.github.com/repos/arch3rPro/Skills-Manager-Plus/releases/latest")
            .send()
            .map_err(|e| AppError::network(format!("Network error: {e}")))?
            .json()
            .map_err(|e| AppError::network(format!("Failed to parse response: {e}")))?;

        let tag = resp["tag_name"]
            .as_str()
            .ok_or_else(|| AppError::network("No tag_name in response"))?;
        let latest_version = tag.strip_prefix('v').unwrap_or(tag).to_string();
        let release_url = resp["html_url"]
            .as_str()
            .unwrap_or("https://github.com/arch3rPro/Skills-Manager-Plus/releases")
            .to_string();

        let has_update = version_gt(&latest_version, &current_version);

        Ok(AppUpdateInfo {
            has_update,
            current_version,
            latest_version,
            release_url,
        })
    })
    .await?
}

#[tauri::command]
pub async fn app_exit(app: tauri::AppHandle) {
    let app_for_main = app.clone();
    if let Err(err) = app.run_on_main_thread(move || crate::quit_app(&app_for_main)) {
        log::error!("Failed to schedule app_exit on main thread: {err}");
        crate::quit_app(&app);
    }
}

#[tauri::command]
pub async fn hide_to_tray(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let show_tray_icon = {
        let store = store.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            let value = store.get_setting("show_tray_icon").map_err(AppError::db)?;
            Ok::<bool, AppError>(!matches!(
                value.as_deref().map(str::trim).map(str::to_ascii_lowercase),
                Some(v) if matches!(v.as_str(), "false" | "0" | "no" | "off")
            ))
        })
        .await??
    };

    if !show_tray_icon {
        crate::quit_app(&app);
        return Ok(());
    }

    window.hide().map_err(|e| AppError::io(e.to_string()))?;
    // On macOS, avoid app.hide() (app-level hidden state can block restore in tray flow).
    // Keep app running and hide only the window + Dock icon.
    #[cfg(target_os = "macos")]
    {
        app.set_dock_visibility(false)
            .map_err(|e| AppError::io(format!("Failed to hide Dock icon on macOS: {e}")))?;
        app.set_activation_policy(tauri::ActivationPolicy::Accessory)
            .map_err(|e| {
                AppError::io(format!("Failed to set activation policy to Accessory: {e}"))
            })?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
    Ok(())
}

fn version_gt(a: &str, b: &str) -> bool {
    // Prefer strict SemVer comparison (supports pre-release/build metadata).
    if let (Ok(a_ver), Ok(b_ver)) = (Version::parse(a), Version::parse(b)) {
        return a_ver > b_ver;
    }

    // Fallback for non-SemVer tags.
    let parse = |s: &str| -> Vec<u64> { s.split('.').filter_map(|p| p.parse().ok()).collect() };
    parse(a) > parse(b)
}
