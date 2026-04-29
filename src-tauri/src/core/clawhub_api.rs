use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::skillssh_api::build_http_client;

const CLAWHUB_BASE_URL: &str = "https://clawhub.ai/api/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawhubSkill {
    pub slug: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub summary: String,
    pub tags: Option<serde_json::Value>,
    pub stats: Option<serde_json::Value>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<i64>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub score: f64,
    pub slug: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    pub summary: String,
    pub version: Option<String>,
    #[serde(rename = "updatedAt")]
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsListResponse {
    pub items: Vec<ClawhubSkill>,
    #[serde(rename = "nextCursor")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SortType {
    Updated,
    Downloads,
    Stars,
    Trending,
}

impl SortType {
    pub fn from_str(s: &str) -> Self {
        match s {
            "downloads" => Self::Downloads,
            "stars" | "rating" => Self::Stars,
            "trending" => Self::Trending,
            _ => Self::Updated,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Updated => "updated",
            Self::Downloads => "downloads",
            Self::Stars => "stars",
            Self::Trending => "trending",
        }
    }
}

pub fn search_skills(
    query: &str,
    limit: usize,
    api_key: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<Vec<SearchResult>> {
    let client = build_http_client(proxy_url, 15);

    let url = format!(
        "{}/search?q={}&limit={}",
        CLAWHUB_BASE_URL,
        urlencoding::encode(query),
        limit.min(200)
    );

    let mut request = client.get(&url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let resp: serde_json::Value = request
        .send()
        .context("Failed to fetch ClawHub")?
        .error_for_status()
        .context("ClawHub request failed")?
        .json()
        .context("Failed to parse ClawHub response")?;

    let results_array = resp
        .get("results")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut results = Vec::new();
    for item in &results_array {
        if let Ok(result) = serde_json::from_value::<SearchResult>(item.clone()) {
            results.push(result);
        }
    }

    Ok(results)
}

pub fn fetch_skills(
    sort: SortType,
    limit: usize,
    cursor: Option<&str>,
    api_key: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<SkillsListResponse> {
    let client = build_http_client(proxy_url, 15);

    let mut url = format!(
        "{}/skills?sort={}&limit={}",
        CLAWHUB_BASE_URL,
        sort.as_str(),
        limit.min(200)
    );

    if let Some(c) = cursor.filter(|c| !c.is_empty()) {
        url.push_str(&format!("&cursor={}", urlencoding::encode(c)));
    }

    let mut request = client.get(&url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let resp: serde_json::Value = request
        .send()
        .context("Failed to fetch ClawHub")?
        .error_for_status()
        .context("ClawHub request failed")?
        .json()
        .context("Failed to parse ClawHub response")?;

    let items: Vec<ClawhubSkill> = resp
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| serde_json::from_value::<ClawhubSkill>(item.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    let next_cursor = resp
        .get("nextCursor")
        .and_then(|v| v.as_str())
        .map(String::from);

    Ok(SkillsListResponse { items, next_cursor })
}

pub fn get_skill_detail(
    slug: &str,
    api_key: Option<&str>,
    proxy_url: Option<&str>,
) -> Result<ClawhubSkill> {
    let client = build_http_client(proxy_url, 15);

    let url = format!("{}/skills/{}", CLAWHUB_BASE_URL, slug);

    let mut request = client.get(&url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    let resp: serde_json::Value = request
        .send()
        .context("Failed to fetch ClawHub")?
        .error_for_status()
        .context("ClawHub request failed")?
        .json()
        .context("Failed to parse ClawHub response")?;

    let skill = resp
        .get("skill")
        .ok_or_else(|| anyhow::anyhow!("Skill not found in response"))?;

    serde_json::from_value::<ClawhubSkill>(skill.clone()).context("Failed to parse skill detail")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_type_from_str_handles_all_variants() {
        assert!(matches!(
            SortType::from_str("downloads"),
            SortType::Downloads
        ));
        assert!(matches!(SortType::from_str("stars"), SortType::Stars));
        assert!(matches!(SortType::from_str("rating"), SortType::Stars));
        assert!(matches!(SortType::from_str("trending"), SortType::Trending));
        assert!(matches!(SortType::from_str("updated"), SortType::Updated));
        assert!(matches!(SortType::from_str("invalid"), SortType::Updated));
    }

    #[test]
    fn sort_type_as_str_returns_correct_values() {
        assert_eq!(SortType::Updated.as_str(), "updated");
        assert_eq!(SortType::Downloads.as_str(), "downloads");
        assert_eq!(SortType::Stars.as_str(), "stars");
        assert_eq!(SortType::Trending.as_str(), "trending");
    }
}
