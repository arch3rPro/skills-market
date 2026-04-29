# Git Backup

## Purpose

`Git Backup` protects the central library by adding version history, restore points, and multi-machine sync.

## What Gets Backed Up

The app backs up the `skills/` directory inside the current central repository.

By default this lives under:

```text
~/.skills-manager-plus/skills/
```

## Basic Flow

1. Save the remote repository URL in `Settings`.
2. Open `Skills Management`.
3. Run the initial backup flow.
4. Use `Sync to Git` for ongoing pull, commit, and push operations.
5. Use `Version History` to inspect and restore snapshots.

## Restore Model

Restoring a version creates a new restore commit instead of deleting later history. That keeps recovery operations safer and auditable.

## Git Backup and WebDAV Cloud Sync

`Git Backup` and `WebDAV Cloud Sync` solve different problems.

`Git Backup` gives the central Skills files version history and Git-based sync. `WebDAV Cloud Sync` transfers a full app-state snapshot, including database metadata and Skills files.

After restoring from WebDAV, Git Backup may show local file changes. The app does not automatically commit or push those changes; review them before running Git sync.

## Important Note

The SQLite database is not included in Git backup. The app treats the skill files as the durable source and can rebuild metadata by scanning them again.
