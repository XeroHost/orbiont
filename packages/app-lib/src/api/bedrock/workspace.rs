//! Read-only access to documented Bedrock user data; never to game binaries.
use super::data::*;
use std::{
    fs,
    hash::{Hash, Hasher},
    io::{self, Read, Seek, SeekFrom},
    path::{Component, Path, PathBuf},
    sync::Mutex,
};

pub(super) const MAX_LOG_BYTES: usize = 1024 * 1024;
const MAX_ENTRIES: usize = 1000;
const MAX_METADATA_BYTES: u64 = 512 * 1024;
const MAX_ICON_BYTES: u64 = 8 * 1024 * 1024;

fn item_icon(
    root: &DataRoot,
    relative: &str,
    kind: ItemKind,
) -> Option<PathBuf> {
    let stem = match kind {
        ItemKind::World => "world_icon",
        ItemKind::Log => return None,
        _ => "pack_icon",
    };
    let folder = resolve(root, relative).ok()?;
    let entries = fs::read_dir(folder).ok()?;
    let mut candidates = entries
        .take(MAX_ENTRIES)
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let file = Path::new(&name);
            if !file.file_stem()?.to_str()?.eq_ignore_ascii_case(stem) {
                return None;
            }
            let extension = file.extension()?.to_str()?.to_ascii_lowercase();
            let priority = match extension.as_str() {
                "png" => 0,
                "jpeg" => 1,
                "jpg" => 2,
                "webp" => 3,
                _ => return None,
            };
            Some((priority, name, extension))
        })
        .collect::<Vec<_>>();
    candidates.sort();
    for (_, name, extension) in candidates {
        let Ok(path) = resolve(root, &format!("{relative}/{name}")) else {
            continue;
        };
        let Ok(file) = fs::File::open(&path) else {
            continue;
        };
        let Ok(meta) = file.metadata() else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_ICON_BYTES {
            continue;
        }
        let mut signature = Vec::new();
        if file.take(12).read_to_end(&mut signature).is_err() {
            continue;
        }
        let valid = match extension.as_str() {
            "png" => signature.starts_with(b"\x89PNG\r\n\x1a\n"),
            "jpeg" | "jpg" => signature.starts_with(&[0xff, 0xd8, 0xff]),
            "webp" => {
                signature.starts_with(b"RIFF")
                    && signature.get(8..12) == Some(b"WEBP")
            }
            _ => false,
        };
        if valid {
            return Some(dunce::simplified(&path).to_path_buf());
        }
    }
    None
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "Select a detected Minecraft data location",
    )
}
fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub(super) fn resolve(root: &DataRoot, relative: &str) -> io::Result<PathBuf> {
    // Reject Windows separators/prefixes on every platform, including test hosts.
    if relative.contains('\\')
        || relative.contains(':')
        || relative.contains('\0')
    {
        return Err(invalid());
    }
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(invalid());
    }
    let mut path = root.path.clone();
    let root_meta = fs::symlink_metadata(&path)?;
    if !root_meta.is_dir() || is_link(&root_meta) {
        return Err(invalid());
    }
    let base = fs::canonicalize(&path)?;
    for part in relative.components() {
        path.push(part.as_os_str());
        if is_link(&fs::symlink_metadata(&path)?) {
            return Err(invalid());
        }
    }
    let canonical = fs::canonicalize(path)?;
    if !canonical.starts_with(base) {
        return Err(invalid());
    }
    Ok(canonical)
}

fn add_root(
    roots: &mut Vec<DataRoot>,
    base: &Path,
    relative: &str,
    id: String,
    kind: RootKind,
) -> io::Result<()> {
    let anchor = DataRoot {
        id: String::new(),
        path: base.into(),
        kind,
    };
    match resolve(&anchor, relative) {
        Ok(path) if path.is_dir() => {
            if !roots.iter().any(|root| root.path == path) {
                roots.push(DataRoot {
                    id,
                    path: dunce::simplified(&path).to_path_buf(),
                    kind,
                });
            }
        }
        Ok(_) => {}
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::InvalidInput
            ) => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

pub(super) fn discover(
    roaming: &Path,
    local: &Path,
) -> io::Result<Vec<DataRoot>> {
    let mut roots = Vec::new();
    add_root(
        &mut roots,
        roaming,
        "Minecraft Bedrock/Users/Shared/games/com.mojang",
        "gdk-shared".into(),
        RootKind::Shared,
    )?;
    let anchor = DataRoot {
        id: String::new(),
        path: roaming.into(),
        kind: RootKind::User,
    };
    if let Ok(users) = resolve(&anchor, "Minecraft Bedrock/Users") {
        let mut names: Vec<_> = fs::read_dir(users)?
            .take(MAX_ENTRIES)
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| {
                !name.is_empty()
                    && name.bytes().all(|byte| byte.is_ascii_digit())
            })
            .collect();
        names.sort();
        for name in names {
            add_root(
                &mut roots,
                roaming,
                &format!("Minecraft Bedrock/Users/{name}/games/com.mojang"),
                format!("gdk-user-{name}"),
                RootKind::User,
            )?;
        }
    }
    add_root(
        &mut roots,
        roaming,
        "Minecraft Bedrock/logs",
        "gdk-logs".into(),
        RootKind::Logs,
    )?;
    let legacy = "Packages/Microsoft.MinecraftUWP_8wekyb3d8bbwe/LocalState";
    add_root(
        &mut roots,
        local,
        &format!("{legacy}/games/com.mojang"),
        "uwp-data".into(),
        RootKind::Legacy,
    )?;
    add_root(
        &mut roots,
        local,
        &format!("{legacy}/logs"),
        "uwp-logs".into(),
        RootKind::Logs,
    )?;
    Ok(roots)
}

pub(super) fn list_files(
    root: &DataRoot,
    relative: &str,
) -> io::Result<DirectoryListing> {
    let path = resolve(root, relative)?;
    if !path.is_dir() {
        return Err(invalid());
    }
    let mut result = DirectoryListing {
        entries: Vec::new(),
        limited: false,
    };
    for (index, entry) in fs::read_dir(path)?.enumerate() {
        if index >= MAX_ENTRIES {
            result.limited = true;
            break;
        }
        let Ok(entry) = entry else {
            result.limited = true;
            continue;
        };
        let Ok(metadata) = fs::symlink_metadata(entry.path()) else {
            result.limited = true;
            continue;
        };
        if is_link(&metadata) || (!metadata.is_dir() && !metadata.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let path = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        // Such names cannot be addressed safely through the IPC boundary.
        if name.contains(['\\', ':', '\0']) || name.starts_with(".orbiont-") {
            continue;
        }
        result.entries.push(FileEntry {
            name,
            path,
            is_dir: metadata.is_dir(),
            size: if metadata.is_file() {
                metadata.len()
            } else {
                0
            },
            created: metadata.created().ok().and_then(unix_seconds),
            modified: metadata.modified().ok().and_then(unix_seconds),
            count: if metadata.is_dir() {
                directory_count(&entry.path())
            } else {
                None
            },
        });
    }
    result.entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(result)
}

fn unix_seconds(time: std::time::SystemTime) -> Option<u64> {
    time.duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
}

// Count immediate entries only, without traversing links or unbounded directories.
fn directory_count(path: &Path) -> Option<usize> {
    let mut count = 0;
    for entry in fs::read_dir(path).ok()?.take(MAX_ENTRIES + 1) {
        entry.ok()?;
        count += 1;
        if count > MAX_ENTRIES {
            return None;
        }
    }
    Some(count)
}

fn small_text(root: &DataRoot, relative: &str) -> Option<String> {
    let path = resolve(root, relative).ok()?;
    let file = fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_METADATA_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_METADATA_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_METADATA_BYTES {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn display_text(text: Option<&str>) -> Option<String> {
    text.map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(4096).collect())
}

pub(super) fn snapshot(roots: Vec<DataRoot>) -> io::Result<Workspace> {
    let mut result = Workspace {
        roots,
        items: Vec::new(),
        incomplete: false,
    };
    let categories = [
        ("resource_packs", ItemKind::ResourcePack, false),
        ("skin_packs", ItemKind::SkinPack, false),
        ("behavior_packs", ItemKind::BehaviorPack, false),
        ("development_resource_packs", ItemKind::ResourcePack, true),
        ("development_skin_packs", ItemKind::SkinPack, true),
        ("development_behavior_packs", ItemKind::BehaviorPack, true),
        ("minecraftWorlds", ItemKind::World, false),
    ];
    for root in &result.roots {
        let mut dirs: Vec<_> = if root.kind == RootKind::Logs {
            vec![("", ItemKind::Log, false)]
        } else {
            categories.to_vec()
        };
        // Catalog imports can embed their packs in a world instead of global folders.
        let mut embedded = Vec::new();
        if root.kind != RootKind::Logs
            && let Ok(worlds) = list_files(root, "minecraftWorlds")
        {
            result.incomplete |= worlds.limited;
            for world in worlds.entries.into_iter().filter(|world| world.is_dir)
            {
                for (category, kind) in [
                    ("resource_packs", ItemKind::ResourcePack),
                    ("behavior_packs", ItemKind::BehaviorPack),
                ] {
                    embedded.push((format!("{}/{category}", world.path), kind));
                }
            }
        }
        dirs.extend(
            embedded
                .iter()
                .map(|(path, kind)| (path.as_str(), *kind, false)),
        );
        for (directory, kind, development) in dirs {
            let listing = match list_files(root, directory) {
                Ok(listing) => listing,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    continue;
                }
                Err(_) => {
                    result.incomplete = true;
                    continue;
                }
            };
            result.incomplete |= listing.limited;
            for entry in listing.entries {
                if result.items.len() >= 5000 {
                    result.incomplete = true;
                    break;
                }
                if kind == ItemKind::Log {
                    if entry.is_dir || !is_log_name(&entry.path) {
                        continue;
                    }
                } else if !entry.is_dir {
                    continue;
                }
                let mut name = entry.name;
                let mut version = None;
                let mut description = None;
                let mut pack_id = None;
                let mut pack_version = None;
                let mut dependencies = Vec::new();
                if kind == ItemKind::World {
                    if let Some(text) = small_text(
                        root,
                        &format!("{}/levelname.txt", entry.path),
                    )
                    .and_then(|text| display_text(Some(&text)))
                    {
                        name = text;
                    }
                } else if kind != ItemKind::Log
                    && let Some(manifest) = small_text(
                        root,
                        &format!("{}/manifest.json", entry.path),
                    )
                    .and_then(|text| {
                        serde_json::from_str::<serde_json::Value>(
                            text.trim_start_matches('\u{feff}'),
                        )
                        .ok()
                    })
                {
                    let header = &manifest["header"];
                    pack_id = header["uuid"]
                        .as_str()
                        .and_then(|id| uuid::Uuid::parse_str(id).ok())
                        .map(|id| id.to_string());
                    pack_version = numeric_version(&header["version"]);
                    if let Some(deps) = manifest["dependencies"].as_array() {
                        for dep in deps {
                            if let (Some(id), Some(version)) = (
                                dep["uuid"].as_str().and_then(|id| {
                                    uuid::Uuid::parse_str(id).ok()
                                }),
                                numeric_version(&dep["version"]),
                            ) {
                                dependencies.push(PackDependency {
                                    pack_id: id.to_string(),
                                    version,
                                });
                            }
                        }
                    }
                    if let Some(text) = display_text(header["name"].as_str()) {
                        name = text;
                    }
                    description = display_text(header["description"].as_str());
                    if let Some(parts) = header["version"].as_array() {
                        if parts.len() <= 4
                            && parts.iter().all(|part| part.as_u64().is_some())
                        {
                            version = Some(
                                parts
                                    .iter()
                                    .map(|part| part.to_string())
                                    .collect::<Vec<_>>()
                                    .join("."),
                            );
                        }
                    } else {
                        version = display_text(header["version"].as_str());
                    }
                    // Resolve common manifest localization keys without evaluating pack code.
                    if (name == "pack.name"
                        || description.as_deref() == Some("pack.description"))
                        && let Some(lang) = small_text(
                            root,
                            &format!("{}/texts/en_US.lang", entry.path),
                        )
                    {
                        for line in lang.lines() {
                            if let Some((key, value)) = line.split_once('=') {
                                if key.trim() == "pack.name"
                                    && name == "pack.name"
                                    && let Some(text) =
                                        display_text(Some(value))
                                {
                                    name = text;
                                }
                                if key.trim() == "pack.description"
                                    && description.as_deref()
                                        == Some("pack.description")
                                {
                                    description = display_text(Some(value));
                                }
                            }
                        }
                    }
                }
                result.items.push(WorkspaceItem {
                    icon_path: item_icon(root, &entry.path, kind),
                    root_id: root.id.clone(),
                    path: entry.path,
                    name,
                    kind,
                    version,
                    description,
                    development,
                    pack_id,
                    pack_version,
                    dependencies,
                    activations: Vec::new(),
                    active: false,
                });
            }
        }
    }
    enrich_activations(&mut result);
    result.items.sort_by_key(|item| item.name.to_lowercase());
    Ok(result)
}

pub(super) fn accessible_pack_root(world: &DataRoot, pack: &DataRoot) -> bool {
    world.id == pack.id
        || (pack.kind == RootKind::Shared
            && matches!(world.kind, RootKind::User | RootKind::Shared))
}
fn numeric_version(value: &serde_json::Value) -> Option<Vec<u32>> {
    let parts = value.as_array()?;
    if parts.len() != 3 {
        return None;
    }
    parts
        .iter()
        .map(|part| part.as_u64().and_then(|n| u32::try_from(n).ok()))
        .collect()
}
fn enrich_activations(result: &mut Workspace) {
    let worlds: Vec<_> = result
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::World)
        .map(|i| (i.root_id.clone(), i.path.clone()))
        .collect();
    for (root_id, world_path) in worlds {
        let Some(root) = result.roots.iter().find(|r| r.id == root_id) else {
            continue;
        };
        for (name, kind) in [
            ("world_resource_packs.json", ItemKind::ResourcePack),
            ("world_behavior_packs.json", ItemKind::BehaviorPack),
        ] {
            let Some(text) = small_text(root, &format!("{world_path}/{name}"))
            else {
                continue;
            };
            let Some(values) = serde_json::from_str::<serde_json::Value>(
                text.trim_start_matches('\u{feff}'),
            )
            .ok()
            .and_then(|v| v.as_array().cloned()) else {
                result.incomplete = true;
                continue;
            };
            for activation in values {
                let Some(id) = activation["pack_id"]
                    .as_str()
                    .and_then(|id| uuid::Uuid::parse_str(id).ok())
                    .map(|id| id.to_string())
                else {
                    continue;
                };
                let Some(version) = numeric_version(&activation["version"])
                else {
                    continue;
                };
                let mut matches: Vec<_> = result
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, item)| {
                        item.kind == kind
                            && item.pack_id.as_deref() == Some(&id)
                            && item.pack_version.as_ref() == Some(&version)
                    })
                    .filter(|(_, item)| {
                        result
                            .roots
                            .iter()
                            .find(|r| r.id == item.root_id)
                            .is_some_and(|pack_root| {
                                accessible_pack_root(root, pack_root)
                            })
                    })
                    .filter(|(_, item)| {
                        !item.path.starts_with("minecraftWorlds/")
                            || (item.root_id == root_id
                                && item
                                    .path
                                    .starts_with(&format!("{world_path}/")))
                    })
                    .map(|(index, item)| {
                        (
                            if item.path.starts_with("minecraftWorlds/") {
                                0
                            } else if item.root_id == root_id {
                                1
                            } else {
                                2
                            },
                            index,
                        )
                    })
                    .collect();
                matches.sort();
                // Duplicate exact versions have no documented file-level selection.
                // Expose all possible providers as active rather than labelling them safe to discard.
                if let Some((priority, _)) = matches.first().copied() {
                    for (_, index) in
                        matches.into_iter().take_while(|(p, _)| *p == priority)
                    {
                        let item = &mut result.items[index];
                        item.active = true;
                        item.activations.push(PackActivation {
                            root_id: root_id.clone(),
                            world_path: world_path.clone(),
                            version: version.clone(),
                        });
                    }
                }
            }
        }
    }
}
static CACHE: Mutex<Option<(u64, Workspace)>> = Mutex::new(None);
fn fingerprint(roots: &[DataRoot]) -> Option<u64> {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    let mut visited = 0;
    for root in roots {
        root.id.hash(&mut hash);
        root.path.hash(&mut hash);
        let mut pending = vec![(root.path.clone(), 0)];
        while let Some((path, depth)) = pending.pop() {
            visited += 1;
            if visited > 50000 {
                return None;
            }
            let meta = fs::symlink_metadata(&path).ok()?;
            if is_link(&meta) {
                return None;
            }
            path.hash(&mut hash);
            meta.len().hash(&mut hash);
            meta.modified().ok()?.hash(&mut hash);
            if meta.is_dir() && depth < 7 {
                let mut children = fs::read_dir(path)
                    .ok()?
                    .map(|entry| entry.map(|e| e.path()))
                    .collect::<io::Result<Vec<_>>>()
                    .ok()?;
                children.sort();
                for child in children.into_iter().rev() {
                    let name = child.file_name()?.to_str()?;
                    if name == "db" || name.starts_with(".orbiont-") {
                        continue;
                    }
                    let child_meta = fs::symlink_metadata(&child).ok()?;
                    if child_meta.is_dir()
                        || [
                            "manifest.json",
                            "levelname.txt",
                            "world_resource_packs.json",
                            "world_behavior_packs.json",
                            "en_US.lang",
                        ]
                        .contains(&name)
                        || name.starts_with("pack_icon.")
                        || name.starts_with("world_icon.")
                    {
                        pending.push((child, depth + 1));
                    }
                }
            }
        }
    }
    Some(hash.finish())
}
pub(super) fn cached_snapshot(
    roots: Vec<DataRoot>,
    refresh: bool,
) -> io::Result<Workspace> {
    let stamp = fingerprint(&roots);
    if !refresh
        && let Some(stamp) = stamp
        && let Ok(cache) = CACHE.lock()
        && let Some((saved, workspace)) = cache.as_ref()
        && *saved == stamp
    {
        return Ok(workspace.clone());
    }
    let result = snapshot(roots.clone())?;
    if stamp.is_some()
        && stamp == fingerprint(&roots)
        && let Ok(mut cache) = CACHE.lock()
    {
        *cache = Some((stamp.unwrap(), result.clone()));
    }
    Ok(result)
}

fn is_log_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("log")
                || extension.eq_ignore_ascii_case("txt")
        })
}
pub(super) fn read_log(root: &DataRoot, relative: &str) -> io::Result<LogText> {
    if root.kind != RootKind::Logs || !is_log_name(relative) {
        return Err(invalid());
    }
    let path = resolve(root, relative)?;
    let mut file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid());
    }
    let truncated = metadata.len() > MAX_LOG_BYTES as u64;
    if truncated {
        file.seek(SeekFrom::End(-(MAX_LOG_BYTES as i64)))?;
    }
    let mut bytes = Vec::new();
    file.take(MAX_LOG_BYTES as u64).read_to_end(&mut bytes)?;
    Ok(LogText {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        truncated,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_metadata_refreshes_after_external_edits_and_embedded_resolution()
    {
        let fixture = tempfile::tempdir().unwrap();
        let root = DataRoot {
            id: "cache-fixture".into(),
            path: fixture.path().into(),
            kind: RootKind::User,
        };
        let id = "8d9932da-e0fb-4a38-bbaf-3307f93ca4ee";
        for path in [
            "resource_packs/global",
            "minecraftWorlds/world/resource_packs/local",
        ] {
            fs::create_dir_all(root.path.join(path)).unwrap();
            fs::write(root.path.join(path).join("manifest.json"),format!(r#"{{"header":{{"uuid":"{id}","name":"old","version":[1,0,0]}}}}"#)).unwrap();
        }
        fs::write(
            root.path
                .join("minecraftWorlds/world/world_resource_packs.json"),
            format!(r#"[{{"pack_id":"{id}","version":[1,0,0]}}]"#),
        )
        .unwrap();
        let first = cached_snapshot(vec![root.clone()], false).unwrap();
        assert!(
            !first
                .items
                .iter()
                .find(|i| i.path == "resource_packs/global")
                .unwrap()
                .active
        );
        assert!(
            first
                .items
                .iter()
                .find(|i| i.path.contains("/local"))
                .unwrap()
                .active
        );
        fs::write(root.path.join("resource_packs/global/manifest.json"),format!(r#"{{"header":{{"uuid":"{id}","name":"externally updated","version":[2,0,0]}}}}"#)).unwrap();
        let fresh = cached_snapshot(vec![root.clone()], false).unwrap();
        assert_eq!(
            fresh
                .items
                .iter()
                .find(|i| i.path == "resource_packs/global")
                .unwrap()
                .name,
            "externally updated"
        );
        assert_eq!(
            cached_snapshot(vec![root], true).unwrap().items.len(),
            fresh.items.len()
        );
    }

    use super::*;
    use std::fs;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("orbiont-bedrock-data-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn dir(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        }
        fn file(&self, name: &str, data: &[u8]) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, data).unwrap();
        }
        fn root(&self, kind: RootKind) -> DataRoot {
            DataRoot {
                id: "fixture".into(),
                path: self.0.clone(),
                kind,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn discovers_gdk_shared_user_and_legacy_without_assuming_an_account() {
        let fixture = Fixture::new();
        let roaming = fixture.dir("roaming");
        let local = fixture.dir("local");
        fixture.dir("roaming/Minecraft Bedrock/Users/Shared/games/com.mojang");
        fixture.dir("roaming/Minecraft Bedrock/Users/123456/games/com.mojang");
        fixture
            .dir("roaming/Minecraft Bedrock/Users/unrelated/games/com.mojang");
        fixture.dir("roaming/Minecraft Bedrock/logs");
        fixture.dir("local/Packages/Microsoft.MinecraftUWP_8wekyb3d8bbwe/LocalState/games/com.mojang");
        let roots = discover(&roaming, &local).unwrap();
        assert_eq!(roots.len(), 4);
        assert_eq!(
            roots
                .iter()
                .filter(|root| root.kind == RootKind::Shared)
                .count(),
            1
        );
        assert_eq!(
            roots
                .iter()
                .filter(|root| root.kind == RootKind::User)
                .count(),
            1
        );
        assert!(
            roots
                .iter()
                .all(|root| !root.path.to_string_lossy().contains("unrelated"))
        );
        assert!(
            discover(
                &fixture.dir("empty-roaming"),
                &fixture.dir("empty-local")
            )
            .unwrap()
            .is_empty()
        );
    }

    #[test]
    fn lists_real_packs_world_names_and_development_content_without_writing() {
        let fixture = Fixture::new();
        let manifest = br#"{"header":{"name":"A pack","description":"Local content","version":[1,2,3]}}"#;
        fixture.file("resource_packs/resource/manifest.json", manifest);
        fixture.file(
            "development_behavior_packs/behavior/manifest.json",
            manifest,
        );
        fixture.file("skin_packs/invalid/manifest.json", b"bad json");
        fixture.file(
            "minecraftWorlds/world/levelname.txt",
            "Mundo español\n".as_bytes(),
        );
        fixture.file("minecraftWorlds/world/level.dat", b"unchanged");
        let result = snapshot(vec![fixture.root(RootKind::Shared)]).unwrap();
        assert_eq!(result.items.len(), 4);
        assert!(!result.incomplete);
        let pack = result
            .items
            .iter()
            .find(|item| item.kind == ItemKind::ResourcePack)
            .unwrap();
        assert_eq!(pack.name, "A pack");
        assert_eq!(pack.version.as_deref(), Some("1.2.3"));
        assert!(result.items.iter().any(|item| item.kind
            == ItemKind::BehaviorPack
            && item.development));
        assert!(
            result.items.iter().any(|item| item.kind == ItemKind::World
                && item.name == "Mundo español")
        );
        assert_eq!(
            fs::read(fixture.0.join("minecraftWorlds/world/level.dat"))
                .unwrap(),
            b"unchanged"
        );
    }

    #[test]
    fn local_icons_cover_global_packs_worlds_and_embedded_packs() {
        let fixture = Fixture::new();
        let png = b"\x89PNG\r\n\x1a\n";
        fixture.file("resource_packs/global/pack_icon.png", png);
        fixture
            .file("minecraftWorlds/world/world_icon.jpeg", &[0xff, 0xd8, 0xff]);
        fixture.file(
            "minecraftWorlds/world/resource_packs/local/PACK_ICON.PNG",
            png,
        );
        fixture.dir("behavior_packs/no-icon");
        let result = snapshot(vec![fixture.root(RootKind::Shared)]).unwrap();
        for (folder, file) in [
            ("resource_packs/global", "pack_icon.png"),
            ("minecraftWorlds/world", "world_icon.jpeg"),
            (
                "minecraftWorlds/world/resource_packs/local",
                "PACK_ICON.PNG",
            ),
        ] {
            let item = result
                .items
                .iter()
                .find(|item| item.path == folder)
                .unwrap();
            let expected =
                fs::canonicalize(fixture.0.join(folder).join(file)).unwrap();
            assert_eq!(
                item.icon_path.as_deref(),
                Some(dunce::simplified(&expected))
            );
        }
        assert!(
            result
                .items
                .iter()
                .find(|item| item.path == "behavior_packs/no-icon")
                .unwrap()
                .icon_path
                .is_none()
        );
    }

    #[test]
    fn local_icons_reject_invalid_oversized_and_outside_files() {
        let fixture = Fixture::new();
        fixture.file("minecraftWorlds/world/world_icon.png", b"not an image");
        fixture
            .file("minecraftWorlds/world/world_icon.jpg", &[0xff, 0xd8, 0xff]);
        fixture
            .file("resource_packs/large/pack_icon.png", b"\x89PNG\r\n\x1a\n");
        fs::OpenOptions::new()
            .write(true)
            .open(fixture.0.join("resource_packs/large/pack_icon.png"))
            .unwrap()
            .set_len(MAX_ICON_BYTES + 1)
            .unwrap();
        let root = fixture.root(RootKind::User);
        assert!(
            item_icon(&root, "resource_packs/large", ItemKind::ResourcePack)
                .is_none()
        );
        assert!(item_icon(&root, "../outside", ItemKind::World).is_none());
        assert!(
            item_icon(&root, "minecraftWorlds/world", ItemKind::World)
                .unwrap()
                .ends_with("world_icon.jpg")
        );
    }

    #[test]
    fn confines_browsing_to_detected_root_and_never_resolves_parent_or_absolute_paths()
     {
        let fixture = Fixture::new();
        fixture.file("folder/a.json", b"{}");
        let root = fixture.root(RootKind::User);
        for path in [
            "../outside",
            "folder/../../outside",
            "/absolute",
            "C:\\Windows",
            "folder\\..\\outside",
        ] {
            assert!(resolve(&root, path).is_err(), "accepted {path}");
        }
        assert!(resolve(&root, fixture.0.to_str().unwrap()).is_err());
        let listing = list_files(&root, "folder").unwrap();
        assert_eq!(listing.entries.len(), 1);
        assert_eq!(listing.entries[0].path, "folder/a.json");
        assert!(!listing.entries[0].is_dir);
        assert_eq!(listing.entries[0].size, 2);
    }

    #[test]
    fn file_browser_reports_metadata_and_bounded_folder_counts() {
        let fixture = Fixture::new();
        fixture.file("folder/a.json", b"{}");
        fixture.file("folder/b.txt", b"hello");
        fixture.dir("empty");
        fixture.file("plain.txt", b"data");
        let root = fixture.root(RootKind::User);
        let listing = list_files(&root, "").unwrap();
        let folder = listing
            .entries
            .iter()
            .find(|entry| entry.name == "folder")
            .unwrap();
        assert_eq!(folder.count, Some(2));
        let empty = listing
            .entries
            .iter()
            .find(|entry| entry.name == "empty")
            .unwrap();
        assert_eq!(empty.count, Some(0));
        let file = listing
            .entries
            .iter()
            .find(|entry| entry.name == "plain.txt")
            .unwrap();
        assert_eq!(file.size, 4);
        assert_eq!(file.count, None);
        assert!(file.modified.is_some_and(|seconds| seconds > 0));
        for index in 0..=MAX_ENTRIES {
            fixture.file(&format!("large/{index}"), b"");
        }
        assert_eq!(directory_count(&fixture.0.join("large")), None);
        assert_eq!(fs::read(fixture.0.join("plain.txt")).unwrap(), b"data");
    }

    #[test]
    fn reads_only_text_logs_and_bounds_the_tail() {
        let fixture = Fixture::new();
        let mut content = vec![b'x'; MAX_LOG_BYTES + 100];
        content.extend_from_slice(b"\nlast line\n");
        fixture.file("content.log", &content);
        fixture.file("token.dat", b"private");
        let root = fixture.root(RootKind::Logs);
        let log = read_log(&root, "content.log").unwrap();
        assert!(log.truncated);
        assert!(log.text.ends_with("last line\n"));
        assert!(log.text.len() <= MAX_LOG_BYTES);
        assert!(read_log(&root, "token.dat").is_err());
        assert!(
            read_log(&fixture.root(RootKind::User), "content.log").is_err()
        );
    }

    #[test]
    fn refuses_directory_links_even_when_the_target_is_inside_the_root() {
        let fixture = Fixture::new();
        let target = fixture.dir("target");
        let link = fixture.0.join("linked");
        #[cfg(windows)]
        {
            let output = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&target)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let root = fixture.root(RootKind::User);
        assert!(resolve(&root, "linked").is_err());
        assert!(
            list_files(&root, "")
                .unwrap()
                .entries
                .iter()
                .all(|item| item.name != "linked")
        );
    }
}
