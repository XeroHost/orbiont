//! Import downloaded Java worlds without overwriting saves implicitly.
use quartz_nbt::{NbtCompound, NbtTag};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File, Metadata};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

const MAX_FILES: usize = 100_000;
const MAX_UNPACKED: u64 = 8 * 1024 * 1024 * 1024;
const MAX_LEVEL: u64 = 16 * 1024 * 1024;
const SOURCE_FILE: &str = ".orbiont-world.json";

fn invalid(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

fn is_link(metadata: &Metadata) -> bool {
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

fn safe_component(name: &str) -> bool {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        && !(stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

fn folder_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(100)
        .collect();
    let name = name.trim().trim_end_matches('.');
    if safe_component(name) {
        name.to_string()
    } else {
        "world".into()
    }
}

fn read_level(bytes: &[u8]) -> crate::Result<NbtCompound> {
    let mut unpacked = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_LEVEL + 1)
        .read_to_end(&mut unpacked)?;
    if unpacked.len() as u64 > MAX_LEVEL {
        return Err(invalid("World level.dat is too large"));
    }
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(unpacked),
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    root.get::<_, &NbtCompound>("Data")?;
    Ok(root)
}

fn level_name(root: &NbtCompound) -> String {
    root.get::<_, &NbtCompound>("Data")
        .ok()
        .and_then(|data| data.get::<_, &str>("LevelName").ok())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("world")
        .to_string()
}

struct ArchiveWorld {
    zip: zip::ZipArchive<File>,
    root: PathBuf,
    folder: String,
    name: String,
    level: NbtCompound,
}

fn open_archive(path: &Path) -> crate::Result<ArchiveWorld> {
    let mut zip = zip::ZipArchive::new(File::open(path)?)
        .map_err(|error| invalid(format!("Invalid world ZIP: {error}")))?;
    if zip.len() > MAX_FILES {
        return Err(invalid("World archive has too many entries"));
    }
    let mut roots = vec![];
    let mut size = 0_u64;
    let mut names = HashSet::new();
    for index in 0..zip.len() {
        let file = zip
            .by_index(index)
            .map_err(|error| invalid(error.to_string()))?;
        let name = file.name().trim_end_matches('/');
        if file.is_symlink()
            || name.split('/').any(|part| !safe_component(part))
            || file.enclosed_name().is_none()
            || !names.insert(name.to_lowercase())
        {
            return Err(invalid("Unsafe or duplicate path in world ZIP"));
        }
        size = size
            .checked_add(file.size())
            .ok_or_else(|| invalid("World archive is too large"))?;
        if size > MAX_UNPACKED {
            return Err(invalid("World archive is too large"));
        }
        if !file.is_dir()
            && Path::new(name)
                .file_name()
                .is_some_and(|part| part == "level.dat")
        {
            roots.push((
                index,
                Path::new(name)
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_path_buf(),
            ));
        }
    }
    if roots.len() != 1 {
        return Err(invalid(
            "Select an archive containing exactly one Java world with level.dat",
        ));
    }
    let (index, root) = roots.pop().unwrap();
    let mut bytes = Vec::new();
    zip.by_index(index)
        .map_err(|error| invalid(error.to_string()))?
        .take(MAX_LEVEL + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LEVEL {
        return Err(invalid("World level.dat is too large"));
    }
    let level = read_level(&bytes)?;
    let name = level_name(&level);
    let folder = folder_name(
        root.file_name()
            .and_then(|part| part.to_str())
            .unwrap_or(&name),
    );
    Ok(ArchiveWorld {
        zip,
        root,
        folder,
        name,
        level,
    })
}

fn saves_path(instance: &Path) -> crate::Result<PathBuf> {
    let saves = instance.join("saves");
    if let Ok(metadata) = fs::symlink_metadata(&saves)
        && (is_link(&metadata) || !metadata.is_dir())
    {
        return Err(invalid("The saves folder must be a regular directory"));
    }
    Ok(saves)
}

fn fingerprint(world: &Path) -> crate::Result<String> {
    let metadata = fs::symlink_metadata(world)?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err(invalid("Cannot replace a linked world or file"));
    }
    let level = world.join("level.dat");
    match fs::symlink_metadata(&level) {
        Ok(metadata)
            if !is_link(&metadata)
                && metadata.is_file()
                && metadata.len() <= MAX_LEVEL =>
        {
            let mut digest = sha1_smol::Sha1::from(fs::read(level)?);
            let mut pending = vec![world.to_path_buf()];
            let mut files = vec![];
            while let Some(directory) = pending.pop() {
                for entry in fs::read_dir(directory)? {
                    let entry = entry?;
                    let path = entry.path();
                    if entry.file_name() == "session.lock" {
                        continue;
                    }
                    let data = fs::symlink_metadata(&path)?;
                    if is_link(&data) {
                        return Err(invalid(
                            "Cannot replace a world containing linked files",
                        ));
                    }
                    if data.is_dir() {
                        pending.push(path);
                    } else {
                        files.push(format!(
                            "{}:{}:{:?}",
                            path.strip_prefix(world).unwrap().display(),
                            data.len(),
                            data.modified()?
                        ));
                        if files.len() > MAX_FILES {
                            return Err(invalid("World has too many files"));
                        }
                    }
                }
            }
            files.sort();
            for file in files {
                digest.update(file.as_bytes());
            }
            Ok(digest.digest().to_string())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok("missing-level".into())
        }
        _ => Err(invalid("Cannot replace an invalid or linked level.dat")),
    }
}

fn preview_for(
    instance: &Path,
    world: &ArchiveWorld,
    project_id: &str,
) -> crate::Result<WorldInstallPreview> {
    let saves = saves_path(instance)?;
    let mut conflicts = vec![];
    if saves.exists() {
        for entry in fs::read_dir(&saves)? {
            let entry = entry?;
            let path = entry.path();
            let folder = entry.file_name().to_string_lossy().into_owned();
            let metadata = fs::symlink_metadata(&path)?;
            if is_link(&metadata) || !metadata.is_dir() {
                if folder.eq_ignore_ascii_case(&world.folder) {
                    return Err(invalid(
                        "The destination is a linked folder or file",
                    ));
                }
                continue;
            }
            let token = match fingerprint(&path) {
                Ok(token) => token,
                Err(error) if folder.eq_ignore_ascii_case(&world.folder) => {
                    return Err(error);
                }
                Err(_) => continue,
            };
            let name = if token == "missing-level" {
                folder.clone()
            } else {
                fs::read(path.join("level.dat"))
                    .ok()
                    .and_then(|bytes| read_level(&bytes).ok())
                    .map(|data| level_name(&data))
                    .unwrap_or_else(|| folder.clone())
            };
            let marker_path = path.join(SOURCE_FILE);
            let same_project = fs::symlink_metadata(&marker_path)
                .ok()
                .filter(|data| !is_link(data) && data.len() < 8192)
                .and_then(|_| fs::read(marker_path).ok())
                .and_then(|bytes| {
                    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                })
                .is_some_and(|data| {
                    data["project_id"].as_str() == Some(project_id)
                });
            if folder.eq_ignore_ascii_case(&world.folder)
                || name.trim().to_lowercase()
                    == world.name.trim().to_lowercase()
                || same_project
            {
                conflicts.push(WorldInstallConflict {
                    path: folder,
                    name,
                    fingerprint: token,
                });
            }
        }
    }
    conflicts.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(WorldInstallPreview {
        folder: world.folder.clone(),
        name: world.name.clone(),
        conflicts,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldInstallConflict {
    pub path: String,
    pub name: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorldInstallPreview {
    pub folder: String,
    pub name: String,
    pub conflicts: Vec<WorldInstallConflict>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldInstallAction {
    Create,
    Replace,
    Copy,
    Keep,
    Cancel,
}

#[derive(Debug, Serialize)]
pub struct WorldInstallResult {
    pub path: Option<String>,
    pub backup: Option<String>,
    pub preview: Option<WorldInstallPreview>,
}

pub fn inspect_archive(
    instance: &Path,
    archive: &Path,
    project_id: &str,
) -> crate::Result<WorldInstallPreview> {
    preview_for(instance, &open_archive(archive)?, project_id)
}

pub fn import_archive(
    instance: &Path,
    archive: &Path,
    project_id: &str,
    action: WorldInstallAction,
    conflict: Option<WorldInstallConflict>,
) -> crate::Result<WorldInstallResult> {
    let mut result = WorldInstallResult {
        path: None,
        backup: None,
        preview: None,
    };
    if matches!(
        action,
        WorldInstallAction::Keep | WorldInstallAction::Cancel
    ) {
        return Ok(result);
    }
    let mut world = open_archive(archive)?;
    let import_lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(instance.join(".orbiont-world-install.lock"))?;
    import_lock.try_lock().map_err(|error| {
        invalid(format!(
            "Another world installation is in progress: {error}"
        ))
    })?;
    let preview = preview_for(instance, &world, project_id)?;
    let replacement = if matches!(action, WorldInstallAction::Replace) {
        let selected = conflict.as_ref().filter(|selection| {
            preview.conflicts.iter().any(|item| {
                item.path == selection.path
                    && item.fingerprint == selection.fingerprint
            })
        });
        if let Some(selected) = selected {
            Some(selected.path.clone())
        } else {
            result.preview = Some(preview);
            return Ok(result);
        }
    } else {
        None
    };
    if matches!(action, WorldInstallAction::Create)
        && !preview.conflicts.is_empty()
    {
        result.preview = Some(preview);
        return Ok(result);
    }
    let saves = saves_path(instance)?;
    fs::create_dir_all(&saves)?;
    let occupied = fs::read_dir(&saves)?
        .map(|item| {
            item.map(|item| item.file_name().to_string_lossy().to_lowercase())
        })
        .collect::<std::io::Result<HashSet<_>>>()?;
    let mut existing_names = HashSet::new();
    for entry in fs::read_dir(&saves)? {
        let entry = entry?;
        if is_link(&fs::symlink_metadata(entry.path())?) {
            continue;
        }
        let path = entry.path().join("level.dat");
        if let Ok(data) = fs::symlink_metadata(&path)
            && !is_link(&data)
            && data.is_file()
            && data.len() <= MAX_LEVEL
            && let Ok(level) = fs::read(path).and_then(|bytes| {
                read_level(&bytes).map_err(std::io::Error::other)
            })
        {
            existing_names.insert(level_name(&level).to_lowercase());
        }
    }
    let mut folder =
        replacement.clone().unwrap_or_else(|| world.folder.clone());
    if replacement.is_none()
        && occupied.contains(&folder.to_lowercase())
        && !matches!(action, WorldInstallAction::Copy)
    {
        result.preview = Some(preview_for(instance, &world, project_id)?);
        return Ok(result);
    }
    let mut counter = 1;
    if matches!(action, WorldInstallAction::Copy) {
        counter = 2;
        loop {
            folder = format!("{} {counter}", world.folder);
            let display = format!("{} {counter}", world.name);
            if !occupied.contains(&folder.to_lowercase())
                && !existing_names.contains(&display.to_lowercase())
            {
                break;
            }
            counter += 1;
        }
    }
    let temp = tempfile::Builder::new()
        .prefix(".world-install-")
        .tempdir_in(instance)?;
    let staged = temp.path().join("world");
    fs::create_dir(&staged)?;
    for index in 0..world.zip.len() {
        let file = world
            .zip
            .by_index(index)
            .map_err(|error| invalid(error.to_string()))?;
        let path = Path::new(file.name());
        let Ok(relative) = path.strip_prefix(&world.root) else {
            continue;
        };
        if relative.as_os_str().is_empty()
            || relative.file_name().is_some_and(|name| {
                name == "session.lock" || name == SOURCE_FILE
            })
        {
            continue;
        }
        let target = staged.join(relative);
        if file.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            fs::create_dir_all(target.parent().unwrap())?;
            let mut output =
                File::options().write(true).create_new(true).open(target)?;
            let expected_size = file.size();
            let actual_size =
                std::io::copy(&mut file.take(expected_size + 1), &mut output)?;
            if actual_size != expected_size {
                return Err(invalid("World ZIP entry size mismatch"));
            }
        }
    }
    if counter > 1 {
        world.level.get_mut::<_, &mut NbtCompound>("Data")?.insert(
            "LevelName",
            NbtTag::String(format!("{} {counter}", world.name)),
        );
        let mut bytes = vec![];
        quartz_nbt::io::write_nbt(
            &mut bytes,
            None,
            &world.level,
            quartz_nbt::io::Flavor::GzCompressed,
        )?;
        fs::write(staged.join("level.dat"), bytes)?;
    }
    fs::write(
        staged.join(SOURCE_FILE),
        serde_json::to_vec(&serde_json::json!({ "project_id": project_id }))?,
    )?;
    let target = saves.join(&folder);
    let mut session_lock = None;
    let mut backup = None;
    if let Some(replacement) = replacement {
        if !safe_component(&replacement)
            || fingerprint(&target)? != conflict.as_ref().unwrap().fingerprint
        {
            result.preview = Some(preview_for(instance, &world, project_id)?);
            return Ok(result);
        }
        reject_links_in_tree(&target)?;
        let lock_path = target.join("session.lock");
        if let Ok(data) = fs::symlink_metadata(&lock_path)
            && is_link(&data)
        {
            return Err(invalid("Invalid world session lock"));
        }
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)?;
        lock.try_lock().map_err(|error| {
            invalid(format!(
                "Close Minecraft before replacing this world: {error}"
            ))
        })?;
        session_lock = Some(lock);
        let backups = instance.join("backups");
        if let Ok(data) = fs::symlink_metadata(&backups)
            && (is_link(&data) || !data.is_dir())
        {
            return Err(invalid("Invalid backup folder"));
        }
        fs::create_dir_all(&backups)?;
        let backup_relative = format!(
            "backups/{}-{}-{}",
            chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"),
            folder,
            uuid::Uuid::new_v4()
        );
        let backup_path = instance.join(&backup_relative);
        // Windows cannot rename a directory containing an open locked file.
        // Keep it locked through validation/staging; release immediately before
        // the synchronous move. A failed move leaves the old save untouched.
        drop(session_lock.take());
        fs::rename(&target, &backup_path)?;
        backup = Some(backup_path);
        result.backup = Some(backup_relative);
    }
    if let Err(error) = fs::rename(&staged, &target) {
        if let Some(backup) = backup {
            fs::rename(&backup, &target).map_err(|rollback| invalid(format!("Install failed ({error}); restore the preserved world from {} ({rollback})", backup.display())))?;
        }
        return Err(error.into());
    }
    drop(session_lock);
    result.path = Some(folder);
    Ok(result)
}

fn reject_links_in_tree(path: &Path) -> crate::Result<()> {
    let mut pending = vec![path.to_path_buf()];
    while let Some(path) = pending.pop() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let data = fs::symlink_metadata(entry.path())?;
            if is_link(&data) {
                return Err(invalid(
                    "Cannot replace a world containing linked files",
                ));
            }
            if data.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quartz_nbt::{NbtCompound, NbtTag};
    use std::io::Write;

    fn level(name: &str) -> Vec<u8> {
        let mut data = NbtCompound::new();
        data.insert("LevelName", NbtTag::String(name.into()));
        let mut root = NbtCompound::new();
        root.insert("Data", NbtTag::Compound(data));
        let mut bytes = vec![];
        quartz_nbt::io::write_nbt(
            &mut bytes,
            None,
            &root,
            quartz_nbt::io::Flavor::GzCompressed,
        )
        .unwrap();
        bytes
    }

    fn archive(dir: &Path, entries: &[(&str, &[u8])]) -> std::path::PathBuf {
        let path = dir.join("download.zip");
        let mut writer =
            zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("saves")).unwrap();
        dir
    }

    #[test]
    fn installs_nested_world_without_wrapper_or_unrelated_files() {
        let dir = fixture();
        let zip = archive(
            dir.path(),
            &[
                ("Release/world/level.dat", &level("Adventure")),
                ("Release/world/region/r.0.0.mca", b"region"),
                ("Readme.txt", b"readme"),
            ],
        );
        let result = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        assert_eq!(result.path.as_deref(), Some("world"));
        assert_eq!(
            std::fs::read(dir.path().join("saves/world/region/r.0.0.mca"))
                .unwrap(),
            b"region"
        );
        assert!(!dir.path().join("saves/Readme.txt").exists());
    }

    #[test]
    fn same_name_or_project_prompts_and_copy_keeps_progress_with_numbered_names()
     {
        let dir = fixture();
        let zip =
            archive(dir.path(), &[("world/level.dat", &level("Adventure"))]);
        import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        std::fs::write(dir.path().join("saves/world/progress"), b"my progress")
            .unwrap();
        let blocked = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        assert!(blocked.path.is_none());
        assert_eq!(blocked.preview.unwrap().conflicts[0].path, "world");
        for expected in ["world 2", "world 3"] {
            let result = import_archive(
                dir.path(),
                &zip,
                "cf-123",
                WorldInstallAction::Copy,
                None,
            )
            .unwrap();
            assert_eq!(result.path.as_deref(), Some(expected));
        }
        assert_eq!(
            std::fs::read(dir.path().join("saves/world/progress")).unwrap(),
            b"my progress"
        );
        let zip = archive(
            dir.path(),
            &[("renamed/level.dat", &level("Other title"))],
        );
        assert_eq!(
            inspect_archive(dir.path(), &zip, "cf-123")
                .unwrap()
                .conflicts
                .len(),
            3
        );
    }

    #[test]
    fn display_name_collision_is_detected_even_in_another_folder() {
        let dir = fixture();
        std::fs::create_dir(dir.path().join("saves/Existing")).unwrap();
        std::fs::write(
            dir.path().join("saves/Existing/level.dat"),
            level("Adventure"),
        )
        .unwrap();
        let zip =
            archive(dir.path(), &[("world/level.dat", &level("adventure"))]);
        let preview = inspect_archive(dir.path(), &zip, "cf-123").unwrap();
        assert_eq!(preview.conflicts[0].path, "Existing");
    }

    #[test]
    fn keep_and_cancel_do_not_touch_existing_world_or_create_backup() {
        let dir = fixture();
        let zip =
            archive(dir.path(), &[("world/level.dat", &level("Adventure"))]);
        import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        for action in [WorldInstallAction::Keep, WorldInstallAction::Cancel] {
            assert!(
                import_archive(dir.path(), &zip, "cf-123", action, None)
                    .unwrap()
                    .path
                    .is_none()
            );
        }
        assert_eq!(
            std::fs::read_dir(dir.path().join("saves")).unwrap().count(),
            1
        );
        assert!(!dir.path().join("backups").exists());
    }

    #[test]
    fn root_archive_and_copy_names_respect_other_world_display_names() {
        let dir = fixture();
        let zip = archive(dir.path(), &[("level.dat", &level("Adventure"))]);
        let result = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        assert_eq!(result.path.as_deref(), Some("Adventure"));
        std::fs::create_dir(dir.path().join("saves/Other")).unwrap();
        std::fs::write(
            dir.path().join("saves/Other/level.dat"),
            level("Adventure 2"),
        )
        .unwrap();
        let copy = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Copy,
            None,
        )
        .unwrap();
        assert_eq!(copy.path.as_deref(), Some("Adventure 3"));
        assert_eq!(
            level_name(
                &read_level(
                    &std::fs::read(
                        dir.path().join("saves/Adventure 3/level.dat")
                    )
                    .unwrap()
                )
                .unwrap()
            ),
            "Adventure 3"
        );
    }

    #[test]
    fn replacement_refuses_a_world_locked_by_minecraft() {
        let dir = fixture();
        let zip =
            archive(dir.path(), &[("world/level.dat", &level("Adventure"))]);
        import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        let conflict = inspect_archive(dir.path(), &zip, "cf-123")
            .unwrap()
            .conflicts[0]
            .clone();
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.path().join("saves/world/session.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert!(
            import_archive(
                dir.path(),
                &zip,
                "cf-123",
                WorldInstallAction::Replace,
                Some(conflict)
            )
            .is_err()
        );
        assert!(dir.path().join("saves/world/level.dat").exists());
    }

    #[test]
    fn region_progress_changes_require_a_new_confirmation() {
        let dir = fixture();
        let zip = archive(
            dir.path(),
            &[
                ("world/level.dat", &level("Adventure")),
                ("world/region/r.0.0.mca", b"original"),
            ],
        );
        import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        let conflict = inspect_archive(dir.path(), &zip, "cf-123")
            .unwrap()
            .conflicts[0]
            .clone();
        std::fs::write(
            dir.path().join("saves/world/region/r.0.0.mca"),
            b"new player progress",
        )
        .unwrap();
        let result = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Replace,
            Some(conflict),
        )
        .unwrap();
        assert!(result.path.is_none());
        assert!(result.preview.is_some());
        assert_eq!(
            std::fs::read(dir.path().join("saves/world/region/r.0.0.mca"))
                .unwrap(),
            b"new player progress"
        );
    }

    #[test]
    fn replacement_keeps_recoverable_backup_and_refuses_stale_confirmation() {
        let dir = fixture();
        let zip =
            archive(dir.path(), &[("world/level.dat", &level("Adventure"))]);
        import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Create,
            None,
        )
        .unwrap();
        std::fs::write(dir.path().join("saves/world/progress"), b"keep me")
            .unwrap();
        let preview = inspect_archive(dir.path(), &zip, "cf-123").unwrap();
        std::fs::write(
            dir.path().join("saves/world/level.dat"),
            level("Changed"),
        )
        .unwrap();
        let result = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Replace,
            Some(preview.conflicts[0].clone()),
        )
        .unwrap();
        assert!(result.path.is_none());
        assert!(dir.path().join("saves/world/progress").exists());
        let conflict = result.preview.unwrap().conflicts[0].clone();
        let result = import_archive(
            dir.path(),
            &zip,
            "cf-123",
            WorldInstallAction::Replace,
            Some(conflict),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(
                dir.path().join(result.backup.unwrap()).join("progress")
            )
            .unwrap(),
            b"keep me"
        );
        assert!(!dir.path().join("saves/world/progress").exists());
    }

    #[test]
    fn rejects_traversal_invalid_nbt_and_multiworld_archives_without_changing_saves()
     {
        let dir = fixture();
        for entries in [
            vec![
                ("world/level.dat", level("World")),
                ("../escape", b"bad".to_vec()),
            ],
            vec![("world/level.dat", b"not nbt".to_vec())],
            vec![("a/level.dat", level("A")), ("b/level.dat", level("B"))],
        ] {
            let borrowed = entries
                .iter()
                .map(|(name, bytes)| (*name, bytes.as_slice()))
                .collect::<Vec<_>>();
            let zip = archive(dir.path(), &borrowed);
            assert!(
                import_archive(
                    dir.path(),
                    &zip,
                    "cf-123",
                    WorldInstallAction::Create,
                    None
                )
                .is_err()
            );
            assert_eq!(
                std::fs::read_dir(dir.path().join("saves")).unwrap().count(),
                0
            );
        }
        assert!(!dir.path().parent().unwrap().join("escape").exists());
    }
}
