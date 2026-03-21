use std::path::PathBuf;
use std::sync::Arc;
use serde::Serialize;
use tauri::State;

use crate::core::error::AppError;
use crate::core::skill_store::{SkillStore, SkillTargetRecord};
use crate::core::sync_engine;
use crate::core::tool_adapters;

use super::scenarios::sync_scenario_skills;

#[derive(Debug, Serialize)]
pub struct ToolInfoDto {
    pub key: String,
    pub display_name: String,
    pub installed: bool,
    pub skills_dir: String,
    pub enabled: bool,
}

fn get_disabled_tools(store: &SkillStore) -> Vec<String> {
    store
        .get_setting("disabled_tools")
        .ok()
        .flatten()
        .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
        .unwrap_or_default()
}

fn set_disabled_tools(store: &SkillStore, disabled: &[String]) -> Result<(), AppError> {
    let json = serde_json::to_string(disabled)
        .map_err(|e| AppError::internal(format!("Failed to serialize: {e}")))?;
    store.set_setting("disabled_tools", &json).map_err(AppError::db)
}

/// Sync active scenario skills to a single tool.
fn sync_active_scenario_to_tool(store: &SkillStore, tool_key: &str) {
    let active_id = match store.get_active_scenario_id() {
        Ok(Some(id)) => id,
        _ => return,
    };
    let skills = match store.get_skills_for_scenario(&active_id) {
        Ok(s) => s,
        _ => return,
    };
    let adapter = match tool_adapters::find_adapter(tool_key) {
        Some(a) if a.is_installed() => a,
        _ => return,
    };
    let configured_mode = store.get_setting("sync_mode").ok().flatten();
    for skill in &skills {
        let source = PathBuf::from(&skill.central_path);
        let target = adapter.skills_dir().join(&skill.name);
        let mode = sync_engine::sync_mode_for_tool(&adapter.key, configured_mode.as_deref());
        if let Ok(actual_mode) = sync_engine::sync_skill(&source, &target, mode) {
            let now = chrono::Utc::now().timestamp_millis();
            let target_record = SkillTargetRecord {
                id: uuid::Uuid::new_v4().to_string(),
                skill_id: skill.id.clone(),
                tool: adapter.key.clone(),
                target_path: target.to_string_lossy().to_string(),
                mode: actual_mode.as_str().to_string(),
                status: "ok".to_string(),
                synced_at: Some(now),
                last_error: None,
            };
            store.insert_target(&target_record).ok();
        }
    }
}

/// Remove all synced skill files and target records for a given tool.
fn unsync_all_for_tool(store: &SkillStore, tool_key: &str) {
    let targets = store.get_all_targets().unwrap_or_default();
    for target in targets.iter().filter(|t| t.tool == tool_key) {
        sync_engine::remove_target(&PathBuf::from(&target.target_path)).ok();
        store.delete_target(&target.skill_id, tool_key).ok();
    }
}

#[tauri::command]
pub async fn get_tool_status(store: State<'_, Arc<SkillStore>>) -> Result<Vec<ToolInfoDto>, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let adapters = tool_adapters::default_tool_adapters();
        let disabled = get_disabled_tools(&store);
        let result: Vec<ToolInfoDto> = adapters
            .into_iter()
            .map(|a| ToolInfoDto {
                key: a.key.clone(),
                display_name: a.display_name.clone(),
                installed: a.is_installed(),
                skills_dir: a.skills_dir().to_string_lossy().to_string(),
                enabled: !disabled.contains(&a.key),
            })
            .collect();
        Ok(result)
    })
    .await?
}

#[tauri::command]
pub async fn set_tool_enabled(
    key: String,
    enabled: bool,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut disabled = get_disabled_tools(&store);
        if enabled {
            disabled.retain(|k| k != &key);
            set_disabled_tools(&store, &disabled)?;
            sync_active_scenario_to_tool(&store, &key);
            Ok(())
        } else {
            if !disabled.contains(&key) {
                disabled.push(key.clone());
            }
            unsync_all_for_tool(&store, &key);
            set_disabled_tools(&store, &disabled)
        }
    })
    .await?
}

#[tauri::command]
pub async fn set_all_tools_enabled(
    enabled: bool,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if enabled {
            set_disabled_tools(&store, &[])?;
            // Re-sync active scenario skills to all (now-enabled) installed tools
            if let Ok(Some(active_id)) = store.get_active_scenario_id() {
                sync_scenario_skills(&store, &active_id).ok();
            }
            Ok(())
        } else {
            let adapters = tool_adapters::default_tool_adapters();
            let all_keys: Vec<String> = adapters.iter().map(|a| a.key.clone()).collect();
            for adapter in &adapters {
                unsync_all_for_tool(&store, &adapter.key);
            }
            set_disabled_tools(&store, &all_keys)
        }
    })
    .await?
}
