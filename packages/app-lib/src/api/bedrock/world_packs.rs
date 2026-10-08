//! Packs are scoped to one existing world. No level data or global packs are changed.
use super::WorldTarget;
use super::{BedrockError, ErrorCode, Result};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

fn invalid(message: impl ToString) -> BedrockError {
    BedrockError::new(ErrorCode::InvalidFile, message)
}
fn io_error(error: impl ToString) -> BedrockError {
    BedrockError::new(ErrorCode::ImportFailed, error)
}
fn plain_path(path: &Path, directory: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path).map_err(io_error)?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = meta.file_type().is_symlink();
    if linked
        || (directory && !meta.is_dir())
        || (!directory && !meta.is_file())
    {
        return Err(invalid("Invalid world content path"));
    }
    Ok(())
}
fn directory(path: &Path) -> Result<()> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            plain_path(path, true)
        }
        Err(error) => Err(io_error(error)),
    }
}
fn game_closed() -> Result<()> {
    super::manage::require_closed(super::manage::game_running())
}
pub(super) fn target_path(
    root: &super::DataRoot,
    target: &WorldTarget,
) -> Result<PathBuf> {
    if root.kind == super::RootKind::Logs
        || target.root_id != root.id
        || !target.path.starts_with("minecraftWorlds/")
        || target.path[16..].is_empty()
        || target.path[16..].contains('/')
    {
        return Err(invalid("Select an existing Bedrock world"));
    }
    let path =
        super::workspace::resolve(root, &target.path).map_err(io_error)?;
    plain_path(&path.join("level.dat"), false)?;
    plain_path(&path.join("db"), true)?;
    Ok(path)
}

struct Pack {
    id: uuid::Uuid,
    version: Value,
    kind: &'static str,
    manifest: Value,
    directory: PathBuf,
}
fn version(value: &Value) -> Result<()> {
    if !value.as_array().is_some_and(|parts| {
        parts.len() == 3
            && parts
                .iter()
                .all(|v| v.as_u64().is_some_and(|n| n <= u32::MAX as u64))
    }) {
        return Err(invalid("Invalid Bedrock pack version"));
    }
    Ok(())
}
fn collect_packs(
    path: &Path,
    stage: &Path,
    packs: &mut Vec<Pack>,
    depth: usize,
    total: &mut u64,
) -> Result<()> {
    if depth > 4 || packs.len() >= 16 {
        return Err(invalid("Too many nested Bedrock packs"));
    }
    crate::util::archive::preflight_file(path).map_err(invalid)?;
    let mut archive =
        zip::ZipArchive::new(fs::File::open(path).map_err(io_error)?)
            .map_err(invalid)?;
    if archive.len() > crate::util::archive::MAX_ARCHIVE_ENTRIES {
        return Err(invalid("Too many archive entries"));
    }
    let manifest = match archive.by_name("manifest.json") {
        Ok(mut entry) => {
            if entry.size() > 512 * 1024 {
                return Err(invalid("Pack manifest too large"));
            }
            let mut bytes = Vec::new();
            (&mut entry)
                .take(512 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() > 512 * 1024 {
                return Err(invalid("Pack manifest too large"));
            }
            Some(
                serde_json::from_slice::<Value>(
                    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes),
                )
                .map_err(invalid)?,
            )
        }
        Err(zip::result::ZipError::FileNotFound) => None,
        Err(error) => return Err(invalid(error)),
    };
    let Some(manifest) = manifest else {
        if archive.index_for_name("level.dat").is_some() {
            return Err(invalid("A world cannot be installed as a pack"));
        }
        let nested = tempfile::tempdir_in(stage).map_err(io_error)?;
        let zip = nested.path().join("bundle.zip");
        super::catalog::require_space(
            stage,
            fs::metadata(path).map_err(io_error)?.len(),
        )?;
        fs::copy(path, &zip).map_err(io_error)?;
        for path in super::catalog::prepare_archive(&zip, nested.path())? {
            collect_packs(&path, stage, packs, depth + 1, total)?;
        }
        return Ok(());
    };
    let id = manifest["header"]["uuid"]
        .as_str()
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .filter(|id| !id.is_nil())
        .ok_or_else(|| invalid("Invalid pack UUID"))?;
    version(&manifest["header"]["version"])?;
    if packs.iter().any(|pack| pack.id == id) {
        return Err(invalid("Duplicate pack UUID in archive"));
    }
    let modules = manifest["modules"]
        .as_array()
        .filter(|modules| !modules.is_empty())
        .ok_or_else(|| invalid("Missing pack modules"))?;
    let resources = modules.iter().all(|module| module["type"] == "resources");
    let behavior = modules.iter().all(|module| {
        matches!(module["type"].as_str(), Some("data" | "script"))
    });
    let kind = if resources {
        "resource"
    } else if behavior {
        "behavior"
    } else {
        return Err(invalid("Unsupported pack modules"));
    };
    let output = stage.join(format!("pack-{}", packs.len()));
    let mut declared = 0;
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(invalid)?;
        crate::util::archive::validate_entry(
            entry.name(),
            entry.size(),
            &mut declared,
        )
        .map_err(invalid)?;
    }
    if total.saturating_add(declared) > crate::util::archive::MAX_EXPANDED_BYTES
    {
        return Err(invalid("Archive exceeds decompression limits"));
    }
    super::catalog::require_space(stage, declared)?;
    fs::create_dir(&output).map_err(io_error)?;
    let mut seen = std::collections::HashSet::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(invalid)?;
        let name = entry.name().to_owned();
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| invalid("Unsafe archive path"))?;
        if name.contains(['\\', ':', '\0'])
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            || !seen.insert(name.to_lowercase())
            || relative.components().any(|part| {
                let name = part.as_os_str().to_string_lossy();
                let stem =
                    name.split('.').next().unwrap_or("").to_ascii_uppercase();
                !matches!(part, std::path::Component::Normal(_))
                    || name.ends_with(['.', ' '])
                    || name.chars().any(|c| c.is_control())
                    || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                    || (stem.len() == 4
                        && (stem.starts_with("COM") || stem.starts_with("LPT"))
                        && stem.as_bytes()[3].is_ascii_digit())
            })
        {
            return Err(invalid("Unsafe archive path"));
        }
        let destination = output.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(destination).map_err(io_error)?;
            continue;
        }
        fs::create_dir_all(destination.parent().unwrap()).map_err(io_error)?;
        let size = entry.size();
        let written = std::io::copy(
            &mut (&mut entry).take(
                size.min(
                    crate::util::archive::MAX_EXPANDED_BYTES
                        .saturating_sub(*total),
                ) + 1,
            ),
            &mut fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)
                .map_err(io_error)?,
        )
        .map_err(io_error)?;
        *total = total
            .checked_add(written)
            .ok_or_else(|| invalid("Archive size overflow"))?;
        if *total > crate::util::archive::MAX_EXPANDED_BYTES {
            return Err(invalid(
                "Archive exceeds streaming decompression limit",
            ));
        }
        if written != size {
            return Err(invalid("Invalid archive entry size"));
        }
    }
    packs.push(Pack {
        id,
        version: manifest["header"]["version"].clone(),
        kind,
        manifest,
        directory: output,
    });
    Ok(())
}
fn read_activation(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            plain_path(path, false)?;
            let mut bytes = Vec::new();
            fs::File::open(path)
                .map_err(io_error)?
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            if bytes.len() > 1024 * 1024 {
                return Err(invalid("World pack list too large"));
            }
            Ok(Some(bytes))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(error)),
    }
}
fn activation(bytes: &Option<Vec<u8>>) -> Result<Vec<Value>> {
    match bytes {
        Some(bytes) => {
            let value: Value = serde_json::from_slice(
                bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes),
            )
            .map_err(invalid)?;
            value
                .as_array()
                .cloned()
                .ok_or_else(|| invalid("Invalid world pack list"))
        }
        None => Ok(Vec::new()),
    }
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(path.parent().unwrap())
        .map_err(io_error)?;
    temp.write_all(bytes).map_err(io_error)?;
    temp.as_file().sync_all().map_err(io_error)?;
    temp.persist(path).map_err(io_error)?;
    Ok(())
}
pub(super) fn install_into_world(
    world: &Path,
    archives: &[PathBuf],
) -> Result<usize> {
    let _lock = super::coordination::mutation()?;
    game_closed()?;
    plain_path(world, true)?;
    plain_path(&world.join("level.dat"), false)?;
    plain_path(&world.join("db"), true)?;
    let files = [
        world.join("world_resource_packs.json"),
        world.join("world_behavior_packs.json"),
    ];
    let original = [read_activation(&files[0])?, read_activation(&files[1])?];
    let mut lists = [activation(&original[0])?, activation(&original[1])?];
    let stage = tempfile::tempdir_in(world).map_err(io_error)?;
    let mut packs = Vec::new();
    let mut total = 0;
    for path in archives {
        collect_packs(path, stage.path(), &mut packs, 0, &mut total)?;
    }
    if packs.is_empty() {
        return Err(invalid("No supported Bedrock packs"));
    }
    for pack in &packs {
        if let Some(dependencies) = pack.manifest.get("dependencies") {
            for dependency in dependencies
                .as_array()
                .ok_or_else(|| invalid("Invalid pack dependencies"))?
            {
                if dependency.get("uuid").is_none()
                    && dependency["module_name"].is_string()
                {
                    continue;
                }
                let id = dependency["uuid"]
                    .as_str()
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    .ok_or_else(|| invalid("Invalid dependency UUID"))?;
                version(&dependency["version"])?;
                let in_bundle = packs.iter().any(|candidate| {
                    candidate.id == id
                        && candidate.version == dependency["version"]
                });
                let active = !packs.iter().any(|candidate| candidate.id == id)
                    && lists.iter().flatten().any(|entry| {
                        entry["pack_id"]
                            .as_str()
                            .and_then(|id| uuid::Uuid::parse_str(id).ok())
                            == Some(id)
                            && entry["version"] == dependency["version"]
                    });
                if !in_bundle && !active {
                    return Err(BedrockError::new(
                        ErrorCode::MissingDependency,
                        "Required pack is not active in this world",
                    ));
                }
            }
        }
        let list = &mut lists[usize::from(pack.kind == "behavior")];
        let entry =
            json!({ "pack_id": pack.id.to_string(), "version": pack.version });
        if let Some(existing) = list.iter_mut().find(|entry| {
            entry["pack_id"]
                .as_str()
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                == Some(pack.id)
        }) {
            *existing = entry;
        } else {
            list.push(entry);
        }
    }
    let updated = [
        serde_json::to_vec_pretty(&lists[0]).map_err(invalid)?,
        serde_json::to_vec_pretty(&lists[1]).map_err(invalid)?,
    ];
    // All archives and both existing lists are validated before any permanent change.
    game_closed()?;
    for index in 0..2 {
        if read_activation(&files[index])? != original[index] {
            return Err(io_error(
                "World packs changed during installation; retry",
            ));
        }
    }
    let backups = world.join(".orbiont-pack-backups");
    let recovery_bytes: u64 = original
        .iter()
        .flatten()
        .map(|bytes| bytes.len() as u64)
        .sum();
    let activation_bytes: u64 =
        updated.iter().map(|bytes| bytes.len() as u64).sum();
    super::catalog::require_space(
        world,
        recovery_bytes
            .saturating_mul(4)
            .saturating_add(activation_bytes.saturating_mul(4)),
    )?;
    directory(&backups)?;
    let backup = backups.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&backup).map_err(io_error)?;
    for index in 0..2 {
        let name = files[index].file_name().unwrap().to_string_lossy();
        if let Some(bytes) = &original[index] {
            atomic_write(&backup.join(&*name), bytes)?;
        } else {
            atomic_write(&backup.join(format!("{name}.absent")), b"")?;
        }
    }
    let mut journal = InstallJournal {
        state: "prepared".into(),
        saved_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io_error)?
            .as_secs(),
        original: original.clone(),
        updated: updated.clone(),
        destinations: packs
            .iter()
            .map(|pack| {
                format!(
                    "{}_packs/orbiont-{}-{}",
                    pack.kind,
                    pack.id,
                    uuid::Uuid::new_v4()
                )
            })
            .collect(),
        diagnostic: None,
    };
    journal.save(&backup)?;
    let mut moved = Vec::new();
    let mut written = Vec::new();
    let commit: Result<()> = (|| {
        for (pack, relative) in packs.iter().zip(&journal.destinations) {
            let base = world.join(format!("{}_packs", pack.kind));
            directory(&base)?;
            let destination = world.join(relative);
            fs::rename(&pack.directory, &destination).map_err(io_error)?;
            moved.push(destination);
        }
        for index in 0..2 {
            atomic_write(&files[index], &updated[index])?;
            written.push(index);
        }
        Ok(())
    })();
    if let Err(error) = commit {
        let mut failures = Vec::new();
        for index in written.into_iter().rev() {
            let restored = if let Some(bytes) = &original[index] {
                atomic_write(&files[index], bytes)
            } else {
                fs::remove_file(&files[index]).map_err(io_error)
            };
            if let Err(rollback) = restored {
                failures.push(rollback.message);
            }
        }
        journal.state = if failures.is_empty() {
            "rolled_back"
        } else {
            "rollback_failed"
        }
        .into();
        journal.diagnostic = Some(format!(
            "{error}; rollback: {}; retained new packs: {}",
            failures.join("; "),
            moved.len()
        ));
        journal.save(&backup)?;
        return Err(io_error(format!(
            "{}; recovery: {}",
            journal.diagnostic.as_deref().unwrap_or(""),
            backup.display()
        )));
    }
    journal.state = "applied".into();
    journal.save(&backup)?;
    Ok(packs.len())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct InstallJournal {
    state: String,
    saved_at: u64,
    original: [Option<Vec<u8>>; 2],
    updated: [Vec<u8>; 2],
    destinations: Vec<String>,
    diagnostic: Option<String>,
}
impl InstallJournal {
    fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec(self).map_err(io_error)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(BedrockError::new(
                ErrorCode::FileTooLarge,
                "Pack recovery metadata exceeds its safe limit",
            ));
        }
        atomic_write(&path.join("install-journal.json"), &bytes)
    }
}
fn load_journal(path: &Path) -> Result<InstallJournal> {
    plain_path(&path.join("install-journal.json"), false)?;
    let bytes =
        fs::File::open(path.join("install-journal.json")).map_err(io_error)?;
    let mut out = Vec::new();
    bytes
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut out)
        .map_err(io_error)?;
    if out.len() > 8 * 1024 * 1024 {
        return Err(invalid("Pack recovery metadata too large"));
    }
    serde_json::from_slice(&out).map_err(invalid)
}
fn backup_path(root: &super::DataRoot, id: &str) -> Result<(PathBuf, PathBuf)> {
    let parts: Vec<_> = id.split(':').collect();
    if parts.len() != 3
        || parts[0] != "pack"
        || uuid::Uuid::parse_str(parts[2]).is_err()
        || parts[1].contains(['/', '\\'])
    {
        return Err(invalid("Invalid pack recovery ID"));
    }
    let world = super::workspace::resolve(
        root,
        &format!("minecraftWorlds/{}", parts[1]),
    )
    .map_err(io_error)?;
    let backup = super::workspace::resolve(
        root,
        &format!(
            "minecraftWorlds/{}/.orbiont-pack-backups/{}",
            parts[1], parts[2]
        ),
    )
    .map_err(io_error)?;
    Ok((world, backup))
}
pub(super) fn recoveries(
    root: &super::DataRoot,
    include_restored: bool,
) -> Result<Vec<super::Recovery>> {
    let worlds = match super::workspace::list_files(root, "minecraftWorlds") {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => return Err(io_error(e)),
    };
    if worlds.limited {
        return Err(io_error(
            "Too many worlds to enumerate all pack recoveries",
        ));
    }
    let mut items = Vec::new();
    for world in worlds.entries.into_iter().filter(|e| e.is_dir) {
        let base = match super::workspace::resolve(
            root,
            &format!("{}/.orbiont-pack-backups", world.path),
        ) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(io_error(e)),
        };
        for entry in fs::read_dir(base).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy().to_string();
            if uuid::Uuid::parse_str(&name).is_err() {
                continue;
            }
            let id = format!("pack:{}:{name}", world.name);
            let Ok((_, path)) = backup_path(root, &id) else {
                continue;
            };
            let (state,saved_at,diagnostic)=match load_journal(&path) {
                Ok(j)=>(j.state,j.saved_at,j.diagnostic),
                Err(error) if path.join("install-journal.json").exists()=> ("partial".into(),0,Some(error.message)),
                Err(_)=>("legacy".into(),entry.metadata().ok().and_then(|m|m.modified().ok()).and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|d.as_secs()).unwrap_or(0),Some("Existing activation-only backup lacks committed revisions; restoration requires unchanged activation files".into())),
            };
            if !include_restored
                && ["restored", "rolled_back"].contains(&state.as_str())
            {
                continue;
            }
            let (size_bytes, size_limited) = super::storage::measure(&path);
            items.push(super::Recovery {
                id,
                root_id: root.id.clone(),
                path: world.path.clone(),
                operation: "install".into(),
                saved_at,
                state,
                source: "world_pack_install".into(),
                size_bytes,
                size_limited,
                diagnostic,
            });
        }
    }
    Ok(items)
}
pub(super) fn preview(root: &super::DataRoot, id: &str) -> Result<()> {
    let (world, backup) = backup_path(root, id)?;
    let files = [
        world.join("world_resource_packs.json"),
        world.join("world_behavior_packs.json"),
    ];
    match load_journal(&backup) {
        Ok(journal) => {
            for (index, file) in files.iter().enumerate() {
                let current = read_activation(file)?;
                if current != journal.original[index]
                    && current.as_deref()
                        != Some(journal.updated[index].as_slice())
                {
                    return Err(io_error(
                        "Activation changed after this installation",
                    ));
                }
            }
        }
        Err(error) if backup.join("install-journal.json").exists() => {
            return Err(error);
        }
        Err(_) => {
            for file in &files {
                let name = file.file_name().unwrap();
                let original = read_activation(&backup.join(name))?;
                let absent =
                    backup.join(format!("{}.absent", name.to_string_lossy()));
                if original.is_none() {
                    plain_path(&absent, false)?;
                }
                if read_activation(file)? != original {
                    return Err(io_error(
                        "Legacy backup has no committed revisions; current activation cannot be safely overwritten",
                    ));
                }
            }
        }
    }
    Ok(())
}
pub(super) fn restore(root: &super::DataRoot, id: &str) -> Result<()> {
    let _lock = super::coordination::mutation()?;
    game_closed()?;
    preview(root, id)?;
    let (world, backup) = backup_path(root, id)?;
    let mut journal = load_journal(&backup).unwrap_or(InstallJournal {
        state: "legacy".into(),
        saved_at: 0,
        original: [None, None],
        updated: [Vec::new(), Vec::new()],
        destinations: vec![],
        diagnostic: None,
    });
    if journal.state == "legacy" {
        journal.state = "restored".into();
        return journal.save(&backup);
    }
    for (index, name) in
        ["world_resource_packs.json", "world_behavior_packs.json"]
            .iter()
            .enumerate()
    {
        let file = world.join(name);
        let result = if let Some(bytes) = &journal.original[index] {
            atomic_write(&file, bytes)
        } else if file.exists() {
            fs::remove_file(&file).map_err(io_error)
        } else {
            Ok(())
        };
        if let Err(e) = result {
            journal.state = "rollback_failed".into();
            journal.diagnostic = Some(e.message.clone());
            journal.save(&backup)?;
            return Err(e);
        }
    }
    journal.state = "restored".into();
    journal.diagnostic = None;
    journal.save(&backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::Write;

    const RP: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const BP: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    #[test]
    fn unsafe_pack_directory_is_rejected_before_reader_or_extraction() {
        let fixture = tempfile::tempdir().unwrap();
        let archive =
            pack(fixture.path(), "rp.mcpack", RP, "resources", json!([]));
        let mut bytes = fs::read(&archive).unwrap();
        let end = bytes.len() - 22;
        bytes[end + 12..end + 16]
            .copy_from_slice(&(17u32 * 1024 * 1024).to_le_bytes());
        fs::write(&archive, bytes).unwrap();
        let stage = tempfile::tempdir().unwrap();
        assert!(
            collect_packs(&archive, stage.path(), &mut Vec::new(), 0, &mut 0)
                .is_err()
        );
        assert_eq!(fs::read_dir(stage.path()).unwrap().count(), 0);
    }
    #[test]
    fn install_recovery_restores_activation_but_keeps_imported_content() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "minecraftWorlds/a");
        let archive = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        install_into_world(&a, &[archive]).unwrap();
        let root = super::super::DataRoot {
            id: "fixture".into(),
            path: dir.path().into(),
            kind: super::super::RootKind::User,
        };
        let recovery = recoveries(&root, false).unwrap().pop().unwrap();
        assert_eq!(recovery.state, "applied");
        preview(&root, &recovery.id).unwrap();
        restore(&root, &recovery.id).unwrap();
        assert!(!a.join("world_resource_packs.json").exists());
        assert_eq!(fs::read_dir(a.join("resource_packs")).unwrap().count(), 1);
        assert!(recoveries(&root, false).unwrap().is_empty());
    }
    #[test]
    fn install_recovery_rejects_later_activation_changes() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "minecraftWorlds/a");
        let archive = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        install_into_world(&a, &[archive]).unwrap();
        let root = super::super::DataRoot {
            id: "fixture".into(),
            path: dir.path().into(),
            kind: super::super::RootKind::User,
        };
        let recovery = recoveries(&root, false).unwrap().pop().unwrap();
        fs::write(
            a.join("world_resource_packs.json"),
            b"[{\"pack_id\":\"external\"}]",
        )
        .unwrap();
        assert!(preview(&root, &recovery.id).is_err());
        assert!(restore(&root, &recovery.id).is_err());
        assert_eq!(recoveries(&root, false).unwrap().len(), 1);
    }
    #[test]
    fn world_install_waits_for_the_same_mutation_guard_as_the_editor() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "a");
        let archive = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        let guard = super::super::coordination::mutation().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            tx.send(install_into_world(&a, &[archive])).unwrap();
        });
        assert!(
            rx.recv_timeout(std::time::Duration::from_millis(80))
                .is_err()
        );
        drop(guard);
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            1
        );
        worker.join().unwrap();
    }
    #[test]
    fn legacy_pack_backup_never_overwrites_an_unverifiable_activation() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "minecraftWorlds/a");
        let id = uuid::Uuid::new_v4().to_string();
        let backup = a.join(".orbiont-pack-backups").join(&id);
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("world_resource_packs.json"), b"[]").unwrap();
        fs::write(backup.join("world_behavior_packs.json.absent"), b"")
            .unwrap();
        fs::write(
            a.join("world_resource_packs.json"),
            b"[{\"pack_id\":\"later\"}]",
        )
        .unwrap();
        let root = super::super::DataRoot {
            id: "fixture".into(),
            path: dir.path().into(),
            kind: super::super::RootKind::User,
        };
        let recovery = recoveries(&root, false).unwrap().pop().unwrap();
        assert_eq!(recovery.state, "legacy");
        assert!(preview(&root, &recovery.id).is_err());
        fs::write(a.join("world_resource_packs.json"), b"[]").unwrap();
        preview(&root, &recovery.id).unwrap();
        restore(&root, &recovery.id).unwrap();
        assert!(backup.join("world_resource_packs.json").exists());
    }
    #[test]
    fn prepared_install_journal_with_mixed_activation_files_can_be_recovered() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "minecraftWorlds/a");
        let archive = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        install_into_world(&a, &[archive]).unwrap();
        let root = super::super::DataRoot {
            id: "fixture".into(),
            path: dir.path().into(),
            kind: super::super::RootKind::User,
        };
        let recovery = recoveries(&root, false).unwrap().pop().unwrap();
        let (_, backup) = backup_path(&root, &recovery.id).unwrap();
        let mut journal = load_journal(&backup).unwrap();
        journal.state = "prepared".into();
        journal.save(&backup).unwrap();
        fs::remove_file(a.join("world_behavior_packs.json")).unwrap();
        preview(&root, &recovery.id).unwrap();
        restore(&root, &recovery.id).unwrap();
        assert!(!a.join("world_resource_packs.json").exists());
        assert!(backup.join("install-journal.json").exists());
    }
    #[test]
    fn targets_only_detected_world_directories() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "minecraftWorlds/a");
        let root = super::super::DataRoot {
            id: "user".into(),
            path: dir.path().into(),
            kind: super::super::RootKind::User,
        };
        let target = WorldTarget {
            root_id: "user".into(),
            path: "minecraftWorlds/a".into(),
        };
        assert_eq!(
            target_path(&root, &target).unwrap(),
            fs::canonicalize(a).unwrap()
        );
        for path in [
            "../outside",
            "minecraftWorlds/a/../b",
            "resource_packs/a",
            "minecraftWorlds/",
            "minecraftWorlds/..",
            "minecraftWorlds/a\\outside",
        ] {
            assert!(
                target_path(
                    &root,
                    &WorldTarget {
                        root_id: "user".into(),
                        path: path.into()
                    }
                )
                .is_err()
            );
        }
        assert!(
            target_path(
                &root,
                &WorldTarget {
                    root_id: "other".into(),
                    path: target.path
                }
            )
            .is_err()
        );
    }
    #[test]
    fn addon_bundles_activate_both_packs_without_executing_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "a");
        let rp = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        let bp = pack(
            dir.path(),
            "bp.mcpack",
            BP,
            "data",
            json!([{ "uuid": RP, "version": [1,0,0] }]),
        );
        let path = dir.path().join("addon.mcaddon");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, source) in [("RP.mcpack", rp), ("BP.mcpack", bp)] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&fs::read(source).unwrap()).unwrap();
        }
        zip.finish().unwrap();
        assert_eq!(install_into_world(&a, &[path]).unwrap(), 2);
        assert!(a.join("world_behavior_packs.json").exists());
        assert!(a.join("world_resource_packs.json").exists());
    }
    #[cfg(windows)]
    #[test]
    fn second_activation_write_failure_restores_first_and_retains_new_packs() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "a");
        let original = b"[]";
        fs::write(a.join("world_resource_packs.json"), original).unwrap();
        let blocked = a.join("world_behavior_packs.json");
        fs::write(&blocked, original).unwrap();
        let original_permissions =
            fs::metadata(&blocked).unwrap().permissions();
        let mut perms = original_permissions.clone();
        perms.set_readonly(true);
        fs::set_permissions(&blocked, perms).unwrap();
        let rp = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        let result = install_into_world(&a, &[rp]);
        fs::set_permissions(&blocked, original_permissions).unwrap();
        assert!(result.is_err());
        assert_eq!(
            fs::read(a.join("world_resource_packs.json")).unwrap(),
            original
        );
        assert_eq!(fs::read(blocked).unwrap(), original);
        assert_eq!(fs::read_dir(a.join("resource_packs")).unwrap().count(), 1);
    }
    fn pack(
        dir: &Path,
        name: &str,
        id: &str,
        kind: &str,
        deps: Value,
    ) -> PathBuf {
        let path = dir.join(name);
        let manifest = json!({ "format_version": 2, "header": { "uuid": id, "version": [1,0,0] }, "modules": [{"type": kind, "uuid": "cccccccc-cccc-4ccc-8ccc-cccccccccccc", "version": [1,0,0]}], "dependencies": deps });
        let mut zip =
            zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, bytes) in [
            ("manifest.json", serde_json::to_vec(&manifest).unwrap()),
            ("scripts/main.js", b"// never executed by Orbiont".to_vec()),
        ] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(&bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    fn world(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::create_dir_all(path.join("db")).unwrap();
        std::fs::write(path.join("level.dat"), b"existing-world").unwrap();
        std::fs::write(path.join("db/CURRENT"), b"existing-db").unwrap();
        path
    }
    #[test]
    fn installs_and_activates_only_in_the_selected_world_with_backups() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "a");
        let b = world(dir.path(), "b");
        let old = b"[{\"pack_id\":\"dddddddd-dddd-4ddd-8ddd-dddddddddddd\",\"version\":[1,0,0]}]";
        std::fs::write(a.join("world_resource_packs.json"), old).unwrap();
        let rp = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        let bp = pack(
            dir.path(),
            "bp.mcpack",
            BP,
            "script",
            json!([{ "uuid": RP, "version": [1,0,0] }, { "module_name": "@minecraft/server", "version": "2.0.0" }]),
        );
        assert_eq!(install_into_world(&a, &[rp, bp]).unwrap(), 2);
        let resources: Value = serde_json::from_slice(
            &std::fs::read(a.join("world_resource_packs.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(resources.as_array().unwrap().len(), 2);
        assert_eq!(resources[1]["pack_id"], RP);
        let behaviors: Value = serde_json::from_slice(
            &std::fs::read(a.join("world_behavior_packs.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(behaviors[0]["pack_id"], BP);
        assert!(a.join("resource_packs").is_dir());
        assert!(a.join("behavior_packs").is_dir());
        assert!(a.join(".orbiont-pack-backups").is_dir());
        assert_eq!(
            std::fs::read(a.join("level.dat")).unwrap(),
            b"existing-world"
        );
        assert_eq!(
            std::fs::read(a.join("db/CURRENT")).unwrap(),
            b"existing-db"
        );
        assert!(!b.join("world_resource_packs.json").exists());
        assert!(!b.join("behavior_packs").exists());
    }
    #[test]
    fn invalid_manifests_missing_dependencies_and_bad_existing_json_leave_world_unchanged()
     {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "a");
        let bad =
            pack(dir.path(), "bad.mcpack", "../../outside", "data", json!([]));
        assert!(install_into_world(&a, &[bad]).is_err());
        let dependency = pack(
            dir.path(),
            "bp.mcpack",
            BP,
            "data",
            json!([{ "uuid": RP, "version": [1,0,0] }]),
        );
        assert!(matches!(
            install_into_world(&a, &[dependency]).unwrap_err().code,
            ErrorCode::MissingDependency
        ));
        let rp = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        std::fs::write(a.join("world_resource_packs.json"), b"broken-json")
            .unwrap();
        assert!(install_into_world(&a, &[rp]).is_err());
        assert_eq!(
            std::fs::read(a.join("world_resource_packs.json")).unwrap(),
            b"broken-json"
        );
        assert!(!a.join("resource_packs").exists());
    }
    #[test]
    fn reinstalling_keeps_one_activation_and_retains_previous_pack_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = world(dir.path(), "a");
        let rp = pack(dir.path(), "rp.mcpack", RP, "resources", json!([]));
        install_into_world(&a, std::slice::from_ref(&rp)).unwrap();
        install_into_world(&a, &[rp]).unwrap();
        let activation: Value = serde_json::from_slice(
            &std::fs::read(a.join("world_resource_packs.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(activation.as_array().unwrap().len(), 1);
        assert_eq!(
            std::fs::read_dir(a.join("resource_packs")).unwrap().count(),
            2
        );
    }
}
