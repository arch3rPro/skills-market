# Plugin Marketplace — Brainstorm Report

> **Date**: 2026-04-23
> **Product**: Skills Manager (Tauri + React + TypeScript)
> **Objective**: Add Claude Code plugin repository-based skill installation management
> **Method**: Multi-perspective ideation (PM / Designer / Engineer)

---

## 1. Opportunity Understanding

### Product Context
- **Skills Manager** is a Tauri desktop app for managing AI coding assistant skills
- **Current install sources**: skills.sh market, ClawHub, local folder, Git repo
- **Tech stack**: Rust backend (SQLite via `SkillStore`), React + TypeScript + Tailwind CSS frontend
- **i18n**: react-i18next with zh / zh-TW / en locales
- **UI patterns**: Tab-based navigation, `app-panel`/`app-input`/`app-button-*` component system, dark theme

### User Need
Add a **Plugin Marketplace** that:
1. Manages plugin market sources (GitHub repos containing Claude Code plugins)
2. Discovers available plugins across all added markets
3. Batch-installs all skills from a selected plugin
4. Tracks which skills were installed from which plugin
5. **Key constraint**: This is a management UI only — actual plugin/skill installation is handled by Claude Code or other tools

### Target Segment
- Developers using Claude Code who want to manage plugin-based skills centrally
- Teams wanting to standardize plugin sources across members

---

## 2. Ideation — PM Perspective (Business Value & Customer Impact)

### PM-1: Three-Tab Plugin Marketplace (Core UX Framework)
**Description**: A complete marketplace with three sub-tabs: Discover (browse/search all plugins), Installed (skills installed via plugins), Markets (manage market sources).

**Why it matters**: Directly maps to the user's mental model of "market → plugin → skills". Each tab has a clear purpose:
- **Discover**: Aggregated plugin list from all markets, search/filter, one-click batch install
- **Installed**: All plugin-sourced skills with origin tracing (which plugin → which market)
- **Markets**: CRUD for market sources with metadata overview

**Assumptions to validate**:
- Users understand the three-layer hierarchy (market/plugin/skill)
- A single plugin can contain multiple skills worth batch-installing

### PM-2: Plugin Detail Modal + Skill Preview
**Description**: Clicking a plugin opens a modal showing name, version, description, source market, and full list of contained skills with names and descriptions.

**Why it matters**: Users need to know what they're installing before committing. The modal serves as the "decision point" between discovery and action.

**Assumptions to validate**:
- Plugin metadata (name, version, description, skill list) can be reliably parsed from repos

### PM-3: Market Health Indicators & Smart Recommendations
**Description**: Each market shows health metrics (plugin count, last update time, response latency). System recommends related plugins based on installed skills ("Users who installed vercel also installed context7").

**Why it matters**: Builds trust in market sources and increases discovery serendipity.

**Assumptions to validate**:
- Market metadata is available or can be inferred
- Recommendation algorithm adds value without being annoying

### PM-4: Install History & Batch Uninstall
**Description**: Track every plugin installation as a snapshot. Support one-click uninstall of all skills from a specific plugin (batch cleanup). Optional rollback support.

**Why it matters**: Gives users confidence to experiment with new plugins knowing they can easily clean up.

**Assumptions to validate**:
- Users want to manage skills at the plugin level, not just individual skill level

### PM-5: Market Configuration Sharing
**Description**: Export/import market lists as JSON config files. Share within teams for standardized plugin sourcing.

**Why it matters**: Team collaboration use case — onboard new team members instantly.

**Assumptions to validate**:
- Teams have a need for shared/standardized plugin configurations

---

## 3. Ideation — Designer Perspective (UX & Usability)

### D-1: Consistent Tab Design (Seamless Integration)
**Description**: Add "Plugins" as a 5th tab in the existing `InstallSkills` page tab bar. Use identical styling (`border-b-2` active state, icon + label). Inside, use `app-segmented` style for the three sub-tabs (Discover / Installed / Markets).

**Reference**: Existing tabs in [InstallSkills.tsx](src/views/InstallSkills.tsx) lines 798-822

**Design tokens to reuse**:
- Tab button: `"flex items-center gap-1.5 border-b-2 px-1 pb-1.5 text-[13px] font-medium"`
- Active: `"border-accent text-accent"`
- Inactive: `"border-transparent text-muted hover:text-tertiary"`
- Segmented control: `app-segmented` / `app-segmented-button` classes

### D-2: Plugin Card Grid (Discover View)
**Description**: Grid layout matching existing market cards (`grid grid-cols-2 lg:grid-cols-3`). Each card shows:
- Plugin name (bold, truncatable)
- Version badge (pill style, e.g., `v0.40.0`)
- Description (2-line clamp)
- Source market name (muted text, e.g., "来自 claude-plugins-official")
- Install button (right-aligned, `app-button-primary` style)

**Reference**: Existing skill cards in [InstallSkills.tsx](src/views/InstallSkills.tsx) lines 1117-1223

**Key difference from current cards**: No Agent/Hook/MCP tags (per user requirement), focus on plugin-level info.

### D-3: Market Add Dialog with Examples
**Description**: Modal dialog for adding a new market source. Input field with rich placeholder examples:
```
Examples:
  owner/repo (GitHub)
  https://github.com/owner/repo.git
  https://example.com/marketplace.json
  /path/to/marketplace
```
Cancel / Add buttons following existing dialog patterns.

**Reference**: Git URL input in [InstallSkills.tsx](src/views/InstallSkills.tsx) lines 1624-1687

### D-4: Grouped Installed Skills View
**Description**: Installed tab groups skills by their source plugin. Each group is a collapsible section with:
- Plugin name as header (with version badge)
- Count badge ("N skills")
- List of skill cards below
- Expand/collapse animation

**Pattern reference**: Scenario grouping in Sidebar, scan result groups in MySkills local tab

### D-5: Comprehensive Empty/Loading States
**Three empty states**:
1. **No markets added**: CTA to add first market (illustration + text + button)
2. **No plugins found**: Check network / market validity message
3. **All installed**: Completion state with suggestion to discover more markets

**Loading states**: Reuse `Loader2` spinner from lucide-react (consistent with existing usage)

---

## 4. Ideation — Engineer Perspective (Technical Possibilities)

### E-1: Database Schema Extension
**New tables to add to SQLite (alongside existing migrations)**:

```sql
-- Plugin market sources
CREATE TABLE plugin_markets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url TEXT NOT NULL UNIQUE,
    description TEXT,
    plugin_count INTEGER DEFAULT 0,
    last_fetched_at INTEGER,
    last_error TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Discovered plugins from markets
CREATE TABLE plugin_cache (
    id TEXT PRIMARY KEY,
    market_id TEXT NOT NULL REFERENCES plugin_markets(id),
    name TEXT NOT NULL,
    version TEXT,
    description TEXT,
    skill_names TEXT NOT NULL,  -- JSON array of skill directory names
    fetched_at INTEGER NOT NULL,
    UNIQUE(market_id, name)
);

-- Track which skills were installed from which plugin
CREATE TABLE plugin_installs (
    id TEXT PRIMARY KEY,
    plugin_cache_id TEXT REFERENCES plugin_cache(id),
    skill_id TEXT REFERENCES skills(id),
    installed_at INTEGER NOT NULL
);
```

**Integration with existing schema**:
- `SkillRecord.source_type` = `"plugin"` for plugin-installed skills
- `SkillRecord.source_ref` = `{market_name}/{plugin_name}` for traceability

### E-2: Tauri Command API Design
**New commands in Rust**:

```rust
// ── Market CRUD ──
#[tauri::command]
async fn add_plugin_market(store: State<'_, SkillStore>, url: String) -> Result<MarketRecord>

#[tauri::command]
async fn list_plugin_markets(store: State<'_, SkillStore>) -> Result<Vec<MarketRecord>>

#[tauri::command]
async fn remove_plugin_market(store: State<'_, SkillStore>, id: String) -> Result<()>

#[tauri::command]
async fn refresh_plugin_market(store: State<'_, SkillStore>, id: String) -> Result<MarketRecord>

// ── Plugin Discovery ──
#[tauri::command]
async fn fetch_plugins_from_market(store: State<'_, SkillStore>, market_id: String) -> Result<Vec<PluginInfo>>

#[tauri::command]
async fn list_all_plugins(store: State<'_, SkillStore>) -> Result<Vec<PluginWithMarket>>

#[tauri::command]
async fn search_plugins(store: State<'_, SkillStore>, query: String) -> Result<Vec<PluginWithMarket>>

// ── Plugin Installation (Batch) ──
#[tauri::command]
async fn install_plugin_skills(
    store: State<'_, SkillStore>,
    market_id: String,
    plugin_name: String,
) -> Result<BatchInstallResult>

// ── Query Installed ──
#[tauri::command]
async fn list_plugin_installed_skills(store: State<'_, SkillStore>) -> Result<Vec<SkillWithPluginSource>>
```

**Reuse existing infrastructure**:
- `git_fetcher` module for cloning/parsing repos
- `installer` module for skill registration
- `skill_metadata::parse_skill_md()` for reading SKILL.md files
- Cache table `skillssh_cache` pattern for plugin list caching

### E-3: Frontend Architecture
**New files to create**:
```
src/
  views/
    PluginMarketplace.tsx       # Main view component (3 tabs)
  components/
    PluginCard.tsx              # Plugin card for discover list
    PluginDetailModal.tsx       # Detail popup
    MarketCard.tsx              # Market item for market list
    AddMarketDialog.tsx         # Add market modal
  lib/
    plugin-api.ts               # Tauri invoke wrappers + types
```

**Integration points**:
- Add route `/plugins` or new tab in `InstallSkills.tsx`
- Extend `AppContext` with plugin state (following `managedSkills` pattern)
- Add i18n keys to all 3 locale files

### E-4: Caching Strategy
**Reuse existing cache pattern** from [`skill_store.rs`](src-tauri/src/core/skill_store.rs#L470):
- Cache key: `plugin_list:{market_id}`
- TTL: 1 hour (configurable)
- Manual force-refresh available
- Fallback to cached data on network error

### E-5: Extensible Market Format Parser
**Phase 1 (MVP)**: Support Claude Code official format
- Parse GitHub repo structure (directories containing `SKILL.md`)
- Read `marketplace.json` if present (standard index file)

**Architecture for future extension**:
```rust
trait MarketParser {
    async fn parse(&self, url: &str) -> Result<Vec<PluginInfo>>;
}

struct GithubRepoParser;      // Phase 1: Directory walking
struct MarketplaceJsonParser; // Phase 1b: JSON index
struct RegistryApiParser;     // Future: Custom registry endpoint
struct LocalPathParser;       // Future: Local filesystem
```

---

## 5. Prioritized Top 5 Ideas

| Rank | Idea | Perspective | Rationale | Key Assumption |
|------|------|-------------|-----------|----------------|
| **1** | **Three-Tab Plugin Marketplace** | PM | Complete feature framework directly addressing user requirements | Users grok market→plugin→skill hierarchy |
| **2** | **Consistent UI Integration** | Designer | Zero learning curve by reusing existing design system | Current component library covers all needs |
| **3** | **Schema + API Design** | Engineer | Foundation layer; determines maintainability and performance | SQLite extension has negligible perf impact |
| **4** | **Plugin Detail Modal** | PM+Designer | Critical decision-point UX between browse and install action | Plugin metadata can be reliably extracted |
| **5** | **Caching + Extensible Parser** | Engineer | Performance today, flexibility tomorrow | Market data volume is manageable |

---

## 6. Implementation Roadmap (Suggested Phases)

### Phase 1 — MVP (Core Loop)
- Database schema (plugin_markets, plugin_cache tables)
- Backend: add/remove/list markets, fetch plugins, batch install
- Frontend: 3-tab view (Discover/Installed/Markets), basic card layouts
- i18n: zh/en/zh-TW keys

### Phase 2 — Polish
- Plugin detail modal with skill list preview
- Search across all markets
- Caching with TTL
- Empty/loading states for all scenarios

### Phase 3 — Advanced
- Market health indicators
- Install history tracking + batch uninstall
- Market config export/import (team sharing)
- Extended market format support (JSON index, registry API)

---

## 7. Risk & Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Market repo may be large/unstructured | Slow fetch, parsing errors | Async fetching with timeout; graceful degradation |
| Plugin format varies across sources | Inconsistent metadata | Standardized parser interface; fallback heuristics |
| Skills already exist from other sources | Duplicate installs | Pre-install check against existing `source_ref` values |
| Network unavailable | Can't discover plugins | Cached data fallback; clear offline indicator |
