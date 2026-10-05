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
// Fixture tests operate in temporary folders, independent of the real game.
#[allow(clippy::cfg_not_test)]
fn game_closed() -> Result<()> {
    #[cfg(not(test))]
    {
        let system = sysinfo::System::new_all();
        if system.processes().values().any(|process| {
            let name = process.name().to_string_lossy();
            name.eq_ignore_ascii_case("Minecraft.Windows.exe")
                || name.eq_ignore_ascii_case("Minecraft.Windows")
        }) {
            return Err(BedrockError::new(
                ErrorCode::GameRunning,
                "Close Minecraft before modifying world packs",
            ));
        }
    }
    Ok(())
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
    let mut archive =
        zip::ZipArchive::new(fs::File::open(path).map_err(io_error)?)
            .map_err(invalid)?;
    if archive.len() > 100_000 {
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
        *total = total
            .checked_add(entry.size())
            .ok_or_else(|| invalid("Archive too large"))?;
        if *total > 8 * 1024 * 1024 * 1024 {
            return Err(BedrockError::new(
                ErrorCode::FileTooLarge,
                "Archive too large",
            ));
        }
        let destination = output.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(destination).map_err(io_error)?;
            continue;
        }
        fs::create_dir_all(destination.parent().unwrap()).map_err(io_error)?;
        let size = entry.size();
        let written = std::io::copy(
            &mut (&mut entry).take(size + 1),
            &mut fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination)
                .map_err(io_error)?,
        )
        .map_err(io_error)?;
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
    directory(&backups)?;
    let backup = backups.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&backup).map_err(io_error)?;
    for index in 0..2 {
        let name = files[index].file_name().unwrap().to_string_lossy();
        if let Some(bytes) = &original[index] {
            fs::write(backup.join(&*name), bytes).map_err(io_error)?;
        } else {
            fs::write(backup.join(format!("{name}.absent")), b"")
                .map_err(io_error)?;
        }
    }
    let mut moved = Vec::new();
    let mut written = Vec::new();
    let commit: Result<()> = (|| {
        for pack in &packs {
            let base = world.join(format!("{}_packs", pack.kind));
            directory(&base)?;
            let destination = base.join(format!(
                "orbiont-{}-{}",
                pack.id,
                uuid::Uuid::new_v4()
            ));
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
        let mut rollback_ok = true;
        for index in written.into_iter().rev() {
            let restored = if let Some(bytes) = &original[index] {
                atomic_write(&files[index], bytes)
            } else {
                fs::remove_file(&files[index]).map_err(io_error)
            };
            rollback_ok &= restored.is_ok();
        }
        // Remove only the new folders created by this transaction, never previous packs.
        if rollback_ok {
            let canonical_world = fs::canonicalize(world).map_err(io_error)?;
            for path in moved {
                if plain_path(&path, true).is_ok()
                    && fs::canonicalize(&path).is_ok_and(|resolved| {
                        resolved.starts_with(&canonical_world)
                    })
                {
                    let _ = fs::remove_dir_all(path);
                }
            }
        }
        return Err(io_error(format!(
            "{error}; activation backup: {}",
            backup.display()
        )));
    }
    Ok(packs.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::io::Write;

    const RP: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    const BP: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
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
    fn second_activation_write_failure_restores_first_and_removes_new_packs() {
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
        assert_eq!(fs::read_dir(a.join("resource_packs")).unwrap().count(), 0);
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
