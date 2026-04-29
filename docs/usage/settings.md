# Settings

## Purpose

`Settings` is where environment-level behavior is configured.

## Main Areas

- Central repository path
- Default sync mode
- Default scenario
- Tool enablement and tool paths
- Custom agents
- Custom-agent sync mode
- Language and theme
- Text size
- Proxy
- SkillsMP API key
- ClawHub API key
- Git backup remote configuration
- WebDAV cloud sync configuration
- App update checks

## Custom Agents

Custom agents let you define additional tool targets beyond the built-in ones.

You can configure:

- display name
- skills path
- optional project workspace path
- independent sync mode

## Search and External Services

`Settings` also controls external-service configuration:

- `SkillsMP API key` for AI-powered marketplace search
- `ClawHub API key` for ClawHub integration
- `proxy` for Git and network requests when needed

### WebDAV Cloud Sync

`WebDAV Cloud Sync` uploads and downloads a full app-state snapshot through a WebDAV-compatible storage service.

The snapshot includes a SQLite metadata export and an archive of the central Skills directory. It is useful when you want multi-device sync without maintaining a Git remote.

Uploading overwrites the remote snapshot. Downloading overwrites local app data after the app creates safety backups, so review the remote metadata before restoring.

First-time flow:

1. Choose a provider preset.
2. Enter the WebDAV URL, username, and app password.
3. Test the connection.
4. Save the configuration.
5. Upload from the source device.
6. On the target device, review the remote metadata, then download.

## Best Practice

Set up `Settings` early, especially repository path, enabled tools, custom agents, and Git remote. That avoids confusion later when managing scenarios or syncing skills.
