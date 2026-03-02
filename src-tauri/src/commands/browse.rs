use std::sync::Arc;
use tauri::State;

use crate::core::{
    skill_store::SkillStore,
    skillssh_api::{self, LeaderboardType, SkillsShSkill},
};

const LEADERBOARD_CACHE_TTL: i64 = 300; // 5 minutes

#[tauri::command]
pub fn fetch_leaderboard(
    board: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<SkillsShSkill>, String> {
    let cache_key = format!("leaderboard_{}", board);

    // Check cache
    if let Ok(Some(cached)) = store.get_cache(&cache_key, LEADERBOARD_CACHE_TTL) {
        if let Ok(skills) = serde_json::from_str::<Vec<SkillsShSkill>>(&cached) {
            return Ok(skills);
        }
    }

    let board_type = LeaderboardType::from_str(&board);
    let skills = skillssh_api::fetch_leaderboard(board_type).map_err(|e| e.to_string())?;

    // Update cache
    if let Ok(json) = serde_json::to_string(&skills) {
        store.set_cache(&cache_key, &json).ok();
    }

    Ok(skills)
}

#[tauri::command]
pub fn search_skillssh(query: String, limit: Option<usize>) -> Result<Vec<SkillsShSkill>, String> {
    let requested = limit.unwrap_or(60);
    let bounded = requested.clamp(1, 300);
    skillssh_api::search_skills(&query, bounded).map_err(|e| e.to_string())
}
