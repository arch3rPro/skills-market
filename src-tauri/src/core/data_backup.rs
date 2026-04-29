use super::{
    crypto, migrations,
    skill_store::{SkillStore, SENSITIVE_KEYS},
    webdav_sync::{redact_settings_for_export, WEBDAV_SETTINGS_KEY},
};
use anyhow::{bail, Context, Result};
use chrono::{Local, Utc};
use rusqlite::{backup::Backup, types::ValueRef, Connection};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const SQL_EXPORT_HEADER: &str = "-- Skills-Manager-Plus SQLite export";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataBackupEntry {
    pub filename: String,
    pub size_bytes: u64,
    pub created_at: String,
}

impl SkillStore {
    pub fn export_data_sql_string(&self) -> Result<String> {
        let snapshot = self.snapshot_to_memory()?;
        dump_sql(&snapshot, Some(&self.secret_key))
    }

    pub fn export_data_backup(&self, target_path: &Path) -> Result<()> {
        let sql = self.export_data_sql_string()?;
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        fs::write(target_path, sql)
            .with_context(|| format!("Failed to write {}", target_path.display()))
    }

    pub fn import_data_backup(&self, source_path: &Path) -> Result<String> {
        let sql = fs::read_to_string(source_path)
            .with_context(|| format!("Failed to read {}", source_path.display()))?;
        self.import_data_sql_string(&sql)
    }

    pub fn import_data_sql_string(&self, sql_raw: &str) -> Result<String> {
        let sql = sql_raw.trim_start_matches('\u{feff}');
        validate_sql_export(sql)?;

        let safety_path = self.create_backup_file()?;
        let safety_id = backup_id_from_path(&safety_path);

        let temp_file = NamedTempFile::new().context("Failed to create temporary database")?;
        let temp_conn =
            Connection::open(temp_file.path()).context("Failed to open temporary database")?;
        temp_conn
            .execute_batch(sql)
            .context("Failed to execute SQL backup")?;
        migrations::run_migrations(&temp_conn)?;
        validate_imported_database(&temp_conn)?;

        {
            let mut main_conn = self.conn.lock().unwrap();
            {
                let backup = Backup::new(&temp_conn, &mut main_conn)?;
                backup.step(-1)?;
            }
            main_conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        }

        Ok(safety_id)
    }

    pub fn create_data_backup(&self) -> Result<String> {
        let backup_path = self.create_backup_file()?;
        Ok(backup_id_from_path(&backup_path))
    }

    pub fn list_data_backups(&self) -> Result<Vec<DataBackupEntry>> {
        let backup_dir = self.backup_dir();
        if !backup_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&backup_dir)
            .with_context(|| format!("Failed to read {}", backup_dir.display()))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "db"))
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                let created_at = metadata
                    .modified()
                    .ok()
                    .map(|time| {
                        let dt: chrono::DateTime<Utc> = time.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default();
                Some(DataBackupEntry {
                    filename: entry.file_name().to_string_lossy().to_string(),
                    size_bytes: metadata.len(),
                    created_at,
                })
            })
            .collect::<Vec<_>>();

        entries.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(entries)
    }

    pub fn restore_data_backup(&self, filename: &str) -> Result<String> {
        validate_backup_filename(filename)?;
        let backup_path = self.backup_dir().join(filename);
        if !backup_path.exists() {
            bail!("Backup file not found: {filename}");
        }

        let safety_path = self.create_backup_file()?;
        let safety_id = backup_id_from_path(&safety_path);
        let source_conn =
            Connection::open(&backup_path).with_context(|| format!("Failed to open {filename}"))?;
        migrations::run_migrations(&source_conn)?;
        validate_imported_database(&source_conn)?;

        {
            let mut main_conn = self.conn.lock().unwrap();
            {
                let backup = Backup::new(&source_conn, &mut main_conn)?;
                backup.step(-1)?;
            }
            main_conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        }

        Ok(safety_id)
    }

    pub fn rename_data_backup(&self, old_filename: &str, new_name: &str) -> Result<String> {
        validate_backup_filename(old_filename)?;
        let new_filename = normalized_new_backup_filename(new_name)?;
        let backup_dir = self.backup_dir();
        let old_path = backup_dir.join(old_filename);
        let new_path = backup_dir.join(&new_filename);

        if !old_path.exists() {
            bail!("Backup file not found: {old_filename}");
        }
        if new_path.exists() {
            bail!("A backup named '{new_filename}' already exists");
        }

        fs::rename(&old_path, &new_path)
            .with_context(|| format!("Failed to rename {}", old_path.display()))?;
        Ok(new_filename)
    }

    pub fn delete_data_backup(&self, filename: &str) -> Result<()> {
        validate_backup_filename(filename)?;
        let backup_path = self.backup_dir().join(filename);
        if !backup_path.exists() {
            bail!("Backup file not found: {filename}");
        }
        fs::remove_file(&backup_path)
            .with_context(|| format!("Failed to delete {}", backup_path.display()))
    }

    fn backup_dir(&self) -> PathBuf {
        self.db_path
            .parent()
            .map(|parent| parent.join("backups"))
            .unwrap_or_else(|| PathBuf::from("backups"))
    }

    fn create_backup_file(&self) -> Result<PathBuf> {
        let backup_dir = self.backup_dir();
        fs::create_dir_all(&backup_dir)
            .with_context(|| format!("Failed to create {}", backup_dir.display()))?;

        let base_id = format!("data_backup_{}", Local::now().format("%Y%m%d_%H%M%S"));
        let mut backup_id = base_id.clone();
        let mut backup_path = backup_dir.join(format!("{backup_id}.db"));
        let mut counter = 1;
        while backup_path.exists() {
            backup_id = format!("{base_id}_{counter}");
            backup_path = backup_dir.join(format!("{backup_id}.db"));
            counter += 1;
        }

        {
            let conn = self.conn.lock().unwrap();
            let mut dest_conn = Connection::open(&backup_path)
                .with_context(|| format!("Failed to open {}", backup_path.display()))?;
            let backup = Backup::new(&conn, &mut dest_conn)?;
            backup.step(-1)?;
        }

        Ok(backup_path)
    }

    fn snapshot_to_memory(&self) -> Result<Connection> {
        let conn = self.conn.lock().unwrap();
        let mut snapshot = Connection::open_in_memory()?;
        {
            let backup = Backup::new(&conn, &mut snapshot)?;
            backup.step(-1)?;
        }
        Ok(snapshot)
    }
}

fn validate_sql_export(sql: &str) -> Result<()> {
    if sql.trim_start().starts_with(SQL_EXPORT_HEADER) {
        Ok(())
    } else {
        bail!("Only SQL backups exported by Skills-Manager-Plus are supported.")
    }
}

fn validate_imported_database(conn: &Connection) -> Result<()> {
    for table in ["settings", "skills", "scenarios", "projects"] {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )?;
        if exists == 0 {
            bail!("Imported database is missing required table: {table}");
        }
    }
    Ok(())
}

fn dump_sql(conn: &Connection, secret_key: Option<&[u8; 32]>) -> Result<String> {
    let mut output = String::new();
    let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S");
    let user_version: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(0);

    output.push_str(SQL_EXPORT_HEADER);
    output.push('\n');
    output.push_str(&format!("-- Generated at: {timestamp}\n"));
    output.push_str(&format!("-- user_version: {user_version}\n"));
    output.push_str("PRAGMA foreign_keys=OFF;\n");
    output.push_str(&format!("PRAGMA user_version={user_version};\n"));
    output.push_str("BEGIN TRANSACTION;\n");

    let mut stmt = conn.prepare(
        "SELECT type, name, sql
         FROM sqlite_master
         WHERE sql IS NOT NULL AND type IN ('table', 'index', 'trigger', 'view')
         ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1 WHEN 'trigger' THEN 2 ELSE 3 END, name",
    )?;
    let objects = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut tables = Vec::new();
    for (object_type, name, sql) in objects {
        if name.starts_with("sqlite_") {
            continue;
        }
        output.push_str(&sql);
        output.push_str(";\n");
        if object_type == "table" {
            tables.push(name);
        }
    }

    for table in tables {
        let columns = table_columns(conn, &table)?;
        if columns.is_empty() {
            continue;
        }
        let settings_key_idx = columns.iter().position(|column| column == "key");
        let settings_value_idx = columns.iter().position(|column| column == "value");

        let mut stmt = conn.prepare(&format!("SELECT * FROM \"{table}\""))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let mut values = Vec::with_capacity(columns.len());
            for idx in 0..columns.len() {
                if table == "settings"
                    && Some(idx) == settings_value_idx
                    && settings_key_idx.is_some()
                {
                    let key: String = row.get(settings_key_idx.unwrap())?;
                    if SENSITIVE_KEYS.contains(&key.as_str()) {
                        let raw_value: String = row.get(idx)?;
                        let mut export_value = match secret_key {
                            Some(secret_key) if crypto::is_encrypted(&raw_value) => {
                                crypto::decrypt(secret_key, &raw_value)?
                            }
                            _ => raw_value,
                        };
                        if key == WEBDAV_SETTINGS_KEY {
                            export_value = redact_settings_for_export(&export_value);
                        }
                        values.push(format_sql_text(&export_value));
                        continue;
                    }
                }
                values.push(format_sql_value(row.get_ref(idx)?)?);
            }
            let quoted_columns = columns
                .iter()
                .map(|column| format!("\"{column}\""))
                .collect::<Vec<_>>()
                .join(", ");
            output.push_str(&format!(
                "INSERT INTO \"{table}\" ({quoted_columns}) VALUES ({});\n",
                values.join(", ")
            ));
        }
    }

    output.push_str("COMMIT;\nPRAGMA foreign_keys=ON;\n");
    Ok(output)
}

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(columns)
}

fn format_sql_value(value: ValueRef<'_>) -> Result<String> {
    match value {
        ValueRef::Null => Ok("NULL".to_string()),
        ValueRef::Integer(value) => Ok(value.to_string()),
        ValueRef::Real(value) => Ok(value.to_string()),
        ValueRef::Text(bytes) => {
            let text = std::str::from_utf8(bytes)?;
            Ok(format_sql_text(text))
        }
        ValueRef::Blob(bytes) => {
            let mut encoded = String::from("X'");
            for byte in bytes {
                use std::fmt::Write;
                let _ = write!(&mut encoded, "{byte:02X}");
            }
            encoded.push('\'');
            Ok(encoded)
        }
    }
}

fn format_sql_text(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn validate_backup_filename(filename: &str) -> Result<()> {
    if filename.is_empty()
        || filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('\0')
        || !filename.ends_with(".db")
    {
        bail!("Invalid backup filename");
    }
    Ok(())
}

fn normalized_new_backup_filename(new_name: &str) -> Result<String> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        bail!("Backup name cannot be empty");
    }
    let name = trimmed.strip_suffix(".db").unwrap_or(trimmed);
    if name.len() > 100 {
        bail!("Backup name is too long");
    }
    if name.contains("..") || name.contains('/') || name.contains('\\') || name.contains('\0') {
        bail!("Invalid backup name");
    }
    Ok(format!("{name}.db"))
}

fn backup_id_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::core::webdav_sync::{WebDavSyncSettings, WEBDAV_SETTINGS_KEY};
    use crate::core::skill_store::SkillStore;

    fn test_store(name: &str) -> (tempfile::TempDir, SkillStore) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join(format!("{name}.db"));
        let store = SkillStore::new(&db_path).unwrap();
        (dir, store)
    }

    #[test]
    fn export_sql_includes_product_header_and_settings() {
        let (_dir, store) = test_store("export");
        store.set_setting("default_scenario", "scenario-a").unwrap();

        let sql = store.export_data_sql_string().unwrap();

        assert!(sql.starts_with("-- Skills-Manager-Plus SQLite export"));
        assert!(sql.contains("INSERT INTO \"settings\""));
        assert!(sql.contains("'default_scenario'"));
        assert!(sql.contains("'scenario-a'"));
    }

    #[test]
    fn import_sql_rejects_unknown_dump_format() {
        let (_dir, store) = test_store("reject");

        let err = store
            .import_data_sql_string("BEGIN TRANSACTION; COMMIT;")
            .unwrap_err();

        assert!(err.to_string().contains("Only SQL backups exported"));
    }

    #[test]
    fn import_sql_applies_data_and_creates_safety_backup() {
        let (_source_dir, source) = test_store("source");
        source.set_setting("default_scenario", "remote").unwrap();
        let sql = source.export_data_sql_string().unwrap();

        let (target_dir, target) = test_store("target");
        target.set_setting("default_scenario", "local").unwrap();

        let safety_id = target.import_data_sql_string(&sql).unwrap();

        assert!(safety_id.starts_with("data_backup_"));
        assert_eq!(
            target.get_setting("default_scenario").unwrap().as_deref(),
            Some("remote")
        );
        assert!(target_dir
            .path()
            .join("backups")
            .join(format!("{safety_id}.db"))
            .exists());
    }

    #[test]
    fn exported_sensitive_settings_import_with_target_store_key() {
        let (_source_dir, source) = test_store("source-secret");
        source
            .set_setting("git_backup_remote_url", "git@example.com:user/repo.git")
            .unwrap();
        let sql = source.export_data_sql_string().unwrap();

        let (_target_dir, target) = test_store("target-secret");
        target.import_data_sql_string(&sql).unwrap();

        assert_eq!(
            target
                .get_setting("git_backup_remote_url")
                .unwrap()
                .as_deref(),
            Some("git@example.com:user/repo.git")
        );
    }

    #[test]
    fn export_sql_redacts_webdav_password_but_keeps_non_secret_config() {
        let (_dir, store) = test_store("webdav-redacted");
        let settings = WebDavSyncSettings {
            enabled: true,
            base_url: "https://dav.example.com/remote.php/dav/files/alice".to_string(),
            username: "alice".to_string(),
            password: "super-secret-password".to_string(),
            remote_root: "team-sync-root".to_string(),
            profile: "default".to_string(),
            status: Default::default(),
        };
        store
            .set_setting(
                WEBDAV_SETTINGS_KEY,
                &serde_json::to_string(&settings).unwrap(),
            )
            .unwrap();

        let sql = store.export_data_sql_string().unwrap();

        assert!(!sql.contains("super-secret-password"));
        assert!(sql.contains("https://dav.example.com/remote.php/dav/files/alice"));
        assert!(sql.contains("team-sync-root"));
    }

    #[test]
    fn create_list_and_restore_binary_backup() {
        let (_dir, store) = test_store("snapshots");
        store.set_setting("default_scenario", "before").unwrap();

        let backup_id = store.create_data_backup().unwrap();
        store.set_setting("default_scenario", "after").unwrap();

        let backups = store.list_data_backups().unwrap();
        assert!(backups
            .iter()
            .any(|entry| entry.filename == format!("{backup_id}.db")));

        let safety_id = store
            .restore_data_backup(&format!("{backup_id}.db"))
            .unwrap();

        assert!(safety_id.starts_with("data_backup_"));
        assert_eq!(
            store.get_setting("default_scenario").unwrap().as_deref(),
            Some("before")
        );
    }

    #[test]
    fn backup_filename_validation_rejects_path_traversal() {
        let (_dir, store) = test_store("validation");

        let err = store.restore_data_backup("../outside.db").unwrap_err();

        assert!(err.to_string().contains("Invalid backup filename"));
    }

    #[test]
    fn rename_backup_rejects_existing_target_name() {
        let (_dir, store) = test_store("rename");
        let first = store.create_data_backup().unwrap();
        store.set_setting("default_scenario", "changed").unwrap();
        let second = store.create_data_backup().unwrap();

        let err = store
            .rename_data_backup(&format!("{first}.db"), &second)
            .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }
}
