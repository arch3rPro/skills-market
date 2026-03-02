# Skills Manager

Desktop app for managing AI agent skills in one place.

[中文说明](./README.zh-CN.md)

## What It Does

Skills Manager is a Tauri desktop application for collecting, organizing, and syncing skill packs across multiple AI coding tools. It keeps a central local repository under `~/.skills-manager`, tracks installed skills in SQLite, and lets you enable different skill sets through scenarios.

Current capabilities implemented in this repo:

- Install skills from local folders, `.zip` / `.skill` files, Git repositories, and the `skills.sh` marketplace
- Scan existing tool skill directories and import discovered skills into the central repository
- Sync or unsync skills to supported tools using symlinks or copies
- Group skills into scenarios and switch the active scenario
- Check for updates for Git-based skills and refresh imported/local skills
- Preview a skill document from `SKILL.md`, `README.md`, or similar files
- Manage app settings including language, theme, default scenario, and sync mode

## Supported Tools

The app currently knows how to detect and sync skills for:

- Cursor
- Claude Code
- Codex
- OpenCode
- Antigravity
- Amp
- Kilo Code
- Roo Code
- Goose
- Gemini CLI
- GitHub Copilot
- Clawdbot
- Droid
- Windsurf
- TRAE IDE

Tool detection is based on whether the corresponding home-directory config folder exists.

## How It Works

Skills are normalized into a central local repository:

- Central repo: `~/.skills-manager`
- Managed skills: `~/.skills-manager/skills`
- Scenario metadata: `~/.skills-manager/scenarios`
- Cache and logs: `~/.skills-manager/cache`, `~/.skills-manager/logs`
- Database: `~/.skills-manager/skills-manager.db`

When you sync a skill to a tool, the app writes the skill into that tool's skill directory, either by symlink or by copy, depending on settings and tool compatibility.

## Tech Stack

- Frontend: React 19, TypeScript, Vite, Tailwind CSS
- Desktop shell: Tauri 2
- Backend: Rust
- Storage: SQLite via `rusqlite`
- Localization: `react-i18next`

## Project Structure

```text
.
├── src/                # React frontend
├── src-tauri/          # Tauri + Rust backend
├── public/             # Static assets
├── docs/               # Extra notes/assets
└── README.zh-CN.md     # Chinese README
```

## Development

### Prerequisites

- Node.js 18+
- npm
- Rust toolchain
- Tauri system dependencies for your OS

On macOS, this project targets the normal Tauri desktop workflow.

### Install Dependencies

```bash
npm install
```

### Run In Development

```bash
npm run tauri:dev
```

If you only need the frontend:

```bash
npm run dev
```

### Build

```bash
npm run tauri:build
```

Frontend-only build:

```bash
npm run build
```

### Lint

```bash
npm run lint
```

## Main Screens

- Dashboard: active scenario, synced skill count, supported tool status
- My Skills: browse managed skills, toggle scenario membership, sync/unsync, update, delete
- Install Skills: browse `skills.sh`, install from Git, install local sources, scan/import existing skills
- Settings: tool detection, central repo access, sync mode, theme, language, active/default scenario

## Notes

- Local and imported skills are marked as `local_only` and can be re-imported rather than Git-updated.
- Git and `skills.sh` installs keep source metadata so update checks can compare revisions.
- On startup, the app restores the preferred default scenario when available and syncs that scenario's skills.
- An older `~/.agent-skills` directory is migrated to `~/.skills-manager` if present.

## License

MIT
