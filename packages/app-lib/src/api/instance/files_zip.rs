use super::{FileContext, State, fs, input};
use crate::util::archive;
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Serialize)]
pub struct ExtractDryRunResult {
    pub modpack_name: Option<String>,
    pub conflicting_files: Vec<String>,
}
struct StagedArchive {
    _directory: tempfile::TempDir,
    entries: Vec<(String, PathBuf)>,
}
fn stage_archive(source: &Path, parent: &str) -> crate::Result<StagedArchive> {
    let mut source_file = std::fs::File::open(source)?;
    archive::preflight(&mut source_file)?;
    let mut reader = zip::ZipArchive::new(std::io::BufReader::new(source_file))
        .map_err(|_| input("Invalid ZIP archive"))?;
    if reader.len() > archive::MAX_ARCHIVE_ENTRIES {
        return Err(input("Too many archive entries"));
    }
    let directory = tempfile::tempdir()?;
    let mut entries = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut total = 0;
    // Validate every header before decompressing or changing any destination.
    for index in 0..reader.len() {
        let entry = reader
            .by_index(index)
            .map_err(|_| input("Invalid ZIP entry"))?;
        archive::validate_entry(entry.name(), entry.size(), &mut total)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(input("Archive links are unsupported"));
        }
        if !names.insert(entry.name().trim_end_matches('/').to_lowercase()) {
            return Err(input("Duplicate archive destination"));
        }
    }
    for index in 0..reader.len() {
        let mut entry = reader
            .by_index(index)
            .map_err(|_| input("Invalid ZIP entry"))?;
        if entry.is_dir() {
            continue;
        }
        let name = if parent.is_empty() {
            entry.name().to_string()
        } else {
            format!("{parent}/{}", entry.name())
        };
        let staged = directory.path().join(index.to_string());
        let mut output = std::fs::File::create(&staged)?;
        let mut buffer = vec![0; 64 * 1024].into_boxed_slice();
        let mut received = 0_u64;
        loop {
            let amount = entry.read(&mut buffer)?;
            if amount == 0 {
                break;
            }
            received += amount as u64;
            if received > archive::MAX_ENTRY_BYTES || received > entry.size() {
                return Err(input("ZIP entry exceeds its declared size"));
            }
            output.write_all(&buffer[..amount])?;
        }
        if received != entry.size() {
            return Err(input("ZIP entry is incomplete"));
        }
        output.sync_all()?;
        entries.push((name, staged));
    }
    Ok(StagedArchive {
        _directory: directory,
        entries,
    })
}

fn preserve_backup(backup: tempfile::TempPath) -> PathBuf {
    let path = backup.to_path_buf();
    if let Err(error) = backup.keep() {
        // Even a failure to remove temporary attributes must never delete the copy.
        std::mem::forget(error.path);
    }
    path
}

async fn rollback_committed(
    base: &Path,
    committed: Vec<(PathBuf, Option<tempfile::TempPath>)>,
) -> Vec<String> {
    let mut errors = Vec::new();
    // Always visit every original: returning early would drop the remaining TempPaths.
    for (destination, backup) in committed.into_iter().rev() {
        if let Some(backup) = backup {
            if let Err(error) =
                super::require_no_links(base, &destination).await
            {
                let preserved = preserve_backup(backup);
                errors.push(format!(
                    "{}: {error}; original preserved at {}",
                    destination.display(),
                    preserved.display()
                ));
                continue;
            }
            if let Err(error) = backup.persist(&destination) {
                let reason = error.error.to_string();
                let preserved = preserve_backup(error.path);
                errors.push(format!(
                    "{}: {reason}; original preserved at {}",
                    destination.display(),
                    preserved.display()
                ));
            }
        } else {
            let removal = async {
                super::require_no_links(base, &destination).await?;
                fs::remove_file(&destination).await?;
                Ok::<_, crate::Error>(())
            }
            .await;
            if let Err(error) = removal {
                errors.push(format!("{}: {error}", destination.display()));
            }
        }
    }
    errors
}

pub async fn extract_instance_zip(
    instance_id: &str,
    file_path: &str,
    override_conflicts: bool,
    dry_run: bool,
) -> crate::Result<Option<ExtractDryRunResult>> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let context = FileContext::new(&state, instance_id).await?;
    let source = context.resolve(file_path, true).await?.path().to_path_buf();
    let parent = super::normalized(file_path)
        .rsplit_once('/')
        .map_or("", |(parent, _)| parent)
        .to_string();
    let staged =
        tokio::task::spawn_blocking(move || stage_archive(&source, &parent))
            .await??;
    let mut destinations = Vec::new();
    let mut conflicts = Vec::new();
    for (name, path) in &staged.entries {
        let destination =
            context.resolve(name, true).await?.path().to_path_buf();
        if fs::try_exists(&destination).await? {
            if !fs::metadata(&destination).await?.is_file() {
                return Err(input("ZIP destination is not a file"));
            }
            conflicts.push(name.clone());
        }
        destinations.push((name.clone(), path.clone(), destination));
    }
    if dry_run {
        return Ok(Some(ExtractDryRunResult {
            modpack_name: None,
            conflicting_files: conflicts,
        }));
    }
    let mut committed: Vec<(PathBuf, Option<tempfile::TempPath>)> = Vec::new();
    for (name, staged, destination) in destinations {
        let operation = async {
            if !override_conflicts && fs::try_exists(&destination).await? {
                return Ok(None);
            }
            context.resolve(&name, true).await?;
            let parent = destination
                .parent()
                .ok_or_else(|| input("Invalid ZIP destination"))?;
            fs::create_dir_all(parent).await?;
            context.resolve(&name, true).await?;
            let backup = if fs::try_exists(&destination).await? {
                if !fs::metadata(&destination).await?.is_file() {
                    return Err(input("ZIP destination is not a file"));
                }
                let backup =
                    tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
                fs::copy(&destination, &backup).await?;
                fs::File::options()
                    .write(true)
                    .open(&backup)
                    .await?
                    .sync_all()
                    .await?;
                Some(backup)
            } else {
                None
            };
            let temporary =
                tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
            fs::copy(staged, &temporary).await?;
            fs::File::options()
                .write(true)
                .open(&temporary)
                .await?
                .sync_all()
                .await?;
            context.resolve(&name, true).await?;
            if override_conflicts {
                temporary.persist(&destination)
            } else {
                temporary.persist_noclobber(&destination)
            }
            .map_err(|error| error.error)?;
            Ok::<_, crate::Error>(Some(backup))
        }
        .await;
        match operation {
            Ok(Some(backup)) => committed.push((destination, backup)),
            Ok(None) => continue,
            Err(error) => {
                let rollback_errors =
                    rollback_committed(&context.base, committed).await;
                if !rollback_errors.is_empty() {
                    return Err(input(format!(
                        "ZIP extraction failed: {error}; rollback errors: {}",
                        rollback_errors.join("; ")
                    )));
                }
                return Err(error);
            }
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[tokio::test]
    async fn blocked_first_rollback_preserves_it_and_restores_all_other_originals()
     {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let mut committed = Vec::new();
        let mut copies = Vec::new();
        for name in ["a", "b", "c"] {
            let destination = dir.path().join(name);
            std::fs::write(&destination, format!("replacement-{name}"))
                .unwrap();
            let backup = tempfile::NamedTempFile::new_in(dir.path())
                .unwrap()
                .into_temp_path();
            std::fs::write(&backup, format!("original-{name}")).unwrap();
            copies.push(backup.to_path_buf());
            committed.push((destination, Some(backup)));
        }
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(dir.path().join("c"))
            .unwrap();
        let errors = rollback_committed(dir.path(), committed).await;
        assert_eq!(errors.len(), 1);
        assert_eq!(std::fs::read(dir.path().join("a")).unwrap(), b"original-a");
        assert_eq!(std::fs::read(dir.path().join("b")).unwrap(), b"original-b");
        assert_eq!(
            std::fs::read(dir.path().join("c")).unwrap(),
            b"replacement-c"
        );
        assert_eq!(std::fs::read(&copies[2]).unwrap(), b"original-c");
        assert!(errors[0].contains(&copies[2].display().to_string()));
        drop(lock);
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn blocked_new_file_removal_does_not_drop_other_originals() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("existing");
        std::fs::write(&destination, b"replacement").unwrap();
        let backup = tempfile::NamedTempFile::new_in(dir.path())
            .unwrap()
            .into_temp_path();
        std::fs::write(&backup, b"original").unwrap();
        let new = dir.path().join("new");
        std::fs::write(&new, b"new").unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(&new)
            .unwrap();
        let errors = rollback_committed(
            dir.path(),
            vec![(destination.clone(), Some(backup)), (new.clone(), None)],
        )
        .await;
        assert_eq!(errors.len(), 1);
        assert_eq!(std::fs::read(destination).unwrap(), b"original");
        assert_eq!(std::fs::read(new).unwrap(), b"new");
        drop(lock);
    }
    #[test]
    fn invalid_late_entry_leaves_existing_files_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("bad.zip");
        std::fs::write(dir.path().join("existing"), b"original").unwrap();
        let mut writer =
            zip::ZipWriter::new(std::fs::File::create(&archive).unwrap());
        writer
            .start_file("existing", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"replace").unwrap();
        writer
            .start_file("../outside", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();
        assert!(stage_archive(&archive, "").is_err());
        assert_eq!(
            std::fs::read(dir.path().join("existing")).unwrap(),
            b"original"
        );
    }
}
