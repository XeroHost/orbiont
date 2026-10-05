//! Recoverable edits and removals in detected Bedrock user-data folders.
use super::{BedrockError, DataRoot, Document, ErrorCode, Recovery, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

const LIMIT: usize = 8 * 1024 * 1024;
const BACKUPS: &str = ".orbiont-backups";
static MUTATION: Mutex<()> = Mutex::new(());

fn error(e: impl ToString) -> BedrockError {
    BedrockError::new(ErrorCode::DataUnavailable, e)
}
fn invalid() -> BedrockError {
    BedrockError::new(
        ErrorCode::InvalidPath,
        "Select an editable file, world or pack in a detected data folder",
    )
}
fn conflict() -> BedrockError {
    BedrockError::new(
        ErrorCode::Conflict,
        "This data changed. Reload it before continuing; the recovery copy has been kept",
    )
}
fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn persist(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(error)?;
    file.write_all(bytes).map_err(error)?;
    file.sync_all().map_err(error)
}
fn load(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(error)?;
    let meta = file.metadata().map_err(error)?;
    if !meta.is_file() {
        return Err(invalid());
    }
    if meta.len() > LIMIT as u64 {
        return Err(BedrockError::new(
            ErrorCode::FileTooLarge,
            "Editor files are limited to 8 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > LIMIT {
        return Err(BedrockError::new(
            ErrorCode::FileTooLarge,
            "Editor files are limited to 8 MiB",
        ));
    }
    Ok(bytes)
}
fn resolve(root: &DataRoot, path: &str) -> Result<PathBuf> {
    super::workspace::resolve(root, path).map_err(error)
}
fn editable(root: &DataRoot, path: &str) -> Result<PathBuf> {
    if root.kind == super::RootKind::Logs
        || path.split('/').any(|p| p.starts_with(".orbiont-"))
    {
        return Err(invalid());
    }
    let ext = Path::new(path)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ![
        "txt",
        "json",
        "json5",
        "jsonc",
        "lang",
        "mcfunction",
        "js",
        "ts",
        "md",
        "log",
        "cfg",
        "conf",
        "properties",
        "ini",
        "yaml",
        "yml",
        "toml",
    ]
    .contains(&ext.as_str())
    {
        return Err(invalid());
    }
    resolve(root, path)
}
fn document(bytes: Vec<u8>) -> Result<Document> {
    let revision = revision(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| {
        BedrockError::new(ErrorCode::InvalidFile, "This file is not UTF-8 text")
    })?;
    if text.contains('\0') {
        return Err(BedrockError::new(
            ErrorCode::InvalidFile,
            "This file contains binary data",
        ));
    }
    Ok(Document { text, revision })
}
fn atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path
        .parent()
        .ok_or_else(invalid)?
        .join(format!(".orbiont-edit-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(error)?;
        file.write_all(bytes).map_err(error)?;
        file.sync_all().map_err(error)?;
        drop(file);
        fs::rename(&temporary, path).map_err(error)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[derive(Serialize, Deserialize)]
struct Change {
    root_id: String,
    path: String,
    before: String,
    after: String,
}
#[derive(Serialize, Deserialize)]
struct Journal {
    recovery: Recovery,
    after_revision: Option<String>,
    changes: Vec<Change>,
}
fn create_backup(
    root: &DataRoot,
    path: &str,
    operation: &str,
    after: Option<String>,
    changes: Vec<Change>,
) -> Result<(Journal, PathBuf)> {
    let base = root.path.join(BACKUPS);
    match fs::create_dir(&base) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(error(e)),
    }
    resolve(root, BACKUPS)?;
    let id = uuid::Uuid::new_v4().to_string();
    let directory = base.join(&id);
    fs::create_dir(&directory).map_err(error)?;
    let journal = Journal {
        recovery: Recovery {
            id,
            root_id: root.id.clone(),
            path: path.into(),
            operation: operation.into(),
            saved_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(error)?
                .as_secs(),
        },
        after_revision: after,
        changes,
    };
    let bytes = serde_json::to_vec(&journal).map_err(error)?;
    if bytes.len() > LIMIT {
        return Err(BedrockError::new(
            ErrorCode::FileTooLarge,
            "Recovery metadata exceeds its safe limit",
        ));
    }
    persist(&directory.join("journal.json"), &bytes)?;
    Ok((journal, directory))
}
fn journal(root: &DataRoot, id: &str) -> Result<(Journal, PathBuf)> {
    if uuid::Uuid::parse_str(id).is_err() || id.contains(['/', '\\', ':']) {
        return Err(invalid());
    }
    let directory = resolve(root, &format!("{BACKUPS}/{id}"))?;
    let bytes = load(&resolve(root, &format!("{BACKUPS}/{id}/journal.json"))?)?;
    let journal: Journal = serde_json::from_slice(&bytes).map_err(error)?;
    if journal.recovery.id != id || journal.recovery.root_id != root.id {
        return Err(invalid());
    }
    Ok((journal, directory))
}

pub(super) fn read(root: &DataRoot, path: &str) -> Result<Document> {
    document(load(&editable(root, path)?)?)
}
pub(super) fn write(
    root: &DataRoot,
    path: &str,
    text: &str,
    expected: &str,
) -> Result<Document> {
    let _lock = MUTATION.lock().map_err(error)?;
    require_closed(game_running())?;
    let file = editable(root, path)?;
    let original = load(&file)?;
    if revision(&original) != expected {
        return Err(conflict());
    }
    if text.len() > LIMIT || text.contains('\0') {
        return Err(BedrockError::new(
            ErrorCode::InvalidFile,
            "Invalid editor content",
        ));
    }
    if Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
    {
        serde_json::from_str::<serde_json::Value>(
            text.trim_start_matches('\u{feff}'),
        )
        .map_err(|_| {
            BedrockError::new(
                ErrorCode::InvalidFile,
                "Correct the JSON syntax before saving",
            )
        })?;
    }
    if original == text.as_bytes() {
        return document(original);
    }
    let (_, backup) = create_backup(
        root,
        path,
        "edit",
        Some(revision(text.as_bytes())),
        vec![],
    )?;
    persist(&backup.join("payload"), &original)?;
    require_closed(game_running())?;
    if revision(&load(&editable(root, path)?)?) != expected {
        return Err(conflict());
    }
    atomic(&file, text.as_bytes())?;
    document(text.as_bytes().to_vec())
}
fn category(category: &str) -> bool {
    [
        "resource_packs",
        "behavior_packs",
        "skin_packs",
        "development_resource_packs",
        "development_behavior_packs",
        "development_skin_packs",
    ]
    .contains(&category)
}
fn removable(root: &DataRoot, path: &str) -> Result<(PathBuf, Option<String>)> {
    if root.kind == super::RootKind::Logs {
        return Err(invalid());
    }
    let parts: Vec<_> = path.split('/').collect();
    let world = parts.len() == 2 && parts[0] == "minecraftWorlds";
    let pack = (parts.len() == 2 && category(parts[0]))
        || (parts.len() == 4
            && parts[0] == "minecraftWorlds"
            && category(parts[2]));
    if !world && !pack {
        return Err(invalid());
    }
    let directory = resolve(root, path)?;
    if !directory.is_dir() {
        return Err(invalid());
    }
    if world {
        let marker = resolve(root, &format!("{path}/level.dat"))?;
        if !marker.is_file() {
            return Err(invalid());
        }
        return Ok((directory, None));
    }
    let bytes = load(&resolve(root, &format!("{path}/manifest.json"))?)?;
    let manifest: serde_json::Value = serde_json::from_slice(
        bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes),
    )
    .map_err(error)?;
    let id = manifest["header"]["uuid"].as_str().ok_or_else(invalid)?;
    Ok((
        directory,
        Some(
            uuid::Uuid::parse_str(id)
                .map_err(|_| invalid())?
                .to_string(),
        ),
    ))
}
fn reference_changes(
    roots: &[DataRoot],
    selected: &DataRoot,
    path: &str,
    id: &str,
) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    let mut total_bytes = 0usize;
    let parts: Vec<_> = path.split('/').collect();
    for root in roots
        .iter()
        .filter(|root| root.kind != super::RootKind::Logs)
    {
        if parts.len() == 4 && root.id != selected.id {
            continue;
        }
        let worlds = match super::workspace::list_files(root, "minecraftWorlds")
        {
            Ok(v) => v,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(error(e)),
        };
        if worlds.limited {
            return Err(error(
                "Too many worlds to safely update pack references",
            ));
        }
        for world in worlds.entries.into_iter().filter(|entry| entry.is_dir) {
            if parts.len() == 4
                && world.path != format!("minecraftWorlds/{}", parts[1])
            {
                continue;
            }
            for name in
                ["world_resource_packs.json", "world_behavior_packs.json"]
            {
                let path = format!("{}/{name}", world.path);
                let file = match super::workspace::resolve(root, &path) {
                    Ok(p) => p,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        continue;
                    }
                    Err(e) => return Err(error(e)),
                };
                let before = String::from_utf8(load(&file)?).map_err(error)?;
                let mut refs: serde_json::Value =
                    serde_json::from_str(before.trim_start_matches('\u{feff}'))
                        .map_err(error)?;
                let list = refs.as_array_mut().ok_or_else(invalid)?;
                let old = list.len();
                list.retain(|value| {
                    !value["pack_id"]
                        .as_str()
                        .is_some_and(|v| v.eq_ignore_ascii_case(id))
                });
                if old != list.len() {
                    let after =
                        serde_json::to_string_pretty(&refs).map_err(error)?;
                    total_bytes += before.len() + after.len();
                    if total_bytes > LIMIT / 2 {
                        return Err(BedrockError::new(
                            ErrorCode::FileTooLarge,
                            "Too much world pack metadata to safely change at once",
                        ));
                    }
                    changes.push(Change {
                        root_id: root.id.clone(),
                        path,
                        before,
                        after,
                    });
                }
            }
        }
    }
    Ok(changes)
}
fn change_path(roots: &[DataRoot], change: &Change) -> Result<PathBuf> {
    let root = roots
        .iter()
        .find(|r| r.id == change.root_id)
        .ok_or_else(invalid)?;
    if !change.path.starts_with("minecraftWorlds/")
        || !["world_resource_packs.json", "world_behavior_packs.json"].contains(
            &Path::new(&change.path)
                .file_name()
                .and_then(|v| v.to_str())
                .unwrap_or(""),
        )
    {
        return Err(invalid());
    }
    resolve(root, &change.path)
}
pub(super) fn remove(
    roots: &[DataRoot],
    root: &DataRoot,
    path: &str,
) -> Result<Recovery> {
    let _lock = MUTATION.lock().map_err(error)?;
    require_closed(game_running())?;
    let (target, pack_id) = removable(root, path)?;
    let changes = match pack_id {
        Some(id) => reference_changes(roots, root, path, &id)?,
        None => vec![],
    };
    let (journal, backup) = create_backup(root, path, "delete", None, changes)?;
    require_closed(game_running())?;
    for change in &journal.changes {
        if load(&change_path(roots, change)?)? != change.before.as_bytes() {
            return Err(conflict());
        }
    }
    fs::rename(target, backup.join("payload")).map_err(error)?;
    for (index, change) in journal.changes.iter().enumerate() {
        if let Err(e) = change_path(roots, change)
            .and_then(|path| atomic(&path, change.after.as_bytes()))
        {
            for previous in journal.changes[..index].iter().rev() {
                let _ = change_path(roots, previous)
                    .and_then(|p| atomic(&p, previous.before.as_bytes()));
            }
            let _ = fs::rename(backup.join("payload"), root.path.join(path));
            return Err(e);
        }
    }
    Ok(journal.recovery)
}
pub(super) fn restore(
    roots: &[DataRoot],
    root: &DataRoot,
    id: &str,
) -> Result<()> {
    let _lock = MUTATION.lock().map_err(error)?;
    require_closed(game_running())?;
    let (journal, directory) = journal(root, id)?;
    let payload = resolve(root, &format!("{BACKUPS}/{id}/payload"))?;
    let relative = &journal.recovery.path;
    let parent = Path::new(relative)
        .parent()
        .and_then(|v| v.to_str())
        .ok_or_else(invalid)?;
    let base = resolve(root, parent)?;
    let filename = Path::new(relative).file_name().ok_or_else(invalid)?;
    let target = base.join(filename);
    let mut original = None;
    if journal.recovery.operation == "edit" {
        let existing = editable(root, relative)?;
        let bytes = load(&existing)?;
        if journal.after_revision.as_deref() != Some(&revision(&bytes)) {
            return Err(conflict());
        }
        original = Some(bytes);
    } else if journal.recovery.operation == "delete" {
        // The journal cannot redirect a recovery to arbitrary Minecraft folders.
        let parts: Vec<_> = relative.split('/').collect();
        if !((parts.len() == 2
            && (parts[0] == "minecraftWorlds" || category(parts[0])))
            || (parts.len() == 4
                && parts[0] == "minecraftWorlds"
                && category(parts[2])))
        {
            return Err(invalid());
        }
        if fs::symlink_metadata(&target).is_ok() {
            return Err(conflict());
        }
    } else {
        return Err(invalid());
    }
    for change in &journal.changes {
        if load(&change_path(roots, change)?)? != change.after.as_bytes() {
            return Err(conflict());
        }
    }
    require_closed(game_running())?;
    if original.is_some() {
        atomic(&target, &load(&payload)?)?;
    } else {
        fs::rename(&payload, &target).map_err(error)?;
    }
    for (index, change) in journal.changes.iter().enumerate() {
        if let Err(e) = change_path(roots, change)
            .and_then(|path| atomic(&path, change.before.as_bytes()))
        {
            for previous in journal.changes[..index].iter().rev() {
                let _ = change_path(roots, previous)
                    .and_then(|p| atomic(&p, previous.after.as_bytes()));
            }
            if let Some(bytes) = &original {
                let _ = atomic(&target, bytes);
            } else {
                let _ = fs::rename(&target, &payload);
            }
            return Err(e);
        }
    }
    // Keep recovery data on any failure. Only remove our own two plain files on success.
    if original.is_some() {
        fs::remove_file(payload).map_err(error)?;
    }
    fs::remove_file(directory.join("journal.json")).map_err(error)?;
    fs::remove_dir(directory).map_err(error)?;
    Ok(())
}
pub(super) fn recoveries(root: &DataRoot) -> Result<Vec<Recovery>> {
    let base = match super::workspace::resolve(root, BACKUPS) {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => return Err(error(e)),
    };
    let mut items = Vec::new();
    for entry in fs::read_dir(base).map_err(error)?.take(1000) {
        let entry = entry.map_err(error)?;
        let id = entry.file_name().to_string_lossy().to_string();
        if let Ok((journal, _)) = journal(root, &id)
            && resolve(root, &format!("{BACKUPS}/{id}/payload")).is_ok()
        {
            items.push(journal.recovery);
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.saved_at));
    Ok(items)
}
fn is_game_process(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.eq_ignore_ascii_case("Minecraft.Windows.exe")
        || name.eq_ignore_ascii_case("Minecraft.Windows")
}

#[cfg(windows)]
pub(super) fn stop_game() -> Result<()> {
    let system = sysinfo::System::new_all();
    for process in system.processes().values() {
        if is_game_process(process.name()) {
            if !process.kill() {
                return Err(BedrockError::new(
                    ErrorCode::LaunchFailed,
                    "Windows could not stop Minecraft Bedrock",
                ));
            }
            process.wait();
        }
    }
    Ok(())
}

// Fixture tests must not depend on a game running on the developer's PC.
#[allow(clippy::cfg_not_test)]
pub(super) fn game_running() -> bool {
    #[cfg(not(test))]
    {
        let system = sysinfo::System::new_all();
        system
            .processes()
            .values()
            .any(|p| is_game_process(p.name()))
    }
    #[cfg(test)]
    {
        false
    }
}
pub(super) fn require_closed(running: bool) -> Result<()> {
    if running {
        Err(BedrockError::new(
            ErrorCode::GameRunning,
            "Close Minecraft before editing its data",
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, path::PathBuf};

    #[test]
    fn stopping_bedrock_never_targets_java_or_the_launcher() {
        use std::ffi::OsStr;
        for name in [
            "Minecraft.Windows.exe",
            "minecraft.windows.EXE",
            "Minecraft.Windows",
        ] {
            assert!(is_game_process(OsStr::new(name)));
        }
        for name in [
            "java.exe",
            "javaw.exe",
            "MinecraftLauncher.exe",
            "Minecraft.Windows.exe.bak",
            "Minecraft",
        ] {
            assert!(!is_game_process(OsStr::new(name)));
        }
    }
    struct Fixture(DataRoot);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "orbiont-bedrock-manage-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&path).unwrap();
            Self(DataRoot {
                id: "fixture".into(),
                path,
                kind: super::super::RootKind::User,
            })
        }
        fn file(&self, path: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.path.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0.path).unwrap();
        }
    }
    #[test]
    fn editor_saves_and_restores_original_bytes() {
        let f = Fixture::new();
        let path = f.file("minecraftpe/options.txt", b"sound:1\r\n");
        let doc = read(&f.0, "minecraftpe/options.txt").unwrap();
        let saved =
            write(&f.0, "minecraftpe/options.txt", "sound:0\n", &doc.revision)
                .unwrap();
        assert_ne!(doc.revision, saved.revision);
        assert_eq!(fs::read(&path).unwrap(), b"sound:0\n");
        let backups = recoveries(&f.0).unwrap();
        assert_eq!(backups.len(), 1);
        restore(std::slice::from_ref(&f.0), &f.0, &backups[0].id).unwrap();
        assert_eq!(fs::read(path).unwrap(), b"sound:1\r\n");
    }
    #[test]
    fn editor_refuses_external_changes_and_binary_files() {
        let f = Fixture::new();
        let path = f.file("minecraftpe/options.txt", b"old");
        let doc = read(&f.0, "minecraftpe/options.txt").unwrap();
        fs::write(&path, b"external").unwrap();
        assert!(
            write(&f.0, "minecraftpe/options.txt", "ours", &doc.revision)
                .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), b"external");
        f.file("minecraftWorlds/world/level.dat", b"binary");
        assert!(read(&f.0, "minecraftWorlds/world/level.dat").is_err());
    }
    #[test]
    fn world_removal_is_recoverable_and_never_overwrites_replacement() {
        let f = Fixture::new();
        let path = f.file("minecraftWorlds/world/level.dat", b"level");
        let backup =
            remove(std::slice::from_ref(&f.0), &f.0, "minecraftWorlds/world")
                .unwrap();
        assert!(!path.exists());
        assert_eq!(recoveries(&f.0).unwrap().len(), 1);
        f.file("minecraftWorlds/world/level.dat", b"replacement");
        assert!(restore(std::slice::from_ref(&f.0), &f.0, &backup.id).is_err());
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
        restore(std::slice::from_ref(&f.0), &f.0, &backup.id).unwrap();
        assert_eq!(fs::read(path).unwrap(), b"level");
        assert!(recoveries(&f.0).unwrap().is_empty());
    }
    #[test]
    fn pack_removal_updates_world_references_and_restores_them() {
        let f = Fixture::new();
        f.file("minecraftWorlds/world/level.dat", b"level");
        f.file(
            "minecraftWorlds/world/behavior_packs/addon/manifest.json",
            br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee"}}"#,
        );
        let original = br#"[{"pack_id":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]},{"pack_id":"other","version":[1,0,0]}]"#;
        let active =
            f.file("minecraftWorlds/world/world_behavior_packs.json", original);
        let backup = remove(
            std::slice::from_ref(&f.0),
            &f.0,
            "minecraftWorlds/world/behavior_packs/addon",
        )
        .unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(&active).unwrap()).unwrap();
        assert_eq!(value.as_array().unwrap().len(), 1);
        restore(std::slice::from_ref(&f.0), &f.0, &backup.id).unwrap();
        assert_eq!(fs::read(active).unwrap(), original);
    }
    #[test]
    fn dangerous_paths_and_running_game_are_rejected() {
        let f = Fixture::new();
        f.file("minecraftpe/options.txt", b"options");
        for path in [
            "",
            "../outside",
            "minecraftpe",
            "/absolute",
            "C:/Windows",
            ".orbiont-backups",
        ] {
            assert!(
                remove(std::slice::from_ref(&f.0), &f.0, path).is_err(),
                "{path}"
            );
        }
        assert_eq!(
            require_closed(true).unwrap_err().code,
            ErrorCode::GameRunning
        );
        assert!(require_closed(false).is_ok());
    }
    #[test]
    fn invalid_json_and_oversized_files_leave_original_untouched() {
        let f = Fixture::new();
        let path = f.file("minecraftpe/config.json", br#"{"valid":true}"#);
        let document = read(&f.0, "minecraftpe/config.json").unwrap();
        assert!(
            write(
                &f.0,
                "minecraftpe/config.json",
                "{broken",
                &document.revision
            )
            .is_err()
        );
        assert_eq!(fs::read(path).unwrap(), br#"{"valid":true}"#);
        assert!(recoveries(&f.0).unwrap().is_empty());
        let large = f.file("minecraftpe/large.txt", b"");
        fs::File::options()
            .write(true)
            .open(large)
            .unwrap()
            .set_len((LIMIT + 1) as u64)
            .unwrap();
        assert_eq!(
            read(&f.0, "minecraftpe/large.txt").unwrap_err().code,
            ErrorCode::FileTooLarge
        );
    }
    #[test]
    fn later_edits_prevent_restore_without_losing_recovery_copy() {
        let f = Fixture::new();
        let path = f.file("minecraftpe/options.txt", b"old");
        let document = read(&f.0, "minecraftpe/options.txt").unwrap();
        write(&f.0, "minecraftpe/options.txt", "saved", &document.revision)
            .unwrap();
        let copy = recoveries(&f.0).unwrap().pop().unwrap();
        fs::write(&path, b"later edit").unwrap();
        assert_eq!(
            restore(std::slice::from_ref(&f.0), &f.0, &copy.id)
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
        assert_eq!(fs::read(path).unwrap(), b"later edit");
        assert_eq!(recoveries(&f.0).unwrap().len(), 1);
    }
    #[test]
    fn global_pack_updates_cross_root_references_but_refuses_invalid_world_json()
     {
        let f = Fixture::new();
        let other = Fixture::new();
        let mut user = other.0.clone();
        user.id = "second-user".into();
        let pack = f.file(
            "resource_packs/pack/manifest.json",
            br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee"}}"#,
        );
        let references = other
            .file("minecraftWorlds/world/world_resource_packs.json", b"broken");
        let roots = vec![f.0.clone(), user];
        assert!(remove(&roots, &f.0, "resource_packs/pack").is_err());
        assert!(pack.exists());
        let original = br#"[{"pack_id":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}]"#;
        fs::write(&references, original).unwrap();
        let copy = remove(&roots, &f.0, "resource_packs/pack").unwrap();
        assert_eq!(fs::read(&references).unwrap(), b"[]");
        fs::write(&references, b"[{}]").unwrap();
        assert!(restore(&roots, &f.0, &copy.id).is_err());
        assert!(!pack.exists());
        fs::write(&references, b"[]").unwrap();
        restore(&roots, &f.0, &copy.id).unwrap();
        assert!(pack.exists());
        assert_eq!(fs::read(references).unwrap(), original);
    }
    #[cfg(windows)]
    #[test]
    fn a_locked_second_reference_rolls_back_the_first_and_keeps_the_pack() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let pack = f.file(
            "behavior_packs/pack/manifest.json",
            br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee"}}"#,
        );
        let original = br#"[{"pack_id":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}]"#;
        let first =
            f.file("minecraftWorlds/world/world_resource_packs.json", original);
        let second =
            f.file("minecraftWorlds/world/world_behavior_packs.json", original);
        // Permit reads during validation; deny Windows replacement of the second file.
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&second)
            .unwrap();
        assert!(
            remove(std::slice::from_ref(&f.0), &f.0, "behavior_packs/pack")
                .is_err()
        );
        assert!(pack.exists());
        assert_eq!(fs::read(first).unwrap(), original);
        assert_eq!(fs::read(second).unwrap(), original);
        assert!(recoveries(&f.0).unwrap().is_empty());
        drop(locked);
    }
}
