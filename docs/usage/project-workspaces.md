# Project Workspaces

## Purpose

`Project Workspaces` are for repositories that already contain local skill directories or need project-specific skill collaboration.

## What They Do

- Link a project or an external skills root.
- Detect per-agent local skill folders.
- Compare project-local skills against the central library.
- Show whether a skill is in sync, only in project, newer in project, newer in center, or diverged.
- Import project-local skills into the central library.
- Export center skills back into the project.

## Workspace Types

- `Project workspace`: tied to a repository and its local agent skill paths.
- `Linked workspace`: an external skills root managed as a standalone workspace without joining global scenario sync.

## Typical Use Cases

- A repository already contains local Claude Code or Codex skills.
- A team keeps project-specific skills in version control.
- You want to compare local skill edits against your central library before deciding which direction to sync.

## Important Distinction

`Project Workspaces` are not the same as the main central library. They are comparison and exchange surfaces for local project state.
