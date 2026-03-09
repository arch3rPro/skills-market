# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.3] - 2026-03-09

### Added
- In-app auto-update support via tauri-plugin-updater

### Fixed
- Improve update UX with semver comparison, fallback download, and i18n fixes

## [1.1.2] - 2026-03-09

### Added
- Check-for-updates button in Settings page

## [1.1.1] - 2026-03-09

### Added
- Sort market search results by download count

### Fixed
- Debounce market search input to reduce lag and prevent stale results
- Improve light/dark mode color contrast and simplify skill status badges
- Improve text readability across light and dark themes
- Increase font sizes for readability and add CJK font stack
- Increase font sizes and window dimensions for better readability

## [1.1.0] - 2026-03-08

### Added
- Windows and Linux support: cross-platform file manager opening, console window suppression
- Backend command `get_central_repo_path` to expose real repo path to frontend
- Tool adapter fallback strategy for `.config/` paths on Windows

### Changed
- UI text from macOS-specific ("Open in Finder", "Built for macOS") to cross-platform wording
- Settings page now displays dynamic repo path instead of hardcoded `~/.skills-manager/`
- CI Windows smoke check reduced to `cargo check` only (avoids duplicate frontend build)
- Renamed `open_central_repo_in_finder` to `open_central_repo_folder` across backend and frontend

### Fixed
- Windows `explorer.exe` false error due to non-zero exit code on success
- Missing Linux `/home/<user>` → `~` path abbreviation in Settings UI

## [1.0.1] - 2026-03-08

### Added
- GitHub Actions cross-platform build workflow (macOS, Linux, Windows)
- CHANGELOG and macOS troubleshooting guide

### Changed
- Moved sync/unsync buttons from skill card list into SkillDetailPanel
- Moved assets (icon, demo GIFs) from docs/ to assets/
- Set bundle targets to "all" for cross-platform builds

## [1.0.0] - 2025-03-08

### Added
- Initial release of Skills Manager v2 with Tauri backend
- Scenario management: create, rename, delete, and switch scenarios
- Scenario icons and sync engine improvements
- Light/dark theme support with system preference detection
- Global search dialog and help dialog
- Configurable sync mode and startup scenario sync
- External link button for market skill cards
- Market search/filter, error banners, and enhanced confirm dialog
- Skill update checking and updating for git-based skills
- Load-more pagination for market skill list
- Skill deduplication: check central path before installing

### Changed
- Redesigned MySkills card and list layout for compactness
- Unified UI styling with compact, consistent design system
- Paginate market skill list and flatten local scan UI
- Consolidated skill card metadata into a single priority-based status badge
- Compact skill card and list row layout with inline action buttons
- Compact market toolbar layout and redesigned skill cards
- Simplified local install section UI
- Improved skill detail panel rendering and market card layout
- Introduced shared app-page utility classes and standardized UI layout
- Removed global search and topbar; added help button to settings
- Updated app icons

### Fixed
- Replaced CSS `-webkit-app-region` drag with programmatic Tauri drag bar
- Replaced Hammer icon with custom app logo image in sidebar
