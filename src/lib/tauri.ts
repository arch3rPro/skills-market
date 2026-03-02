import { invoke } from "@tauri-apps/api/core";

// ── Types ──

export interface ToolInfo {
  key: string;
  display_name: string;
  installed: boolean;
  skills_dir: string;
}

export interface ManagedSkill {
  id: string;
  name: string;
  description: string | null;
  source_type: string;
  source_ref: string | null;
  source_revision: string | null;
  remote_revision: string | null;
  update_status: string;
  last_checked_at: number | null;
  last_check_error: string | null;
  central_path: string;
  enabled: boolean;
  created_at: number;
  updated_at: number;
  status: string;
  targets: SkillTarget[];
  scenario_ids: string[];
}

export interface SkillTarget {
  id: string;
  skill_id: string;
  tool: string;
  target_path: string;
  mode: string;
  status: string;
  synced_at: number | null;
}

export interface SkillDocument {
  skill_id: string;
  filename: string;
  content: string;
  central_path: string;
}

export interface Scenario {
  id: string;
  name: string;
  description: string | null;
  icon: string | null;
  sort_order: number;
  skill_count: number;
  created_at: number;
  updated_at: number;
}

export interface DiscoveredGroup {
  name: string;
  fingerprint: string | null;
  locations: { id: string; tool: string; found_path: string }[];
  imported: boolean;
}

export interface ScanResult {
  tools_scanned: number;
  skills_found: number;
  groups: DiscoveredGroup[];
}

export interface SkillsShSkill {
  id: string;
  skill_id: string;
  name: string;
  source: string;
  installs: number;
}

// ── Tools ──

export const getToolStatus = () => invoke<ToolInfo[]>("get_tool_status");

// ── Skills ──

export const getManagedSkills = () =>
  invoke<ManagedSkill[]>("get_managed_skills");

export const getSkillsForScenario = (scenarioId: string) =>
  invoke<ManagedSkill[]>("get_skills_for_scenario", {
    scenarioId,
  });

export const getSkillDocument = (skillId: string) =>
  invoke<SkillDocument>("get_skill_document", { skillId });

export const deleteManagedSkill = (skillId: string) =>
  invoke<void>("delete_managed_skill", { skillId });

export const installLocal = (sourcePath: string, name?: string) =>
  invoke<void>("install_local", { sourcePath, name: name || null });

export const installGit = (repoUrl: string, name?: string) =>
  invoke<void>("install_git", { repoUrl, name: name || null });

export const installFromSkillssh = (source: string, skillId: string) =>
  invoke<void>("install_from_skillssh", { source, skillId });

export const checkSkillUpdate = (skillId: string, force?: boolean) =>
  invoke<ManagedSkill>("check_skill_update", {
    skillId,
    force: force ?? false,
  });

export const checkAllSkillUpdates = (force?: boolean) =>
  invoke<ManagedSkill[]>("check_all_skill_updates", {
    force: force ?? false,
  });

export const updateSkill = (skillId: string) =>
  invoke<ManagedSkill>("update_skill", { skillId });

export const reimportLocalSkill = (skillId: string) =>
  invoke<ManagedSkill>("reimport_local_skill", { skillId });

// ── Sync ──

export const syncSkillToTool = (skillId: string, tool: string) =>
  invoke<void>("sync_skill_to_tool", { skillId, tool });

export const unsyncSkillFromTool = (skillId: string, tool: string) =>
  invoke<void>("unsync_skill_from_tool", { skillId, tool });

// ── Scan ──

export const scanLocalSkills = () => invoke<ScanResult>("scan_local_skills");

export const importExistingSkill = (sourcePath: string, name?: string) =>
  invoke<void>("import_existing_skill", { sourcePath, name: name || null });

export const importAllDiscovered = () =>
  invoke<void>("import_all_discovered");

// ── Browse ──

export const fetchLeaderboard = (board: string) =>
  invoke<SkillsShSkill[]>("fetch_leaderboard", { board });

export const searchSkillssh = (query: string, limit?: number) =>
  invoke<SkillsShSkill[]>("search_skillssh", {
    query,
    limit: limit ?? null,
  });

// ── Settings ──

export const getSettings = (key: string) =>
  invoke<string | null>("get_settings", { key });

export const setSettings = (key: string, value: string) =>
  invoke<void>("set_settings", { key, value });

export const openCentralRepoInFinder = () =>
  invoke<void>("open_central_repo_in_finder");

// ── Scenarios ──

export const getScenarios = () => invoke<Scenario[]>("get_scenarios");

export const getActiveScenario = () =>
  invoke<Scenario | null>("get_active_scenario");

export const createScenario = (name: string, description?: string, icon?: string) =>
  invoke<Scenario>("create_scenario", {
    name,
    description: description || null,
    icon: icon || null,
  });

export const updateScenario = (
  id: string,
  name: string,
  description?: string,
  icon?: string
) =>
  invoke<void>("update_scenario", {
    id,
    name,
    description: description || null,
    icon: icon || null,
  });

export const deleteScenario = (id: string) =>
  invoke<void>("delete_scenario", { id });

export const switchScenario = (id: string) =>
  invoke<void>("switch_scenario", { id });

export const addSkillToScenario = (skillId: string, scenarioId: string) =>
  invoke<void>("add_skill_to_scenario", { skillId, scenarioId });

export const removeSkillFromScenario = (skillId: string, scenarioId: string) =>
  invoke<void>("remove_skill_from_scenario", { skillId, scenarioId });

export const reorderScenarios = (ids: string[]) =>
  invoke<void>("reorder_scenarios", { ids });
