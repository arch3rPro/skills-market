# Usage Overview

## Core Model

Skills-Manager-Plus is built around one central idea: keep a central skills library, then decide how each workflow, agent, and project should consume it.

The main concepts are:

- `Central library`: the main repository where imported skills are stored.
- `Scenario`: a named workflow configuration with its own enabled skill set.
- `Installed tool`: a supported AI coding tool that can receive synced skills.
- `Project workspace`: a project-local skills area that can be compared with the central library.
- `Custom agent`: a user-defined tool path with its own sync configuration.

## Typical Workflow

1. Create a scenario.
2. Install skills through `Skills Store`.
3. Open `Skills Management` and choose which skills are enabled in the current scenario.
4. Sync those skills to installed tools.
5. Use `Project Workspaces` when a repository has its own local skills that need to be reviewed or exchanged.
6. Use `Git Backup` when you want restore points or multi-machine sync.

## Main Navigation

- `Dashboard`: quick status and shortcuts.
- `Skills Management`: manage the central library and local-scan results.
- `Skills Store`: import new skills from external sources.
- `Scenarios`: switch between different workflow setups.
- `Project Workspaces`: inspect and synchronize repository-local skills.
- `Settings`: configure paths, tools, sync, search keys, and environment behavior.
