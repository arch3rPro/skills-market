use super::MAX_SYNC_ARTIFACT_BYTES;
use crate::core::central_repo;
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::{self, Cursor, Read, Write};
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

const MAX_ZIP_ENTRIES: usize = 10_000;

pub struct SkillsBackup {
    _temp_dir: tempfile::TempDir,
    path: PathBuf,
    existed: bool,
}

struct ArchiveEntry {
    path: PathBuf,
    name: String,
    is_dir: bool,
}

pub fn zip_central_skills(dest_path: &Path) -> Result<()> {
    let skills_dir = central_repo::skills_dir();
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = fs::File::create(dest_path)
        .with_context(|| format!("Failed to create skills archive {}", dest_path.display()))?;
    let mut zip = ZipWriter::new(file);
    let mut entries = collect_archive_entries(&skills_dir)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    let fixed_time = DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0)?;
    let base_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(fixed_time);

    for entry in entries {
        if entry.is_dir {
            zip.add_directory(
                ensure_trailing_slash(&entry.name),
                base_options.unix_permissions(0o755),
            )?;
        } else {
            zip.start_file(&entry.name, base_options.unix_permissions(0o644))?;
            let mut source = fs::File::open(&entry.path)
                .with_context(|| format!("Failed to read {}", entry.path.display()))?;
            io::copy(&mut source, &mut zip)?;
        }
    }

    zip.finish()?;
    Ok(())
}

pub fn restore_skills_zip(raw: &[u8]) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let extract_dir = temp_dir.path().join("skills");
    fs::create_dir_all(&extract_dir)?;

    let mut archive = ZipArchive::new(Cursor::new(raw))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        bail!("Skills archive contains too many entries");
    }

    let mut extracted_size = 0u64;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| anyhow::anyhow!("Skills archive contains an unsafe path"))?;
        let enclosed_raw = enclosed.to_string_lossy();
        if !is_safe_zip_name(&enclosed_raw) {
            bail!("Skills archive contains an unsafe path");
        }

        let declared_size = entry.size();
        extracted_size = extracted_size
            .checked_add(declared_size)
            .ok_or_else(|| anyhow::anyhow!("Skills archive is too large"))?;
        if extracted_size > MAX_SYNC_ARTIFACT_BYTES {
            bail!("Skills archive exceeds the maximum sync artifact size");
        }

        let out_path = extract_dir.join(enclosed);
        if !out_path.starts_with(&extract_dir) {
            bail!("Skills archive contains an unsafe path");
        }

        if entry.is_dir() {
            fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = fs::File::create(&out_path)
                .with_context(|| format!("Failed to extract {}", out_path.display()))?;
            let copied = copy_zip_entry_limited(&mut entry, &mut outfile, declared_size)?;
            if copied > declared_size {
                bail!("Skills archive entry exceeded its declared size");
            }
        }
    }

    replace_skills_dir(&extract_dir)?;
    Ok(())
}

pub fn backup_current_skills() -> Result<SkillsBackup> {
    backup_skills_dir(&central_repo::skills_dir())
}

fn backup_skills_dir(current: &Path) -> Result<SkillsBackup> {
    let temp_dir = tempfile::tempdir()?;
    let backup_path = temp_dir.path().join("skills");
    let existed = current.exists();

    if existed {
        copy_dir_recursive(current, &backup_path)?;
    }

    Ok(SkillsBackup {
        _temp_dir: temp_dir,
        path: backup_path,
        existed,
    })
}

pub fn restore_skills_from_backup(backup: &SkillsBackup) -> Result<()> {
    restore_skills_dir_from_backup(backup, &central_repo::skills_dir())
}

fn restore_skills_dir_from_backup(backup: &SkillsBackup, skills_dir: &Path) -> Result<()> {
    remove_path_if_exists(skills_dir)?;

    if backup.existed {
        copy_dir_recursive(&backup.path, skills_dir)?;
    }

    Ok(())
}

fn is_safe_zip_name(name: &str) -> bool {
    let path = Path::new(name);
    if path.as_os_str().is_empty() || path.is_absolute() {
        return false;
    }

    path.components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn collect_archive_entries(skills_dir: &Path) -> Result<Vec<ArchiveEntry>> {
    if !skills_dir.exists() {
        return Ok(Vec::new());
    }

    let root = skills_dir
        .canonicalize()
        .with_context(|| format!("Failed to resolve {}", skills_dir.display()))?;
    let mut entries = Vec::new();

    for entry in WalkDir::new(skills_dir).follow_links(false) {
        let entry = entry?;
        let path = entry.path();
        if path == skills_dir {
            continue;
        }

        let file_type = entry.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if !file_type.is_dir() && !file_type.is_file() {
            continue;
        }

        let canonical = path
            .canonicalize()
            .with_context(|| format!("Failed to resolve {}", path.display()))?;
        if !canonical.starts_with(&root) {
            continue;
        }

        let relative = path.strip_prefix(skills_dir)?;
        let name = zip_name(relative)?;
        if !is_safe_zip_name(&name) {
            continue;
        }

        entries.push(ArchiveEntry {
            path: path.to_path_buf(),
            name,
            is_dir: file_type.is_dir(),
        });
    }

    Ok(entries)
}

fn zip_name(path: &Path) -> Result<String> {
    let name = path
        .components()
        .map(|component| match component {
            Component::Normal(value) => Ok(value.to_string_lossy().to_string()),
            _ => Err(anyhow::anyhow!("Unsafe archive entry path")),
        })
        .collect::<Result<Vec<_>>>()?
        .join("/");

    Ok(name)
}

fn ensure_trailing_slash(name: &str) -> String {
    if name.ends_with('/') {
        name.to_string()
    } else {
        format!("{name}/")
    }
}

fn replace_skills_dir(new_skills_dir: &Path) -> Result<()> {
    let skills_dir = central_repo::skills_dir();
    if let Some(parent) = skills_dir.parent() {
        fs::create_dir_all(parent)?;
    }
    remove_path_if_exists(&skills_dir)?;
    fs::rename(new_skills_dir, &skills_dir).or_else(|_| {
        copy_dir_recursive(new_skills_dir, &skills_dir)?;
        fs::remove_dir_all(new_skills_dir)?;
        Ok(())
    })
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for entry in WalkDir::new(source).follow_links(false) {
        let entry = entry?;
        let relative = entry.path().strip_prefix(source)?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let destination = target.join(relative);
        let file_type = entry.file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            fs::create_dir_all(&destination)?;
        } else if file_type.is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &destination).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    entry.path().display(),
                    destination.display()
                )
            })?;
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
        Ok(_) => fs::remove_file(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn copy_zip_entry_limited<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    declared_size: u64,
) -> Result<u64> {
    let max_read = declared_size.saturating_add(1);
    let copied = io::copy(&mut reader.take(max_read), writer)?;
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_zip_name_rejects_parent_segments() {
        assert!(!is_safe_zip_name("../outside.txt"));
        assert!(!is_safe_zip_name("nested/../../outside.txt"));
        assert!(is_safe_zip_name("skill/SKILL.md"));
    }

    #[test]
    fn backup_restore_restores_original_skills_tree() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skills_dir = temp_dir.path().join("skills");
        let skill_dir = skills_dir.join("original-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "original").unwrap();

        let backup = backup_skills_dir(&skills_dir).unwrap();

        std::fs::remove_dir_all(&skills_dir).unwrap();
        std::fs::create_dir_all(skills_dir.join("changed-skill")).unwrap();
        std::fs::write(skills_dir.join("changed-skill/SKILL.md"), "changed").unwrap();

        restore_skills_dir_from_backup(&backup, &skills_dir).unwrap();

        assert_eq!(
            std::fs::read_to_string(skills_dir.join("original-skill/SKILL.md")).unwrap(),
            "original"
        );
        assert!(!skills_dir.join("changed-skill").exists());
    }
}
