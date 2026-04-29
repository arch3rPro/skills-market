use crate::core::{data_backup::DataBackupEntry, error::AppError, skill_store::SkillStore};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn export_data_backup(
    store: State<'_, Arc<SkillStore>>,
    target_path: String,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || {
        store
            .export_data_backup(&PathBuf::from(target_path))
            .map_err(AppError::db)
    })
    .await?
}

#[tauri::command]
pub async fn import_data_backup(
    store: State<'_, Arc<SkillStore>>,
    source_path: String,
) -> Result<String, AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || {
        store
            .import_data_backup(&PathBuf::from(source_path))
            .map_err(AppError::db)
    })
    .await?
}

#[tauri::command]
pub async fn create_data_backup(store: State<'_, Arc<SkillStore>>) -> Result<String, AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || store.create_data_backup().map_err(AppError::db)).await?
}

#[tauri::command]
pub async fn list_data_backups(
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<DataBackupEntry>, AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || store.list_data_backups().map_err(AppError::db)).await?
}

#[tauri::command]
pub async fn restore_data_backup(
    store: State<'_, Arc<SkillStore>>,
    filename: String,
) -> Result<String, AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || store.restore_data_backup(&filename).map_err(AppError::db))
        .await?
}

#[tauri::command]
pub async fn rename_data_backup(
    store: State<'_, Arc<SkillStore>>,
    old_filename: String,
    new_name: String,
) -> Result<String, AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || {
        store
            .rename_data_backup(&old_filename, &new_name)
            .map_err(AppError::db)
    })
    .await?
}

#[tauri::command]
pub async fn delete_data_backup(
    store: State<'_, Arc<SkillStore>>,
    filename: String,
) -> Result<(), AppError> {
    let store = store.inner().clone();
    tokio::task::spawn_blocking(move || store.delete_data_backup(&filename).map_err(AppError::db))
        .await?
}
