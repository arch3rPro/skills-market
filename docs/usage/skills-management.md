# Skills Management

![Skills Management](../../assets/skills-management.png)

## Purpose

`Skills Management` is the main operational surface for the central library.

## Tabs

- `Skill Repository`: the central library and current scenario management.
- `Local Skills`: scan results and import flow for locally discovered skills.

## What You Can Do

- Search and filter skills.
- Enable or disable skills for the current scenario.
- Sync or unsync skills to supported tools.
- Review skill docs and metadata.
- Compare local content with upstream source content when available.
- Check for updates and refresh imported skills.
- Edit tags and use batch operations.
- Open version history after Git backup is configured.

## Key Behaviors

### Scenario-Specific Enablement

The central library is not the same as the active scenario. A skill can exist in the library without being enabled for the current scenario.

### Per-Agent Sync

Each skill can be synced to one or more installed tools. Sync coverage is shown directly in the management UI.

### Source Awareness

Skills keep source metadata when imported from Git, marketplace, ClawHub, or plugins. That lets the app track updates or compare against upstream content.

### Batch Operations

You can select multiple skills and perform batch enable, delete, tag, or update operations instead of working item by item.

## Best Practice

Use `Skills Store` to bring skills in, then use `Skills Management` as the long-term control surface for deciding which skills stay active, how they are organized, and where they are synced.
