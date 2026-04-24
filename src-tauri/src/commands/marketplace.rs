use std::sync::Arc;
use tauri::State;

use crate::core::{
    error::AppError,
    git_fetcher,
    skill_metadata,
    skill_store::{
        PluginCacheRecord, PluginInstallRecord, PluginMarketRecord, SkillRecord, SkillStore,
    },
};

#[derive(Debug, serde::Serialize)]
pub struct PluginWithMarketDto {
    pub id: String,
    pub market_id: String,
    pub name: String,
    pub version: Option<String>,
    pub description: Option<String>,
    pub skill_names: Vec<String>,
    pub fetched_at: i64,
    pub market_name: String,
}

#[derive(Debug, serde::Serialize)]
pub struct BatchPluginInstallResult {
    pub installed: usize,
    pub skipped: usize,
    pub failed: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct PluginInstalledSkillDto {
    pub skill_id: String,
    pub skill_name: String,
    pub skill_description: Option<String>,
    pub market_id: String,
    pub market_name: String,
    pub plugin_name: String,
    pub installed_at: i64,
}

#[tauri::command]
pub async fn add_plugin_market(
    url: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<PluginMarketRecord, AppError> {
    let url_clone = url.clone();
    let store_clone = store.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let parsed = git_fetcher::parse_git_source(&url_clone);
        git_fetcher::validate_git_url(&parsed.clone_url)
            .map_err(|e| AppError::invalid_input(e.to_string()))?;

        if store_clone.get_plugin_market_by_url(&parsed.clone_url)?.is_some() {
            return Err(AppError::invalid_input("Market with this URL already exists"));
        }

        let proxy_url = store_clone.proxy_url();
        let temp_dir = git_fetcher::clone_repo_ref(
            &parsed.clone_url,
            parsed.branch.as_deref(),
            None,
            proxy_url.as_deref(),
        )
        .map_err(|e| AppError::network(e.to_string()))?;

        let plugins = discover_plugins(&temp_dir);

        let repo_name = parsed
            .clone_url
            .trim_end_matches(".git")
            .split('/')
            .next_back()
            .unwrap_or("unknown")
            .to_string();

        let now = chrono::Utc::now().timestamp_millis();
        let market_id = uuid::Uuid::new_v4().to_string();

        let market = PluginMarketRecord {
            id: market_id.clone(),
            name: repo_name,
            url: parsed.clone_url,
            description: None,
            plugin_count: plugins.len() as i32,
            last_fetched_at: Some(now),
            last_error: None,
            created_at: now,
            updated_at: now,
        };

        store_clone.insert_plugin_market(&market)?;

        for plugin in &plugins {
            let cache_id = uuid::Uuid::new_v4().to_string();
            let rec = PluginCacheRecord {
                id: cache_id,
                market_id: market_id.clone(),
                name: plugin.name.clone(),
                version: plugin.version.clone(),
                description: plugin.description.clone(),
                skill_names: serde_json::to_string(&plugin.skill_names)
                    .unwrap_or_else(|_| "[]".to_string()),
                fetched_at: now,
            };
            store_clone.insert_plugin_cache(&rec)?;
        }

        git_fetcher::cleanup_temp(&temp_dir);

        Ok::<PluginMarketRecord, AppError>(market)
    })
    .await??;

    Ok(result)
}

#[tauri::command]
pub async fn list_plugin_markets(
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<PluginMarketRecord>, AppError> {
    let store_clone = store.inner().clone();
    let markets = tauri::async_runtime::spawn_blocking(move || {
        store_clone.get_all_plugin_markets().map_err(AppError::db)
    })
    .await??;
    Ok(markets)
}

#[tauri::command]
pub async fn remove_plugin_market(
    id: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), AppError> {
    let store_clone = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store_clone
            .delete_plugin_market(&id)
            .map_err(AppError::db)
    })
    .await??;
    Ok(())
}

#[tauri::command]
pub async fn refresh_plugin_market(
    id: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<PluginCacheRecord>, AppError> {
    let store_clone = store.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let market = store_clone
            .get_plugin_market_by_id(&id)?
            .ok_or_else(|| AppError::not_found("Market not found"))?;

        let proxy_url = store_clone.proxy_url();
        let temp_dir = git_fetcher::clone_repo_ref(&market.url, None, None, proxy_url.as_deref())
            .map_err(|e| AppError::network(e.to_string()))?;

        let plugins = discover_plugins(&temp_dir);
        let now = chrono::Utc::now().timestamp_millis();

        store_clone.clear_plugin_cache_for_market(&id)?;
        store_clone.update_plugin_market_fetch(&id, plugins.len() as i32, None)?;

        for plugin in &plugins {
            let cache_id = uuid::Uuid::new_v4().to_string();
            let rec = PluginCacheRecord {
                id: cache_id,
                market_id: id.clone(),
                name: plugin.name.clone(),
                version: plugin.version.clone(),
                description: plugin.description.clone(),
                skill_names: serde_json::to_string(&plugin.skill_names)
                    .unwrap_or_else(|_| "[]".to_string()),
                fetched_at: now,
            };
            store_clone.insert_plugin_cache(&rec)?;
        }

        git_fetcher::cleanup_temp(&temp_dir);

        store_clone
            .get_plugins_for_market(&id)
            .map_err(AppError::db)
    })
    .await??;

    Ok(result)
}

#[tauri::command]
pub async fn list_all_plugins(
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<PluginWithMarketDto>, AppError> {
    let store_clone = store.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let markets = store_clone.get_all_plugin_markets().map_err(AppError::db)?;
        let market_map: std::collections::HashMap<String, String> = markets
            .iter()
            .map(|m| (m.id.clone(), m.name.clone()))
            .collect();

        let all_cache = store_clone.get_all_plugins().map_err(AppError::db)?;

        let dtos: Vec<PluginWithMarketDto> = all_cache
            .into_iter()
            .map(|c| {
                let skill_names: Vec<String> =
                    serde_json::from_str(&c.skill_names).unwrap_or_default();
                PluginWithMarketDto {
                    id: c.id,
                    market_id: c.market_id.clone(),
                    name: c.name,
                    version: c.version,
                    description: c.description,
                    skill_names,
                    fetched_at: c.fetched_at,
                    market_name: market_map.get(&c.market_id).cloned().unwrap_or_default(),
                }
            })
            .collect();

        Ok::<Vec<PluginWithMarketDto>, AppError>(dtos)
    })
    .await??;

    Ok(result)
}

#[tauri::command]
pub async fn search_plugins(
    query: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<PluginWithMarketDto>, AppError> {
    let store_clone = store.inner().clone();
    let query_lower = query.to_lowercase();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let markets = store_clone.get_all_plugin_markets().map_err(AppError::db)?;
        let market_map: std::collections::HashMap<String, String> = markets
            .iter()
            .map(|m| (m.id.clone(), m.name.clone()))
            .collect();

        let all_cache = store_clone.get_all_plugins().map_err(AppError::db)?;

        let dtos: Vec<PluginWithMarketDto> = all_cache
            .into_iter()
            .filter(|c| {
                c.name.to_lowercase().contains(&query_lower)
                    || c.description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&query_lower)
            })
            .map(|c| {
                let skill_names: Vec<String> =
                    serde_json::from_str(&c.skill_names).unwrap_or_default();
                PluginWithMarketDto {
                    id: c.id,
                    market_id: c.market_id.clone(),
                    name: c.name,
                    version: c.version,
                    description: c.description,
                    skill_names,
                    fetched_at: c.fetched_at,
                    market_name: market_map.get(&c.market_id).cloned().unwrap_or_default(),
                }
            })
            .collect();

        Ok::<Vec<PluginWithMarketDto>, AppError>(dtos)
    })
    .await??;

    Ok(result)
}

#[tauri::command]
pub async fn install_plugin_skills(
    market_id: String,
    plugin_name: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<BatchPluginInstallResult, AppError> {
    let store_clone = store.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let market = store_clone
            .get_plugin_market_by_id(&market_id)?
            .ok_or_else(|| AppError::not_found("Market not found"))?;

        let plugins = store_clone
            .get_plugins_for_market(&market_id)
            .map_err(AppError::db)?;

        let plugin = plugins
            .iter()
            .find(|p| p.name == plugin_name)
            .ok_or_else(|| {
                AppError::not_found(format!("Plugin {} not found in market", plugin_name))
            })?;

        let skill_names: Vec<String> =
            serde_json::from_str(&plugin.skill_names).unwrap_or_default();

        let existing_skills = store_clone.get_all_skills().map_err(AppError::db)?;
        let existing_names: std::collections::HashSet<String> =
            existing_skills.iter().map(|s| s.name.clone()).collect();

        let proxy_url = store_clone.proxy_url();
        let temp_dir = git_fetcher::clone_repo_ref(&market.url, None, None, proxy_url.as_deref())
            .map_err(|e| AppError::network(e.to_string()))?;

        let central_repo = store_clone
            .get_setting("central_repo_path_override")
            .ok()
            .flatten()
            .filter(|s| !s.is_empty())
            .map(|s| std::path::PathBuf::from(s))
            .unwrap_or_else(|| {
                dirs::data_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join("skills-manager")
                    .join("central-repo")
            });

        let mut installed = 0usize;
        let mut skipped = 0usize;
        let mut failed: Vec<String> = Vec::new();
        let now = chrono::Utc::now().timestamp_millis();

        for skill_name in &skill_names {
            if existing_names.contains(skill_name) {
                skipped += 1;
                continue;
            }

            let source_dir = find_skill_dir_in_repo(&temp_dir, skill_name);
            if source_dir.is_none() {
                failed.push(skill_name.clone());
                continue;
            }
            let source_dir = source_dir.unwrap();

            if !skill_metadata::is_valid_skill_dir(&source_dir) {
                failed.push(skill_name.clone());
                continue;
            }

            let dest_dir = central_repo.join(skill_name);
            if dest_dir.exists() {
                skipped += 1;
                continue;
            }

            if let Err(e) = copy_dir_recursive(&source_dir, &dest_dir) {
                failed.push(format!("{}: {}", skill_name, e));
                continue;
            }

            let skill_id = uuid::Uuid::new_v4().to_string();
            let meta = skill_metadata::parse_skill_md(&dest_dir);
            let name = meta.name.clone().unwrap_or_else(|| skill_name.clone());
            let description = meta.description.clone();

            let skill_record = SkillRecord {
                id: skill_id.clone(),
                name,
                description,
                source_type: "plugin".to_string(),
                source_ref: Some(format!("{}/{}", market.name, plugin_name)),
                source_ref_resolved: Some(market.url.clone()),
                source_subpath: None,
                source_branch: None,
                source_revision: None,
                remote_revision: None,
                central_path: dest_dir.to_string_lossy().to_string(),
                content_hash: None,
                enabled: true,
                created_at: now,
                updated_at: now,
                status: "ok".to_string(),
                update_status: "unknown".to_string(),
                last_checked_at: None,
                last_check_error: None,
            };

            if let Err(e) = store_clone.insert_skill(&skill_record) {
                let _ = std::fs::remove_dir_all(&dest_dir);
                failed.push(format!("{}: {}", skill_name, e));
                continue;
            }

            let install_rec = PluginInstallRecord {
                id: uuid::Uuid::new_v4().to_string(),
                market_id: market_id.clone(),
                plugin_name: plugin_name.clone(),
                skill_id,
                installed_at: now,
            };
            let _ = store_clone.insert_plugin_install(&install_rec);

            installed += 1;
        }

        git_fetcher::cleanup_temp(&temp_dir);

        Ok::<BatchPluginInstallResult, AppError>(BatchPluginInstallResult {
            installed,
            skipped,
            failed,
        })
    })
    .await??;

    Ok(result)
}

#[tauri::command]
pub async fn list_plugin_installed_skills(
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<PluginInstalledSkillDto>, AppError> {
    let store_clone = store.inner().clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let installs = store_clone.get_plugin_installs().map_err(AppError::db)?;
        let markets = store_clone.get_all_plugin_markets().map_err(AppError::db)?;
        let market_map: std::collections::HashMap<String, String> = markets
            .iter()
            .map(|m| (m.id.clone(), m.name.clone()))
            .collect();

        let skill_ids: Vec<String> = installs.iter().map(|i| i.skill_id.clone()).collect();
        let skills = store_clone.get_skills_by_ids(&skill_ids).unwrap_or_default();
        let skill_map: std::collections::HashMap<String, SkillRecord> = skills
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();

        let dtos: Vec<PluginInstalledSkillDto> = installs
            .into_iter()
            .filter_map(|inst| {
                let skill = skill_map.get(&inst.skill_id)?;
                Some(PluginInstalledSkillDto {
                    skill_id: inst.skill_id,
                    skill_name: skill.name.clone(),
                    skill_description: skill.description.clone(),
                    market_id: inst.market_id.clone(),
                    market_name: market_map.get(&inst.market_id).cloned().unwrap_or_default(),
                    plugin_name: inst.plugin_name,
                    installed_at: inst.installed_at,
                })
            })
            .collect();

        Ok::<Vec<PluginInstalledSkillDto>, AppError>(dtos)
    })
    .await??;

    Ok(result)
}

struct DiscoveredPlugin {
    name: String,
    version: Option<String>,
    description: Option<String>,
    skill_names: Vec<String>,
}

fn discover_plugins(repo_dir: &std::path::Path) -> Vec<DiscoveredPlugin> {
    let mut plugins: Vec<DiscoveredPlugin> = Vec::new();

    if try_discover_from_marketplace_json(repo_dir, &mut plugins) {
        return plugins;
    }

    if try_discover_from_plugin_dirs(repo_dir, &mut plugins) {
        return plugins;
    }

    discover_skills_as_plugins(repo_dir, &mut plugins);

    plugins
}

fn try_discover_from_marketplace_json(
    repo_dir: &std::path::Path,
    plugins: &mut Vec<DiscoveredPlugin>,
) -> bool {
    let marketplace_paths = [
        repo_dir.join(".claude-plugin").join("marketplace.json"),
        repo_dir.join("marketplace.json"),
    ];

    let marketplace_json = match marketplace_paths.iter().find(|p| p.exists()) {
        Some(p) => p,
        None => return false,
    };

    let content = match std::fs::read_to_string(marketplace_json) {
        Ok(c) => c,
        Err(_) => return false,
    };

    let index: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return false,
    };

    let plugin_list = index
        .get("plugins")
        .and_then(|v| v.as_array())
        .or_else(|| index.as_array());

    let Some(arr) = plugin_list else {
        return false;
    };

    let mut found_any = false;

    for entry in arr {
        let source = entry
            .get("source")
            .and_then(|v| v.as_str())
            .or_else(|| entry.get("name").and_then(|v| v.as_str()))
            .unwrap_or("");
        if source.is_empty() {
            continue;
        }

        let plugin_dir = repo_dir.join(source);
        if !plugin_dir.is_dir() {
            continue;
        }

        let name = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(source)
            .to_string();

        let (version, description) = read_plugin_json_metadata(&plugin_dir)
            .unwrap_or_else(|| {
                let ver = entry.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
                let desc = entry.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                (ver, desc)
            });

        let skill_names = discover_skill_names_in_plugin_dir(&plugin_dir);

        if !skill_names.is_empty() {
            plugins.push(DiscoveredPlugin {
                name,
                version,
                description,
                skill_names,
            });
            found_any = true;
        }
    }

    found_any
}

fn try_discover_from_plugin_dirs(
    repo_dir: &std::path::Path,
    plugins: &mut Vec<DiscoveredPlugin>,
) -> bool {
    let mut found_any = false;

    let Ok(entries) = std::fs::read_dir(repo_dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if dir_name.starts_with('.') || dir_name == "node_modules" {
            continue;
        }

        let has_plugin_json = path.join(".claude-plugin").join("plugin.json").exists();
        let has_skills_dir = path.join("skills").is_dir();

        if !has_plugin_json && !has_skills_dir {
            continue;
        }

        let (version, description) = read_plugin_json_metadata(&path).unwrap_or((None, None));

        let skill_names = discover_skill_names_in_plugin_dir(&path);

        if !skill_names.is_empty() {
            plugins.push(DiscoveredPlugin {
                name: dir_name,
                version,
                description,
                skill_names,
            });
            found_any = true;
        }
    }

    found_any
}

fn discover_skills_as_plugins(
    repo_dir: &std::path::Path,
    plugins: &mut Vec<DiscoveredPlugin>,
) {
    let mut skills: Vec<(std::path::PathBuf, String)> = Vec::new();
    scan_skills_recursive(repo_dir, repo_dir, &mut skills);

    for (dir, name) in skills {
        let meta = skill_metadata::parse_skill_md(&dir);
        if !plugins.iter().any(|p| p.name == name) {
            plugins.push(DiscoveredPlugin {
                name: name.clone(),
                version: None,
                description: meta.description,
                skill_names: vec![name],
            });
        }
    }
}

fn scan_skills_recursive(
    current_dir: &std::path::Path,
    base_dir: &std::path::Path,
    skills: &mut Vec<(std::path::PathBuf, String)>,
) {
    if skill_metadata::is_valid_skill_dir(current_dir) && current_dir != base_dir {
        let name = current_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !name.is_empty() && !name.starts_with('.') {
            skills.push((current_dir.to_path_buf(), name));
            return;
        }
    }

    let Ok(entries) = std::fs::read_dir(current_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if dir_name.starts_with('.') || dir_name == "node_modules" {
            continue;
        }

        scan_skills_recursive(&path, base_dir, skills);
    }
}

fn read_plugin_json_metadata(
    plugin_dir: &std::path::Path,
) -> Option<(Option<String>, Option<String>)> {
    let plugin_json = plugin_dir.join(".claude-plugin").join("plugin.json");
    if !plugin_json.exists() {
        return None;
    }

    let content = std::fs::read_to_string(&plugin_json).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    let version = json.get("version").and_then(|v| v.as_str()).map(|s| s.to_string());
    let description = json
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some((version, description))
}

fn discover_skill_names_in_plugin_dir(plugin_dir: &std::path::Path) -> Vec<String> {
    let mut names = Vec::new();

    let skills_dir = plugin_dir.join("skills");
    if skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if skill_metadata::is_valid_skill_dir(&path) {
                    if let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) {
                        names.push(name);
                    }
                }
            }
        }
    }

    if names.is_empty() && skill_metadata::is_valid_skill_dir(plugin_dir) {
        if let Some(name) = plugin_dir.file_name().map(|n| n.to_string_lossy().to_string()) {
            names.push(name);
        }
    }

    names
}

fn find_skill_dir_in_repo(
    repo_dir: &std::path::Path,
    skill_name: &str,
) -> Option<std::path::PathBuf> {
    let direct = repo_dir.join(skill_name);
    if direct.is_dir() && skill_metadata::is_valid_skill_dir(&direct) {
        return Some(direct);
    }

    let in_skills = repo_dir.join("skills").join(skill_name);
    if in_skills.is_dir() && skill_metadata::is_valid_skill_dir(&in_skills) {
        return Some(in_skills);
    }

    find_skill_dir_recursive(repo_dir, skill_name)
}

fn find_skill_dir_recursive(
    current_dir: &std::path::Path,
    skill_name: &str,
) -> Option<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(current_dir) else {
        return None;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let dir_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if dir_name.starts_with('.') || dir_name == "node_modules" {
            continue;
        }

        if dir_name == skill_name && skill_metadata::is_valid_skill_dir(&path) {
            return Some(path);
        }

        if let Some(found) = find_skill_dir_recursive(&path, skill_name) {
            return Some(found);
        }
    }

    None
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
