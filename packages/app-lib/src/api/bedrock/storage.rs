//! Bounded inventory; removal requires an explicit safe selected recovery/import.
use super::{
    BedrockError, DataRoot, ErrorCode, Result, StorageEntry, StorageSummary,
};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
const MAX_FILES: usize = 50000;
fn err(e: impl ToString) -> BedrockError {
    BedrockError::new(ErrorCode::DataUnavailable, e)
}
fn joined(base: &Path, relative: &Path) -> PathBuf {
    if relative.as_os_str().is_empty() {
        base.to_path_buf()
    } else {
        base.join(relative)
    }
}
fn tree(path: &Path) -> Result<Vec<(PathBuf, bool, u64)>> {
    tree_excluding(path, &[])
}
fn tree_excluding(
    path: &Path,
    excluded: &[PathBuf],
) -> Result<Vec<(PathBuf, bool, u64)>> {
    let mut pending = vec![PathBuf::new()];
    let mut out = Vec::new();
    while let Some(relative) = pending.pop() {
        if excluded.iter().any(|subtree| relative.starts_with(subtree)) {
            continue;
        }
        if out.len() >= MAX_FILES {
            return Err(err("Storage tree exceeds safe traversal limit"));
        }
        let absolute = joined(path, &relative);
        let meta = fs::symlink_metadata(&absolute).map_err(err)?;
        #[cfg(windows)]
        let linked = {
            use std::os::windows::fs::MetadataExt;
            meta.file_attributes() & 0x400 != 0
        };
        #[cfg(not(windows))]
        let linked = meta.file_type().is_symlink();
        if linked || (!meta.is_file() && !meta.is_dir()) {
            return Err(err("Linked storage paths are not permitted"));
        }
        if meta.is_dir() {
            for child in fs::read_dir(&absolute).map_err(err)? {
                if pending.len() + out.len() >= MAX_FILES {
                    return Err(err(
                        "Storage tree exceeds safe traversal limit",
                    ));
                }
                pending.push(relative.join(child.map_err(err)?.file_name()));
            }
        }
        out.push((
            relative,
            meta.is_dir(),
            if meta.is_file() { meta.len() } else { 0 },
        ));
    }
    Ok(out)
}
pub(super) fn measure(path: &Path) -> (u64, bool) {
    measure_excluding(path, &[])
}
fn measure_excluding(path: &Path, excluded: &[PathBuf]) -> (u64, bool) {
    measure_bounded(path, excluded, MAX_FILES)
}
fn plain_metadata(path: &Path) -> Result<fs::Metadata> {
    let meta = fs::symlink_metadata(path).map_err(err)?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = meta.file_type().is_symlink();
    if linked || (!meta.is_file() && !meta.is_dir()) {
        return Err(err("Linked storage paths are not permitted"));
    }
    Ok(meta)
}
fn measure_bounded(
    path: &Path,
    excluded: &[PathBuf],
    limit: usize,
) -> (u64, bool) {
    // Keep only open directory iterators for the current depth. Large folders
    // never allocate a collection containing every child path.
    let mut bytes = 0u64;
    let mut count = 0usize;
    let mut pending = Some(PathBuf::new());
    let mut parents: Vec<(PathBuf, fs::ReadDir)> = Vec::new();
    loop {
        let relative = if let Some(relative) = pending.take() {
            relative
        } else {
            let Some((parent, children)) = parents.last_mut() else {
                break;
            };
            match children.next() {
                Some(Ok(child)) => parent.join(child.file_name()),
                Some(Err(_)) => return (bytes, true),
                None => {
                    parents.pop();
                    continue;
                }
            }
        };
        if excluded.iter().any(|subtree| relative.starts_with(subtree)) {
            continue;
        }
        if count >= limit || parents.len() >= 256 {
            return (bytes, true);
        }
        count += 1;
        let absolute = joined(path, &relative);
        let Ok(meta) = plain_metadata(&absolute) else {
            return (bytes, true);
        };
        if meta.is_file() {
            let Some(total) = bytes.checked_add(meta.len()) else {
                return (bytes, true);
            };
            bytes = total;
        } else {
            let Ok(children) = fs::read_dir(absolute) else {
                return (bytes, true);
            };
            parents.push((relative, children));
        }
    }
    (bytes, false)
}
pub(super) fn validate_tree(path: &Path) -> Result<()> {
    tree(path).map(|_| ())
}
fn same_file(a: &Path, b: &Path) -> Result<bool> {
    use sha2::{Digest, Sha256};
    fn hash(path: &Path) -> Result<Vec<u8>> {
        let mut f = fs::File::open(path).map_err(err)?;
        let mut h = Sha256::new();
        let mut buffer = vec![0; 65536].into_boxed_slice();
        loop {
            let n = f.read(&mut buffer).map_err(err)?;
            if n == 0 {
                break;
            }
            h.update(&buffer[..n]);
        }
        Ok(h.finalize().to_vec())
    }
    Ok(hash(a)? == hash(b)?)
}
pub(super) fn can_copy(source: &Path, target: &Path) -> Result<()> {
    let source_tree: std::collections::HashMap<_, _> = tree(source)?
        .into_iter()
        .map(|(relative, directory, _)| (relative, directory))
        .collect();
    if target.exists() {
        for (relative, directory, _) in tree(target)? {
            if source_tree.get(&relative) != Some(&directory) {
                return Err(err("Recovery destination has extra content"));
            }
            if !directory
                && !same_file(
                    &joined(source, &relative),
                    &joined(target, &relative),
                )?
            {
                return Err(err("Recovery destination was changed"));
            }
        }
    }
    Ok(())
}
fn atomic_copy(
    source: &Path,
    target: &Path,
    staging: &Path,
    transfer: impl FnOnce(&mut fs::File, &mut fs::File) -> std::io::Result<()>,
) -> Result<()> {
    // Staging lives beside the journal/payload, outside both the immutable
    // payload tree and the destination. A crash cannot publish a partial file.
    let mut temporary = tempfile::Builder::new()
        .prefix(".orbiont-restore-")
        .tempfile_in(staging)
        .map_err(err)?;
    let mut input = fs::File::open(source).map_err(err)?;
    transfer(&mut input, temporary.as_file_mut()).map_err(err)?;
    temporary.as_file().sync_all().map_err(err)?;
    temporary.persist_noclobber(target).map_err(err)?;
    Ok(())
}
pub(super) fn copy(source: &Path, target: &Path) -> Result<()> {
    can_copy(source, target)?;
    let mut entries = tree(source)?;
    entries.sort_by_key(|e| e.0.components().count());
    for (relative, directory, _) in entries {
        let dest = joined(target, &relative);
        if directory {
            fs::create_dir_all(dest).map_err(err)?;
        } else if !dest.exists() {
            let staging = source.parent().ok_or_else(|| {
                err("Recovery payload has no staging directory")
            })?;
            atomic_copy(
                &joined(source, &relative),
                &dest,
                staging,
                |input, output| std::io::copy(input, output).map(|_| ()),
            )?;
        }
    }
    Ok(())
}
pub(super) fn remove_tree(path: &Path) -> Result<()> {
    let mut entries = tree(path)?;
    entries.sort_by_key(|e| std::cmp::Reverse(e.0.components().count()));
    for (relative, directory, _) in entries {
        let target = joined(path, &relative);
        // Recheck every parent immediately before removal, including a junction
        // inserted after the initial bounded inventory. Avoid rescanning subtrees.
        plain_metadata(path)?;
        let mut parent = path.to_path_buf();
        for component in relative.components() {
            parent.push(component);
            plain_metadata(&parent)?;
        }
        if directory {
            fs::remove_dir(target).map_err(err)?;
        } else {
            fs::remove_file(target).map_err(err)?;
        }
    }
    Ok(())
}
fn world_exclusions(
    items: &[super::WorkspaceItem],
) -> HashMap<(&str, &Path), Vec<PathBuf>> {
    let mut worlds: HashMap<_, _> = items
        .iter()
        .filter(|item| item.kind == super::ItemKind::World)
        .map(|item| {
            (
                (item.root_id.as_str(), Path::new(&item.path)),
                vec![PathBuf::from(".orbiont-pack-backups")],
            )
        })
        .collect();
    // Index each item's world ancestors once rather than scanning all items
    // again for every world. Paths are compared by components, within a root.
    for item in items
        .iter()
        .filter(|item| item.kind != super::ItemKind::Log)
    {
        let path = Path::new(&item.path);
        for ancestor in path.ancestors().skip(1) {
            if let Some(excluded) =
                worlds.get_mut(&(item.root_id.as_str(), ancestor))
                && let Ok(relative) = path.strip_prefix(ancestor)
            {
                excluded.push(relative.to_path_buf());
            }
        }
    }
    worlds
}
pub(super) fn summary(
    roots: &[DataRoot],
    imports: Option<&Path>,
) -> Result<StorageSummary> {
    let mut entries = Vec::new();
    let mut limited = false;
    for root in roots.iter().filter(|r| r.kind != super::RootKind::Logs) {
        let mut recoveries = super::manage::all_recoveries(root)?;
        recoveries.extend(super::world_packs::recoveries(root, true)?);
        for recovery in recoveries {
            entries.push(StorageEntry {
                id: recovery.id,
                root_id: root.id.clone(),
                path: recovery.path,
                category: "recovery".into(),
                size_bytes: recovery.size_bytes,
                size_limited: recovery.size_limited,
                removable: !recovery.size_limited
                    && ["applied", "restored", "rolled_back", "legacy"]
                        .contains(&recovery.state.as_str()),
            });
        }
    }
    let workspace = super::workspace::snapshot(roots.to_vec()).map_err(err)?;
    limited |= workspace.incomplete;
    let exclusions = world_exclusions(&workspace.items);
    let root_index: HashMap<_, _> =
        roots.iter().map(|root| (root.id.as_str(), root)).collect();
    for item in workspace
        .items
        .iter()
        .filter(|i| i.kind != super::ItemKind::Log)
    {
        let root = root_index
            .get(item.root_id.as_str())
            .ok_or_else(|| err("Storage item has no detected data root"))?;
        let excluded = exclusions
            .get(&(item.root_id.as_str(), Path::new(&item.path)))
            .map_or(&[][..], Vec::as_slice);
        let (size_bytes, size_limited) =
            measure_excluding(&root.path.join(&item.path), excluded);
        entries.push(StorageEntry {
            id: format!("content:{}", item.path),
            root_id: item.root_id.clone(),
            path: item.path.clone(),
            category: if item.pack_id.is_some() && !item.active {
                "retained_version"
            } else {
                "content"
            }
            .into(),
            size_bytes,
            size_limited,
            removable: false,
        });
    }
    if let Some(imports) = imports.filter(|p| p.exists()) {
        for (index, entry) in
            fs::read_dir(imports).map_err(err)?.take(1001).enumerate()
        {
            if index == 1000 {
                limited = true;
                break;
            }
            if entries.len() >= 20000 {
                limited = true;
                break;
            }
            let entry = entry.map_err(err)?;
            let id = entry.file_name().to_string_lossy().to_string();
            let (size_bytes, size_limited) = measure(&entry.path());
            let old = fs::metadata(entry.path())
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|d| d.as_secs() >= 86400);
            entries.push(StorageEntry {
                id: id.clone(),
                root_id: "imports".into(),
                path: id,
                category: "import".into(),
                size_bytes,
                size_limited,
                removable: old && !size_limited,
            });
        }
    }
    entries.sort_by(|a, b| {
        a.category
            .cmp(&b.category)
            .then_with(|| a.path.cmp(&b.path))
    });
    if entries.len() > 20000 {
        limited = true;
        entries.truncate(20000);
    }
    limited |= entries.iter().any(|entry| entry.size_limited);
    Ok(StorageSummary { entries, limited })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_measurement_keeps_a_truthful_partial_count() {
        let fixture = tempfile::tempdir().unwrap();
        fs::write(fixture.path().join("a"), b"abc").unwrap();
        fs::write(fixture.path().join("b"), b"abc").unwrap();
        assert_eq!(measure_bounded(fixture.path(), &[], 2), (3, true));
        assert_eq!(measure_bounded(fixture.path(), &[], 3), (6, false));
        assert!(fixture.path().join("a").exists());
    }
    #[test]
    fn recovery_and_content_categories_partition_world_bytes_without_overlap() {
        let fixture = tempfile::tempdir().unwrap();
        let root = DataRoot {
            id: "fixture".into(),
            path: fixture.path().into(),
            kind: super::super::RootKind::User,
        };
        let world = root.path.join("minecraftWorlds/world");
        let pack = world.join("resource_packs/embedded");
        fs::create_dir_all(&pack).unwrap();
        fs::write(world.join("level.dat"), b"world-data").unwrap();
        fs::write(pack.join("manifest.json"),br#"{"header":{"uuid":"8d9932da-e0fb-4a38-bbaf-3307f93ca4ee","version":[1,0,0]}}"#).unwrap();
        fs::write(pack.join("texture.bin"), b"pack-data").unwrap();
        let backup = world
            .join(".orbiont-pack-backups")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&backup).unwrap();
        fs::write(backup.join("world_resource_packs.json"), b"[]").unwrap();
        fs::write(backup.join("world_behavior_packs.json.absent"), b"")
            .unwrap();
        let result = summary(std::slice::from_ref(&root), None).unwrap();
        assert!(!result.limited);
        assert!(result.entries.iter().all(|e| !e.size_limited));
        let total: u64 = result.entries.iter().map(|e| e.size_bytes).sum();
        assert_eq!(total, measure(&world).0);
        assert_eq!(
            result
                .entries
                .iter()
                .find(|e| e.path == "minecraftWorlds/world"
                    && e.category == "content")
                .unwrap()
                .size_bytes,
            b"world-data".len() as u64
        );
        assert_eq!(
            result
                .entries
                .iter()
                .find(|e| e.category == "retained_version")
                .unwrap()
                .size_bytes,
            measure(&pack).0
        );
        assert_eq!(
            result
                .entries
                .iter()
                .find(|e| e.category == "recovery")
                .unwrap()
                .size_bytes,
            measure(&backup).0
        );
        assert!(world.join("level.dat").exists());
        assert!(pack.join("texture.bin").exists());
        assert!(backup.join("world_resource_packs.json").exists());
    }
    #[test]
    fn interrupted_file_copy_never_publishes_partial_destination_and_keeps_original()
     {
        use std::io::Write;
        let fixture = tempfile::tempdir().unwrap();
        let original = fixture.path().join("original");
        let destination = fixture.path().join("destination");
        fs::write(&original, b"complete original").unwrap();
        let failed = atomic_copy(
            &original,
            &destination,
            fixture.path(),
            |_, output| {
                output.write_all(b"incomplete")?;
                Err(std::io::Error::other("simulated storage failure"))
            },
        );
        assert!(failed.is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read(&original).unwrap(), b"complete original");
        atomic_copy(
            &original,
            &destination,
            fixture.path(),
            |input, output| std::io::copy(input, output).map(|_| ()),
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"complete original");
        assert!(
            atomic_copy(
                &original,
                &destination,
                fixture.path(),
                |input, output| std::io::copy(input, output).map(|_| ())
            )
            .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"complete original");
    }
    #[test]
    fn inventory_separates_recoveries_imports_active_content_and_retained_versions()
     {
        let fixture = tempfile::tempdir().unwrap();
        let root = DataRoot {
            id: "fixture".into(),
            path: fixture.path().join("data"),
            kind: super::super::RootKind::User,
        };
        let id = "8d9932da-e0fb-4a38-bbaf-3307f93ca4ee";
        for (folder, version) in [("old", 1), ("active", 2)] {
            let path = root.path.join(format!("resource_packs/{folder}"));
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("manifest.json"),format!(r#"{{"header":{{"uuid":"{id}","version":[{version},0,0]}}}}"#)).unwrap();
        }
        fs::create_dir_all(root.path.join("minecraftWorlds/world")).unwrap();
        fs::write(root.path.join("minecraftWorlds/world/level.dat"), b"world")
            .unwrap();
        fs::write(
            root.path
                .join("minecraftWorlds/world/world_resource_packs.json"),
            format!(r#"[{{"pack_id":"{id}","version":[2,0,0]}}]"#),
        )
        .unwrap();
        fs::create_dir_all(root.path.join("minecraftpe")).unwrap();
        fs::write(root.path.join("minecraftpe/options.txt"), b"old").unwrap();
        let doc = super::super::manage::read(&root, "minecraftpe/options.txt")
            .unwrap();
        super::super::manage::write(
            &root,
            "minecraftpe/options.txt",
            "new",
            &doc.revision,
        )
        .unwrap();
        let imports = fixture.path().join("imports");
        fs::create_dir(&imports).unwrap();
        fs::write(imports.join("fresh"), b"new").unwrap();
        let completed = fs::File::create(imports.join("completed")).unwrap();
        completed
            .set_times(fs::FileTimes::new().set_modified(
                std::time::SystemTime::now()
                    - std::time::Duration::from_secs(90000),
            ))
            .unwrap();
        let summary = summary(&[root], Some(&imports)).unwrap();
        assert!(
            summary
                .entries
                .iter()
                .any(|e| e.category == "recovery" && e.removable)
        );
        assert!(
            summary
                .entries
                .iter()
                .any(|e| e.category == "retained_version"
                    && e.path == "resource_packs/old"
                    && !e.removable)
        );
        assert!(summary.entries.iter().any(|e| e.category == "content"
            && e.path == "resource_packs/active"
            && !e.removable));
        assert!(summary.entries.iter().any(|e| e.category == "import"
            && e.id == "completed"
            && e.removable));
        assert!(summary.entries.iter().any(|e| e.category == "import"
            && e.id == "fresh"
            && !e.removable));
    }
    #[test]
    fn retiring_a_linked_copy_never_follows_the_link_or_touches_originals() {
        let fixture = tempfile::tempdir().unwrap();
        let target = fixture.path().join("outside");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("original"), b"keep").unwrap();
        let selected = fixture.path().join("copy");
        fs::create_dir(&selected).unwrap();
        let link = selected.join("linked");
        #[cfg(windows)]
        {
            let output = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&target)
                .output()
                .unwrap();
            assert!(output.status.success());
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(remove_tree(&selected).is_err());
        assert_eq!(fs::read(target.join("original")).unwrap(), b"keep");
        assert!(link.exists());
        #[cfg(windows)]
        fs::remove_dir(link).unwrap();
        #[cfg(unix)]
        fs::remove_file(link).unwrap();
    }
    #[test]
    fn selected_tree_removal_and_recovery_copy_are_confined() {
        let f = tempfile::tempdir().unwrap();
        let source = f.path().join("source");
        let target = f.path().join("target");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("file"), b"original").unwrap();
        copy(&source, &target).unwrap();
        assert!(source.join("file").exists());
        fs::write(target.join("file"), b"external").unwrap();
        assert!(copy(&source, &target).is_err());
        remove_tree(&target).unwrap();
        assert!(source.join("file").exists());
        assert_eq!(measure(&source), (8, false));
    }
}
