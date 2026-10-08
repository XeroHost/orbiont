//! Recoverable edits and removals in detected Bedrock user-data folders.
use super::{BedrockError, DataRoot, Document, ErrorCode, Recovery, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const LIMIT: usize = 8 * 1024 * 1024;
const BACKUPS: &str = ".orbiont-backups";

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
            state: "prepared".into(),
            source: "management".into(),
            size_bytes: 0,
            size_limited: false,
            diagnostic: None,
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
    let _lock = super::coordination::mutation()?;
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
    let (mut journal, backup) = create_backup(
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
    phase(&mut journal, &backup, "applied", None)?;
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
fn removable(
    root: &DataRoot,
    path: &str,
) -> Result<(PathBuf, Option<(String, Option<serde_json::Value>)>)> {
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
        Some((
            uuid::Uuid::parse_str(id)
                .map_err(|_| invalid())?
                .to_string(),
            manifest["header"].get("version").cloned(),
        )),
    ))
}
fn reference_changes(
    roots: &[DataRoot],
    selected: &DataRoot,
    path: &str,
    id: &str,
    version: Option<&serde_json::Value>,
) -> Result<Vec<Change>> {
    let selected_path = path;
    let mut changes = Vec::new();
    let mut total_bytes = 0usize;
    let parts: Vec<_> = path.split('/').collect();
    for root in roots
        .iter()
        .filter(|root| root.kind != super::RootKind::Logs)
    {
        if (parts.len() == 4 && root.id != selected.id)
            || (parts.len() == 2
                && !super::workspace::accessible_pack_root(root, selected))
        {
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
                let mut retained = Vec::new();
                for value in list.drain(..) {
                    let selected_reference = value["pack_id"]
                        .as_str()
                        .is_some_and(|v| v.eq_ignore_ascii_case(id))
                        && version.is_none_or(|v| value["version"] == *v);
                    if !selected_reference
                        || alternate_pack(
                            roots,
                            root,
                            &world.path,
                            selected,
                            selected_path,
                            id,
                            &value["version"],
                        )?
                    {
                        retained.push(value);
                    }
                }
                *list = retained;
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
                        path: path.clone(),
                        before,
                        after,
                    });
                }
            }
        }
    }
    Ok(changes)
}
fn alternate_pack(
    roots: &[DataRoot],
    world_root: &DataRoot,
    world: &str,
    selected: &DataRoot,
    selected_path: &str,
    id: &str,
    version: &serde_json::Value,
) -> Result<bool> {
    // Embedded content shadows a global location. Preserve an activation whenever
    // another matching location can still provide its exact version.
    let directories = [
        "resource_packs",
        "behavior_packs",
        "development_resource_packs",
        "development_behavior_packs",
    ];
    for root in roots.iter().filter(|r| {
        r.kind != super::RootKind::Logs
            && super::workspace::accessible_pack_root(world_root, r)
    }) {
        for category in directories {
            let mut locations = vec![category.to_string()];
            if root.id == world_root.id {
                locations.insert(0, format!("{world}/{category}"));
            }
            for location in locations {
                let listing =
                    match super::workspace::list_files(root, &location) {
                        Ok(listing) => listing,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            continue;
                        }
                        Err(e) => return Err(error(e)),
                    };
                if listing.limited {
                    return Err(error(
                        "Pack locations exceed the safe reference resolution limit",
                    ));
                }
                for entry in listing.entries.into_iter().filter(|e| e.is_dir) {
                    if root.id == selected.id && entry.path == selected_path {
                        continue;
                    }
                    let manifest = match resolve(
                        root,
                        &format!("{}/manifest.json", entry.path),
                    ) {
                        Ok(p) => p,
                        Err(e) if e.code == ErrorCode::DataUnavailable => {
                            continue;
                        }
                        Err(e) => return Err(e),
                    };
                    let bytes = load(&manifest)?;
                    let value: serde_json::Value = serde_json::from_slice(
                        bytes
                            .strip_prefix(&[0xef, 0xbb, 0xbf])
                            .unwrap_or(&bytes),
                    )
                    .map_err(error)?;
                    if value["header"]["uuid"]
                        .as_str()
                        .is_some_and(|v| v.eq_ignore_ascii_case(id))
                        && value["header"]["version"] == *version
                    {
                        return Ok(true);
                    }
                }
            }
        }
    }
    Ok(false)
}
fn save_journal(journal: &Journal, directory: &Path) -> Result<()> {
    atomic(
        &directory.join("journal.json"),
        &serde_json::to_vec(journal).map_err(error)?,
    )
}
fn phase(
    journal: &mut Journal,
    directory: &Path,
    state: &str,
    diagnostic: Option<String>,
) -> Result<()> {
    journal.recovery.state = state.into();
    journal.recovery.diagnostic = diagnostic;
    save_journal(journal, directory)
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
    let _lock = super::coordination::mutation()?;
    require_closed(game_running())?;
    let (target, pack_id) = removable(root, path)?;
    super::storage::validate_tree(&target)?;
    let changes = match pack_id {
        Some((id, version)) => {
            let changes =
                reference_changes(roots, root, path, &id, version.as_ref())?;
            if !changes.is_empty() {
                let workspace = super::workspace::snapshot(roots.to_vec())
                    .map_err(error)?;
                for change in &changes {
                    let world = Path::new(&change.path)
                        .parent()
                        .and_then(|p| p.to_str())
                        .ok_or_else(invalid)?;
                    for item in &workspace.items {
                        if item.activations.iter().any(|a| {
                            a.root_id == change.root_id && a.world_path == world
                        }) && item.pack_id.as_deref() != Some(&id)
                            && item.dependencies.iter().any(|d| {
                                d.pack_id == id
                                    && version.as_ref().is_none_or(|v| {
                                        serde_json::to_value(&d.version)
                                            .ok()
                                            .as_ref()
                                            == Some(v)
                                    })
                            })
                        {
                            return Err(BedrockError::new(
                                ErrorCode::MissingDependency,
                                "An active world pack depends on this version. Remove the dependent pack first",
                            ));
                        }
                    }
                }
            }
            changes
        }
        None => vec![],
    };
    let (mut journal, backup) =
        create_backup(root, path, "delete", None, changes)?;
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
            let mut failures = Vec::new();
            for previous in journal.changes[..index].iter().rev() {
                if let Err(rollback) = change_path(roots, previous)
                    .and_then(|p| atomic(&p, previous.before.as_bytes()))
                {
                    failures.push(rollback.message);
                }
            }
            if let Err(rollback) = super::storage::copy(
                &backup.join("payload"),
                &root.path.join(path),
            ) {
                failures.push(rollback.to_string());
            }
            let state = if failures.is_empty() {
                "rolled_back"
            } else {
                "rollback_failed"
            };
            let detail = format!(
                "{e}; rollback: {}; recovery {}",
                failures.join("; "),
                journal.recovery.id
            );
            phase(&mut journal, &backup, state, Some(detail.clone()))?;
            return Err(error(detail));
        }
    }
    phase(&mut journal, &backup, "applied", None)?;
    Ok(journal.recovery)
}
pub(super) fn preview(
    roots: &[DataRoot],
    root: &DataRoot,
    id: &str,
) -> Result<()> {
    let (journal, _) = journal(root, id)?;
    if ["restored", "rolled_back"].contains(&journal.recovery.state.as_str()) {
        return Err(conflict());
    }
    let payload = resolve(root, &format!("{BACKUPS}/{id}/payload"))?;
    let relative = &journal.recovery.path;
    let parent = Path::new(relative)
        .parent()
        .and_then(|p| p.to_str())
        .ok_or_else(invalid)?;
    let target = resolve(root, parent)?
        .join(Path::new(relative).file_name().ok_or_else(invalid)?);
    if journal.recovery.operation == "edit" {
        let current = load(&editable(root, relative)?)?;
        if journal.after_revision.as_deref() != Some(&revision(&current))
            && current != load(&payload)?
        {
            return Err(conflict());
        }
    } else if journal.recovery.operation == "delete" {
        super::storage::validate_tree(&payload)?;
        if target.exists() {
            if !["rollback_failed", "restoring"]
                .contains(&journal.recovery.state.as_str())
            {
                return Err(conflict());
            }
            super::storage::can_copy(&payload, &target)?;
        }
    } else {
        return Err(invalid());
    }
    for change in &journal.changes {
        let bytes = load(&change_path(roots, change)?)?;
        if bytes != change.before.as_bytes() && bytes != change.after.as_bytes()
        {
            return Err(conflict());
        }
    }
    Ok(())
}
pub(super) fn restore(
    roots: &[DataRoot],
    root: &DataRoot,
    id: &str,
) -> Result<()> {
    let _lock = super::coordination::mutation()?;
    require_closed(game_running())?;
    let (mut journal, directory) = journal(root, id)?;
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
        if journal.after_revision.as_deref() != Some(&revision(&bytes))
            && bytes != load(&payload)?
        {
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
            if !["rollback_failed", "restoring"]
                .contains(&journal.recovery.state.as_str())
            {
                return Err(conflict());
            }
            super::storage::can_copy(&payload, &target)?;
        }
        super::storage::validate_tree(&payload)?;
    } else {
        return Err(invalid());
    }
    for change in &journal.changes {
        let bytes = load(&change_path(roots, change)?)?;
        if bytes != change.after.as_bytes() && bytes != change.before.as_bytes()
        {
            return Err(conflict());
        }
    }
    require_closed(game_running())?;
    phase(&mut journal, &directory, "restoring", None)?;
    let restoration = if original.is_some() {
        atomic(&target, &load(&payload)?)
    } else {
        super::storage::copy(&payload, &target)
    };
    if let Err(e) = restoration {
        phase(
            &mut journal,
            &directory,
            "rollback_failed",
            Some(e.message.clone()),
        )?;
        return Err(e);
    }
    for (index, change) in journal.changes.iter().enumerate() {
        if let Err(e) = change_path(roots, change)
            .and_then(|path| atomic(&path, change.before.as_bytes()))
        {
            // Keep the restored target and original payload on a partial restore.
            // Retrying accepts each reference in either its original or applied state.
            let detail = format!(
                "{e}; restoration partial at reference {index}; recovery {id}"
            );
            phase(
                &mut journal,
                &directory,
                "rollback_failed",
                Some(detail.clone()),
            )?;
            return Err(error(detail));
        }
    }
    // Keep the immutable editor original until the user explicitly retires it.
    phase(&mut journal, &directory, "restored", None)?;
    Ok(())
}
pub(super) fn recoveries(root: &DataRoot) -> Result<Vec<Recovery>> {
    Ok(all_recoveries(root)?
        .into_iter()
        .filter(|r| !["restored", "rolled_back"].contains(&r.state.as_str()))
        .collect())
}
pub(super) fn all_recoveries(root: &DataRoot) -> Result<Vec<Recovery>> {
    let base = match super::workspace::resolve(root, BACKUPS) {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(vec![]);
        }
        Err(e) => return Err(error(e)),
    };
    let mut items = Vec::new();
    for entry in fs::read_dir(base).map_err(error)? {
        let entry = entry.map_err(error)?;
        let id = entry.file_name().to_string_lossy().to_string();
        if uuid::Uuid::parse_str(&id).is_err() {
            continue;
        }
        let Ok(directory) = resolve(root, &format!("{BACKUPS}/{id}")) else {
            continue;
        };
        match journal(root, &id) {
            Ok((mut journal, directory)) => {
                let (bytes, limited) = super::storage::measure(&directory);
                journal.recovery.size_bytes = bytes;
                journal.recovery.size_limited = limited;
                if !directory.join("payload").exists()
                    && !["restored", "rolled_back"]
                        .contains(&journal.recovery.state.as_str())
                {
                    journal.recovery.state = "partial".into();
                }
                items.push(journal.recovery);
            }
            Err(e) => {
                let (size_bytes, size_limited) =
                    super::storage::measure(&directory);
                items.push(Recovery {
                    id,
                    root_id: root.id.clone(),
                    path: String::new(),
                    saved_at: fs::metadata(&directory)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| {
                            t.duration_since(std::time::UNIX_EPOCH).ok()
                        })
                        .map(|d| d.as_secs())
                        .unwrap_or(0),
                    operation: "unknown".into(),
                    state: "partial".into(),
                    source: "management".into(),
                    size_bytes,
                    size_limited,
                    diagnostic: Some(e.message),
                });
            }
        }
    }
    items.sort_by_key(|item| std::cmp::Reverse(item.saved_at));
    Ok(items)
}
#[cfg(test)]
fn is_game_process(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.eq_ignore_ascii_case("Minecraft.Windows.exe")
        || name.eq_ignore_ascii_case("Minecraft.Windows")
}

// Fixture tests must not depend on a game running on the developer's PC.
#[allow(clippy::cfg_not_test)]
pub(super) fn game_running() -> bool {
    #[cfg(not(test))]
    {
        super::process::running()
    }
    #[cfg(test)]
    {
        false
    }
}
#[allow(
    clippy::cfg_not_test,
    reason = "Fixture mutations must not depend on shared launch state from the developer's game"
)]
pub(super) fn require_closed(running: bool) -> Result<()> {
    #[cfg(not(test))]
    let running = running || super::process::mutation_pending();
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
    fn deleting_an_old_version_preserves_the_active_new_version() {
        let f = Fixture::new();
        let id = "8d9932da-e0fb-4a38-bbaf-3307f93ca4ee";
        f.file(
            "resource_packs/old/manifest.json",
            format!(r#"{{"header":{{"uuid":"{id}","version":[1,0,0]}}}}"#)
                .as_bytes(),
        );
        f.file(
            "resource_packs/new/manifest.json",
            format!(r#"{{"header":{{"uuid":"{id}","version":[2,0,0]}}}}"#)
                .as_bytes(),
        );
        let refs = format!(r#"[{{"pack_id":"{id}","version":[2,0,0]}}]"#);
        let active = f.file(
            "minecraftWorlds/world/world_resource_packs.json",
            refs.as_bytes(),
        );
        remove(std::slice::from_ref(&f.0), &f.0, "resource_packs/old").unwrap();
        assert_eq!(fs::read(active).unwrap(), refs.as_bytes());
    }
    #[test]
    fn deleting_global_pack_preserves_embedded_activation_of_same_uuid() {
        let f = Fixture::new();
        let manifest = br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}}"#;
        f.file("resource_packs/global/manifest.json", manifest);
        f.file(
            "minecraftWorlds/world/resource_packs/local/manifest.json",
            manifest,
        );
        let refs = br#"[{"pack_id":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}]"#;
        let active =
            f.file("minecraftWorlds/world/world_resource_packs.json", refs);
        remove(std::slice::from_ref(&f.0), &f.0, "resource_packs/global")
            .unwrap();
        assert_eq!(fs::read(active).unwrap(), refs);
    }
    #[test]
    fn recovery_enumeration_does_not_hide_newest_after_one_thousand_copies() {
        let f = Fixture::new();
        for n in 0..1005 {
            let (mut item, backup) = create_backup(
                &f.0,
                "minecraftpe/options.txt",
                "edit",
                None,
                vec![],
            )
            .unwrap();
            item.recovery.saved_at = n;
            fs::write(
                backup.join("journal.json"),
                serde_json::to_vec(&item).unwrap(),
            )
            .unwrap();
            persist(&backup.join("payload"), b"old").unwrap();
        }
        let items = recoveries(&f.0).unwrap();
        assert_eq!(items.len(), 1005);
        assert_eq!(items[0].saved_at, 1004);
    }
    #[test]
    fn a_prepared_editor_journal_can_restore_without_overwriting_external_data()
    {
        let f = Fixture::new();
        f.file("minecraftpe/options.txt", b"old");
        let (_, directory) = create_backup(
            &f.0,
            "minecraftpe/options.txt",
            "edit",
            Some(revision(b"new")),
            vec![],
        )
        .unwrap();
        persist(&directory.join("payload"), b"old").unwrap();
        let recovery = recoveries(&f.0).unwrap().pop().unwrap();
        assert_eq!(recovery.state, "prepared");
        preview(std::slice::from_ref(&f.0), &f.0, &recovery.id).unwrap();
        restore(std::slice::from_ref(&f.0), &f.0, &recovery.id).unwrap();
        assert!(directory.join("payload").exists());
        assert_eq!(all_recoveries(&f.0).unwrap()[0].state, "restored");
    }
    #[cfg(windows)]
    #[test]
    fn failed_partial_restore_keeps_payload_and_can_resume() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        f.file("resource_packs/pack/manifest.json",br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}}"#);
        let refs=br#"[{"pack_id":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}]"#;
        let active =
            f.file("minecraftWorlds/world/world_resource_packs.json", refs);
        let recovery =
            remove(std::slice::from_ref(&f.0), &f.0, "resource_packs/pack")
                .unwrap();
        let locked = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&active)
            .unwrap();
        assert!(
            restore(std::slice::from_ref(&f.0), &f.0, &recovery.id).is_err()
        );
        let item = recoveries(&f.0).unwrap().pop().unwrap();
        assert_eq!(item.state, "rollback_failed");
        assert!(item.diagnostic.is_some());
        assert!(
            f.0.path
                .join(format!("{BACKUPS}/{}/payload", recovery.id))
                .exists()
        );
        drop(locked);
        restore(std::slice::from_ref(&f.0), &f.0, &recovery.id).unwrap();
        assert_eq!(fs::read(active).unwrap(), refs);
    }
    #[test]
    fn incomplete_journal_remains_visible_and_cannot_be_silently_discarded() {
        let f = Fixture::new();
        let id = uuid::Uuid::new_v4().to_string();
        f.file(&format!("{BACKUPS}/{id}/journal.json"), b"{interrupted");
        f.file(&format!("{BACKUPS}/{id}/payload"), b"original");
        let items = recoveries(&f.0).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].state, "partial");
        assert!(items[0].diagnostic.is_some());
        assert!(preview(std::slice::from_ref(&f.0), &f.0, &id).is_err());
        assert_eq!(
            fs::read(f.0.path.join(format!("{BACKUPS}/{id}/payload"))).unwrap(),
            b"original"
        );
    }
    #[test]
    fn bom_manifest_exposes_active_dependency_and_blocks_removing_its_required_pack()
     {
        let f = Fixture::new();
        let id = "8d9932da-e0fb-4a38-bbaf-3307f93ca4ee";
        let dependent = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let required = f.file(
            "resource_packs/required/manifest.json",
            format!(r#"{{"header":{{"uuid":"{id}","version":[1,0,0]}}}}"#)
                .as_bytes(),
        );
        let mut manifest = vec![0xef, 0xbb, 0xbf];
        manifest.extend_from_slice(format!(r#"{{"header":{{"uuid":"{dependent}","version":[2,0,0]}},"dependencies":[{{"uuid":"{id}","version":[1,0,0]}}]}}"#).as_bytes());
        let addon = f.file("behavior_packs/dependent/manifest.json", &manifest);
        let refs = format!(r#"[{{"pack_id":"{id}","version":[1,0,0]}}]"#);
        let activation = f.file(
            "minecraftWorlds/world/world_resource_packs.json",
            refs.as_bytes(),
        );
        f.file(
            "minecraftWorlds/world/world_behavior_packs.json",
            format!(r#"[{{"pack_id":"{dependent}","version":[2,0,0]}}]"#)
                .as_bytes(),
        );
        let result =
            remove(std::slice::from_ref(&f.0), &f.0, "resource_packs/required");
        assert_eq!(result.unwrap_err().code, ErrorCode::MissingDependency);
        assert!(required.exists());
        assert_eq!(fs::read(&addon).unwrap(), manifest);
        assert_eq!(fs::read(&activation).unwrap(), refs.as_bytes());
        assert!(recoveries(&f.0).unwrap().is_empty());
        let workspace =
            super::super::workspace::snapshot(vec![f.0.clone()]).unwrap();
        let item = workspace
            .items
            .iter()
            .find(|i| i.path == "behavior_packs/dependent")
            .unwrap();
        assert_eq!(item.pack_id.as_deref(), Some(dependent));
        assert_eq!(item.pack_version, Some(vec![2, 0, 0]));
        assert!(item.active);
        assert_eq!(item.dependencies.len(), 1);
        assert_eq!(item.dependencies[0].pack_id, id);
        assert_eq!(item.dependencies[0].version, vec![1, 0, 0]);
    }
    #[test]
    fn deleting_a_required_active_version_does_not_break_a_dependent_pack() {
        let f = Fixture::new();
        let id = "8d9932da-e0fb-4a38-bbaf-3307f93ca4ee";
        let dependent = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
        let pack = f.file(
            "resource_packs/required/manifest.json",
            format!(r#"{{"header":{{"uuid":"{id}","version":[1,0,0]}}}}"#)
                .as_bytes(),
        );
        f.file("behavior_packs/dependent/manifest.json",format!(r#"{{"header":{{"uuid":"{dependent}","version":[1,0,0]}},"dependencies":[{{"uuid":"{id}","version":[1,0,0]}}]}}"#).as_bytes());
        let refs = f.file(
            "minecraftWorlds/world/world_resource_packs.json",
            format!(r#"[{{"pack_id":"{id}","version":[1,0,0]}}]"#).as_bytes(),
        );
        f.file(
            "minecraftWorlds/world/world_behavior_packs.json",
            format!(r#"[{{"pack_id":"{dependent}","version":[1,0,0]}}]"#)
                .as_bytes(),
        );
        assert_eq!(
            remove(std::slice::from_ref(&f.0), &f.0, "resource_packs/required")
                .unwrap_err()
                .code,
            ErrorCode::MissingDependency
        );
        assert!(pack.exists());
        assert_ne!(fs::read(refs).unwrap(), b"[]");
        assert!(recoveries(&f.0).unwrap().is_empty());
    }
    #[test]
    fn another_users_pack_is_not_a_provider_for_this_world() {
        let f = Fixture::new();
        let other = Fixture::new();
        let mut second = other.0.clone();
        second.id = "second-user".into();
        let manifest=br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}}"#;
        f.file("resource_packs/pack/manifest.json", manifest);
        other.file("resource_packs/pack/manifest.json", manifest);
        let refs=br#"[{"pack_id":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}]"#;
        let own =
            f.file("minecraftWorlds/world/world_resource_packs.json", refs);
        let theirs =
            other.file("minecraftWorlds/world/world_resource_packs.json", refs);
        let roots = vec![f.0.clone(), second];
        remove(&roots, &f.0, "resource_packs/pack").unwrap();
        assert_eq!(fs::read(own).unwrap(), b"[]");
        assert_eq!(fs::read(theirs).unwrap(), refs);
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
        let mut f = Fixture::new();
        f.0.kind = super::super::RootKind::Shared;
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
