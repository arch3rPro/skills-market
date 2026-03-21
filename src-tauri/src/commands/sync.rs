use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

use crate::core::{
    error::AppError,
    skill_store::{SkillStore, SkillTargetRecord},
    sync_engine,
    tool_adapters,
};

#[tauri::command]
pub async fn sync_skill_to_tool(
    skill_id: String,
    tool: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let adapter = tool_adapters::find_adapter(&tool)
            .ok_or_else(|| AppError::not_found(format!("Unknown tool: {}", tool)))?;

        if !adapter.is_installed() {
            return Err(AppError::not_found(format!("{} is not installed", adapter.display_name)));
        }

        // Reject sync to disabled tools
        let disabled: Vec<String> = store
            .get_setting("disabled_tools")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str(&v).ok())
            .unwrap_or_default();
        if disabled.contains(&tool) {
            return Err(AppError::invalid_input(format!("{} is disabled", adapter.display_name)));
        }

        let skill = store
            .get_skill_by_id(&skill_id)
            .map_err(AppError::db)?
            .ok_or_else(|| AppError::not_found("Skill not found"))?;

        let source = PathBuf::from(&skill.central_path);
        let target = adapter.skills_dir().join(&skill.name);
        let configured_mode = store
            .get_setting("sync_mode")
            .map_err(AppError::db)?;
        let mode = sync_engine::sync_mode_for_tool(&tool, configured_mode.as_deref());

        let actual_mode =
            sync_engine::sync_skill(&source, &target, mode).map_err(AppError::io)?;

        let now = chrono::Utc::now().timestamp_millis();
        let target_record = SkillTargetRecord {
            id: uuid::Uuid::new_v4().to_string(),
            skill_id: skill_id.clone(),
            tool: tool.clone(),
            target_path: target.to_string_lossy().to_string(),
            mode: actual_mode.as_str().to_string(),
            status: "ok".to_string(),
            synced_at: Some(now),
            last_error: None,
        };

        store
            .insert_target(&target_record)
            .map_err(AppError::db)?;

        Ok(())
    })
    .await?
}

#[tauri::command]
pub async fn unsync_skill_from_tool(
    skill_id: String,
    tool: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let targets = store
            .get_targets_for_skill(&skill_id)
            .map_err(AppError::db)?;

        if let Some(target) = targets.iter().find(|t| t.tool == tool) {
            let target_path = PathBuf::from(&target.target_path);
            sync_engine::remove_target(&target_path).ok();
        }

        store
            .delete_target(&skill_id, &tool)
            .map_err(AppError::db)?;

        Ok(())
    })
    .await?
}
