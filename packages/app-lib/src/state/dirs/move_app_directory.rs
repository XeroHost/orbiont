use crate::state::content_store;
mod managed_content;

use crate::state::content_store::{
    hash_file, input, normalize, relative_link, sync_directory,
};
use crate::state::instances::adapters::sqlite::instance_rows;
use crate::state::{Instance, JavaVersion};
use crate::util::content_hash::copy_and_hash;
use futures::stream::{FuturesUnordered, StreamExt};
use serde_json::Value;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;
use tokio::io::AsyncWriteExt;

/// Files copied at the same time while moving the app directory.
const COPY_CONCURRENCY: usize = 8;
/// Share of the move progress used by plain file copies; managed content follows.
const COPY_PROGRESS_SHARE: f64 = 0.95;

const MOVED_APP_DIRECTORIES: [&str; 6] = [
    "store",
    "profiles",
    "meta",
    "caches",
    "icons",
    "synced-options",
];
pub(crate) async fn relocate_tree(from: &Path, to: &Path) -> crate::Result<()> {
    if !fs::try_exists(from).await? {
        return Ok(());
    }
    if fs::symlink_metadata(from).await?.file_type().is_symlink() {
        return Err(input(
            "A shared-data root must not be a symlink during migration",
        ));
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).await?;
    }
    if !fs::try_exists(to).await?
        && crate::util::io::is_same_disk(from, to.parent().unwrap_or(to))
            .unwrap_or(false)
    {
        fs::rename(from, to).await?;
        if let Some(parent) = to.parent() {
            sync_directory(parent).await?;
        }
        return Ok(());
    }
    copy_tree(
        from,
        to,
        &[(from.to_path_buf(), to.to_path_buf())],
        &std::collections::HashSet::new(),
        &|_| {},
    )
    .await
}

async fn copy_tree(
    from: &Path,
    to: &Path,
    mappings: &[(PathBuf, PathBuf)],
    managed_paths: &std::collections::HashSet<PathBuf>,
    on_copied: &(dyn Fn(u64) + Send + Sync),
) -> crate::Result<()> {
    let mut copies = FuturesUnordered::new();
    let mut pending = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((source, target)) = pending.pop() {
        if managed_paths.contains(&source) {
            continue;
        }
        let metadata = fs::symlink_metadata(&source).await?;
        if metadata.file_type().is_symlink() {
            let link = fs::read_link(&source).await?;
            let resolved = normalize(
                &source
                    .parent()
                    .ok_or_else(|| input("Invalid symlink"))?
                    .join(&link),
            );
            let mapped = remap(&resolved, mappings);
            let link = relative_link(
                &mapped,
                target
                    .parent()
                    .ok_or_else(|| input("Invalid symlink destination"))?,
            );
            if let Ok(existing) = fs::symlink_metadata(&target).await {
                if existing.file_type().is_symlink()
                    && fs::read_link(&target).await? == link
                {
                    continue;
                }
                if existing.is_file()
                    && !existing.file_type().is_symlink()
                    && hash_file(&resolved).await? == hash_file(&target).await?
                {
                    continue;
                }
                return Err(input(format!(
                    "Migration destination conflicts with {}",
                    target.display()
                )));
            }
            create_link(&link, &target, &resolved).await?;
        } else if metadata.is_dir() {
            if let Ok(existing) = fs::symlink_metadata(&target).await
                && (!existing.is_dir() || existing.file_type().is_symlink())
            {
                return Err(input(
                    "Migration destination contains a directory link or conflicting file",
                ));
            }
            fs::create_dir_all(&target).await?;
            let mut entries = fs::read_dir(&source).await?;
            while let Some(entry) = entries.next_entry().await? {
                pending.push((entry.path(), target.join(entry.file_name())));
            }
        } else if metadata.is_file() {
            if let Ok(existing) = fs::symlink_metadata(&target).await {
                if !existing.is_file()
                    || existing.file_type().is_symlink()
                    || hash_file(&source).await? != hash_file(&target).await?
                {
                    return Err(input(format!(
                        "Migration would overwrite different data at {}",
                        target.display()
                    )));
                }
                continue;
            }
            // Per-file sync latency dominates many small files, so overlap a few copies.
            if copies.len() >= COPY_CONCURRENCY
                && let Some(result) = copies.next().await
            {
                on_copied(result?);
            }
            copies.push(copy_file(source, target, metadata));
        }
    }
    while let Some(result) = copies.next().await {
        on_copied(result?);
    }
    Ok(())
}

/// Copies one file through a temporary sibling and returns its size. The source is read once
/// to copy and hash it; the copy is then re-read and must match before it is moved into place.
async fn copy_file(
    source: PathBuf,
    target: PathBuf,
    metadata: std::fs::Metadata,
) -> crate::Result<u64> {
    let parent = target
        .parent()
        .ok_or_else(|| input("Invalid migration target"))?;
    fs::create_dir_all(parent).await?;
    let temporary =
        parent.join(format!(".orbiont-move-{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        let mut output = fs::File::create(&temporary).await?;
        let copied = copy_and_hash(
            fs::File::open(&source).await?,
            &mut output,
            &|_| {},
        )
        .await?;
        output.flush().await?;
        output.sync_all().await?;
        drop(output);
        fs::set_permissions(&temporary, metadata.permissions()).await?;
        let after = fs::metadata(&source).await?;
        if after.len() != copied.size
            || after.modified().ok() != metadata.modified().ok()
            || hash_file(&temporary).await? != copied
        {
            return Err(input(
                "File changed or failed verification during directory migration",
            ));
        }
        fs::rename(&temporary, &target).await?;
        sync_directory(parent).await?;
        Ok(copied.size)
    }
    .await;
    if result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    result
}

async fn create_link(
    link: &Path,
    target: &Path,
    original: &Path,
) -> crate::Result<()> {
    #[cfg(unix)]
    {
        let _ = original;
        fs::symlink(link, target).await?;
    }
    #[cfg(windows)]
    {
        if fs::metadata(original).await?.is_dir() {
            fs::symlink_dir(link, target).await?;
        } else {
            match fs::symlink_file(link, target).await {
                Ok(()) => {}
                Err(error)
                    if crate::state::content_store::link_unavailable(
                        &error,
                    ) =>
                {
                    crate::state::content_store::writable_copy(
                        original, target,
                    )
                    .await?;
                    fs::File::options()
                        .write(true)
                        .open(target)
                        .await?
                        .sync_all()
                        .await?;
                    if hash_file(original).await? != hash_file(target).await? {
                        return Err(input(
                            "Symlink fallback copy changed during migration",
                        ));
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}

fn remap(path: &Path, mappings: &[(PathBuf, PathBuf)]) -> PathBuf {
    let map = |path: &Path| {
        mappings.iter().find_map(|(from, to)| {
            strip_directory_prefix(path, from).map(|suffix| to.join(suffix))
        })
    };
    map(&normalize(path))
        .or_else(|| {
            std::fs::canonicalize(path).ok().and_then(|path| map(&path))
        })
        .unwrap_or_else(|| path.to_path_buf())
}

fn strip_directory_prefix<'a>(
    path: &'a Path,
    directory: &Path,
) -> Option<&'a Path> {
    let mut components = path.components();
    for expected in directory.components() {
        let actual = components.next()?;
        #[cfg(windows)]
        let matches = {
            use std::path::{Component, Prefix};
            match (actual, expected) {
                (Component::Prefix(actual), Component::Prefix(expected)) => {
                    match (actual.kind(), expected.kind()) {
                        (
                            Prefix::Disk(a) | Prefix::VerbatimDisk(a),
                            Prefix::Disk(b) | Prefix::VerbatimDisk(b),
                        ) => a.eq_ignore_ascii_case(&b),
                        (
                            Prefix::UNC(a, share_a)
                            | Prefix::VerbatimUNC(a, share_a),
                            Prefix::UNC(b, share_b)
                            | Prefix::VerbatimUNC(b, share_b),
                        ) => {
                            a.eq_ignore_ascii_case(b)
                                && share_a.eq_ignore_ascii_case(share_b)
                        }
                        _ => actual == expected,
                    }
                }
                _ => actual == expected,
            }
        };
        #[cfg(not(windows))]
        let matches = actual == expected;
        if !matches {
            return None;
        }
    }
    Some(components.as_path())
}

fn rewrite_value(value: &mut Value, mappings: &[(PathBuf, PathBuf)]) {
    match value {
        Value::String(value) => {
            let path = Path::new(value);
            if path.is_absolute() {
                *value = dunce::simplified(&remap(path, mappings))
                    .to_string_lossy()
                    .into_owned();
            }
        }
        Value::Array(values) => {
            for value in values {
                rewrite_value(value, mappings);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                rewrite_value(value, mappings);
            }
        }
        _ => {}
    }
}

async fn rewrite_database_paths(
    pool: &SqlitePool,
    mappings: &[(PathBuf, PathBuf)],
    checkpoint: &str,
) -> crate::Result<()> {
    let java_versions = JavaVersion::get_all_registered(pool).await?;
    let instances = instance_rows::list_instances(pool).await?;
    let mut overrides_by_instance = std::collections::HashMap::new();
    for instance in &instances {
        overrides_by_instance.insert(
            instance.id.clone(),
            instance_rows::get_instance_launch_overrides(&instance.id, pool)
                .await?,
        );
    }
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    for (_, mut java) in java_versions {
        java.path = dunce::simplified(&remap(Path::new(&java.path), mappings))
            .to_string_lossy()
            .into_owned();
        java.upsert(&mut *tx).await?;
    }
    for instance in instances {
        let mut value = serde_json::to_value(&instance)?;
        rewrite_value(&mut value, mappings);
        let updated: Instance = serde_json::from_value(value)?;
        let overrides = overrides_by_instance.remove(&instance.id).flatten();
        instance_rows::update_instance(&updated, &mut tx).await?;
        if let Some(overrides) = overrides {
            let mut value = serde_json::to_value(&overrides)?;
            rewrite_value(&mut value, mappings);
            instance_rows::upsert_instance_launch_overrides(
                &serde_json::from_value(value)?,
                &mut tx,
            )
            .await?;
        }
    }
    let jobs = sqlx::query!(
        "SELECT id, json(state) AS \"state!: String\" FROM install_jobs"
    )
    .fetch_all(&mut *tx)
    .await?;
    for job in jobs {
        let mut state: Value = serde_json::from_str(&job.state)?;
        rewrite_value(&mut state, mappings);
        let state = serde_json::to_string(&state)?;
        sqlx::query!(
            "UPDATE install_jobs SET state = jsonb(?) WHERE id = ?",
            state,
            job.id
        )
        .execute(&mut *tx)
        .await?;
    }
    content_store::set_setting(
        &mut *tx,
        "store_directory_move_copied",
        checkpoint,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// `on_progress` receives the completed fraction of the copy, from 0 to 1.
pub(crate) async fn move_app_directory(
    from: &Path,
    to: &Path,
    pool: &SqlitePool,
    on_progress: &(dyn Fn(f64) + Send + Sync),
) -> crate::Result<()> {
    let original_from = normalize(from);
    fs::create_dir_all(to).await?;
    let from = fs::canonicalize(from).await?;
    let to = fs::canonicalize(to).await?;
    if from == to {
        return Ok(());
    }
    if from.starts_with(&to) || to.starts_with(&from) {
        return Err(input(
            "The new app directory cannot contain, or be inside, the old app directory",
        ));
    }
    let processes = sqlx::query!("SELECT pid, start_time FROM processes")
        .fetch_all(pool)
        .await?;
    let system = sysinfo::System::new_all();
    if processes.iter().any(|process| {
        u32::try_from(process.pid)
            .ok()
            .and_then(|pid| system.process(sysinfo::Pid::from_u32(pid)))
            .is_some_and(|running| {
                (running.start_time() as i64).abs_diff(process.start_time) <= 2
            })
    }) {
        return Err(input(
            "Close Minecraft instances before moving the app directory",
        ));
    }
    let mappings = [&from, &original_from]
        .into_iter()
        .flat_map(|source| {
            let destination = &to;
            MOVED_APP_DIRECTORIES.iter().map(move |directory| {
                (source.join(directory), destination.join(directory))
            })
        })
        .collect::<Vec<_>>();
    let checkpoint = serde_json::to_string(&(from.clone(), to.clone()))?;
    if let Some(previous) =
        content_store::setting(pool, "store_directory_move").await?
        && !previous.is_empty()
        && previous != checkpoint
    {
        return Err(input(
            "Finish or cancel the previous app-directory move before choosing another destination",
        ));
    }
    content_store::set_setting(pool, "store_directory_move", &checkpoint)
        .await?;
    let managed_files = managed_content::prepare(pool, &from, &to).await?;
    let managed_paths = managed_files
        .iter()
        .map(|file| file.source.clone())
        .collect();
    let mut required = 0_u64;
    for directory in MOVED_APP_DIRECTORIES {
        let source = from.join(directory);
        match fs::symlink_metadata(&source).await {
            Ok(metadata)
                if !metadata.is_dir() || metadata.file_type().is_symlink() =>
            {
                return Err(input(format!(
                    "The shared-data root {} must be a directory without a symbolic link before moving the app directory",
                    source.display(),
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(error) => return Err(error.into()),
        }
        if fs::try_exists(&source).await? {
            required = required.saturating_add(
                required_copy_bytes(
                    &source,
                    &to.join(directory),
                    &managed_paths,
                )
                .await?,
            );
        }
    }
    ensure_move_space(&to, required)?;
    let copied = AtomicU64::new(0);
    let on_copied = |bytes: u64| {
        let done = copied.fetch_add(bytes, Ordering::Relaxed) + bytes;
        on_progress(
            COPY_PROGRESS_SHARE
                * (done as f64 / required.max(1) as f64).min(1.0),
        );
    };
    for directory in MOVED_APP_DIRECTORIES {
        let source = from.join(directory);
        if fs::try_exists(&source).await? {
            copy_tree(
                &source,
                &to.join(directory),
                &mappings,
                &managed_paths,
                &on_copied,
            )
            .await?;
        }
    }
    on_progress(COPY_PROGRESS_SHARE);
    let managed_count = managed_files.len().max(1) as f64;
    managed_content::copy_and_checkpoint(
        pool,
        managed_files,
        &from,
        &to,
        &|moved| {
            on_progress(
                COPY_PROGRESS_SHARE
                    + (1.0 - COPY_PROGRESS_SHARE) * moved as f64
                        / managed_count,
            );
        },
    )
    .await?;
    rewrite_database_paths(pool, &mappings, &checkpoint).await?;
    Ok(())
}

pub(crate) async fn finish_app_directory_move(
    from: &Path,
    to: &Path,
    pool: &SqlitePool,
) -> crate::Result<()> {
    let from = fs::canonicalize(from).await?;
    let to = fs::canonicalize(to).await?;
    if from == to {
        return Ok(());
    }
    if from.starts_with(&to) || to.starts_with(&from) {
        return Err(input("Invalid app-directory cleanup roots"));
    }
    let expected = serde_json::to_string(&(from.clone(), to.clone()))?;
    if content_store::setting(pool, "store_directory_move_copied")
        .await?
        .as_deref()
        != Some(expected.as_str())
    {
        return Err(input(
            "The app-directory copy has not completed; preserving the source",
        ));
    }
    managed_content::commit(pool).await?;
    for directory in MOVED_APP_DIRECTORIES {
        let source = from.join(directory);
        if fs::try_exists(&source).await?
            && fs::try_exists(to.join(directory)).await?
        {
            remove_migrated_tree(&source).await?;
        }
    }
    content_store::set_setting(pool, "store_directory_move", "").await?;
    content_store::set_setting(pool, "store_directory_move_copied", "").await?;
    managed_content::clear_checkpoint(pool).await?;
    Ok(())
}

fn ensure_move_space(destination: &Path, required: u64) -> crate::Result<()> {
    if fs4::available_space(destination)?
        < required.saturating_add(64 * 1024 * 1024)
    {
        return Err(input(format!(
            "Not enough space: the destination needs at least {required} bytes plus working space for the app-directory move"
        )));
    }
    Ok(())
}

async fn required_copy_bytes(
    from: &Path,
    to: &Path,
    managed_paths: &std::collections::HashSet<PathBuf>,
) -> crate::Result<u64> {
    let mut pending = vec![(from.to_path_buf(), to.to_path_buf())];
    let mut bytes = 0_u64;
    while let Some((source, target)) = pending.pop() {
        if managed_paths.contains(&source) {
            continue;
        }
        let metadata = fs::symlink_metadata(&source).await?;
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            let mut entries = fs::read_dir(&source).await?;
            while let Some(entry) = entries.next_entry().await? {
                pending.push((entry.path(), target.join(entry.file_name())));
            }
        } else if metadata.is_file()
            && fs::symlink_metadata(&target).await.is_err()
        {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(bytes)
}

pub(crate) async fn resume_completed_move(
    destination: &Path,
    pool: &SqlitePool,
) -> crate::Result<()> {
    if let Some(checkpoint) =
        content_store::setting(pool, "store_directory_move").await?
        && !checkpoint.is_empty()
    {
        let (from, to): (PathBuf, PathBuf) = serde_json::from_str(&checkpoint)?;
        if fs::canonicalize(destination).await? == to {
            finish_app_directory_move(&from, &to, pool).await?;
        }
    }
    Ok(())
}

pub(crate) async fn cancel_move(pool: &SqlitePool) -> crate::Result<()> {
    if let Some(checkpoint) =
        content_store::setting(pool, "store_directory_move").await?
        && !checkpoint.is_empty()
    {
        let (from, to): (PathBuf, PathBuf) = serde_json::from_str(&checkpoint)?;
        let settings = crate::state::Settings::get(pool).await?;
        let committed = match settings.prev_custom_dir.as_deref() {
            Some(previous) => fs::canonicalize(previous).await? == to,
            None => false,
        };
        if committed {
            return Err(input(
                "The move has committed; finish its cleanup before moving back",
            ));
        }
        let mappings = MOVED_APP_DIRECTORIES
            .iter()
            .map(|directory| (to.join(directory), from.join(directory)))
            .collect::<Vec<_>>();
        if content_store::setting(pool, "store_directory_move_copied")
            .await?
            .as_deref()
            == Some(checkpoint.as_str())
        {
            rewrite_database_paths(pool, &mappings, "").await?;
        }
        content_store::set_setting(pool, "store_directory_move", "").await?;
        content_store::set_setting(pool, "store_directory_move_copied", "")
            .await?;
        managed_content::clear_checkpoint(pool).await?;
    }
    Ok(())
}

pub(crate) async fn remove_migrated_tree(root: &Path) -> crate::Result<()> {
    if fs::symlink_metadata(root).await?.file_type().is_symlink() {
        return Err(input(
            "A migrated directory was replaced by a link; preserving it",
        ));
    }
    #[cfg(windows)]
    {
        let mut pending = vec![root.to_path_buf()];
        while let Some(path) = pending.pop() {
            let metadata = fs::symlink_metadata(&path).await?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                let mut entries = fs::read_dir(&path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    pending.push(entry.path());
                }
            } else if metadata.is_file() && metadata.permissions().readonly() {
                let mut permissions = metadata.permissions();
                // Owner write only: `set_readonly(false)` on Unix would make
                // the file writable by everyone.
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    permissions.set_mode(permissions.mode() | 0o200);
                }
                #[cfg(not(unix))]
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                fs::set_permissions(path, permissions).await?;
            }
        }
    }
    fs::remove_dir_all(root).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    #[tokio::test]
    async fn concurrent_copy_reports_every_byte_and_leaves_no_temporaries() {
        let root = tempfile::tempdir().unwrap();
        let from = root.path().join("from");
        let to = root.path().join("to");
        let mut expected = 0_u64;
        for index in 0..(COPY_CONCURRENCY * 3) {
            let folder = from.join(format!("folder-{}", index % 4));
            fs::create_dir_all(&folder).await.unwrap();
            let body = vec![index as u8; 1000 + index];
            expected += body.len() as u64;
            fs::write(folder.join(format!("file-{index}.bin")), body)
                .await
                .unwrap();
        }
        let reported = Mutex::new(Vec::new());
        copy_tree(&from, &to, &[], &HashSet::new(), &|bytes| {
            reported.lock().unwrap().push(bytes)
        })
        .await
        .unwrap();
        let reported = reported.into_inner().unwrap();
        assert_eq!(reported.len(), COPY_CONCURRENCY * 3);
        assert_eq!(reported.iter().sum::<u64>(), expected);
        for index in 0..(COPY_CONCURRENCY * 3) {
            let name = format!("folder-{}/file-{index}.bin", index % 4);
            assert_eq!(
                fs::read(to.join(&name)).await.unwrap(),
                fs::read(from.join(&name)).await.unwrap()
            );
        }
        for index in 0..4 {
            let mut entries = fs::read_dir(to.join(format!("folder-{index}")))
                .await
                .unwrap();
            while let Some(entry) = entries.next_entry().await.unwrap() {
                assert!(!entry.file_name().to_string_lossy().ends_with(".tmp"));
            }
        }
    }

    #[tokio::test]
    async fn resumed_copy_skips_identical_files_and_rejects_different_data() {
        let root = tempfile::tempdir().unwrap();
        let from = root.path().join("from");
        let to = root.path().join("to");
        fs::create_dir_all(&from).await.unwrap();
        fs::create_dir_all(&to).await.unwrap();
        fs::write(from.join("same.txt"), b"same").await.unwrap();
        fs::write(to.join("same.txt"), b"same").await.unwrap();
        fs::write(from.join("new.txt"), b"new data").await.unwrap();
        let reported = Mutex::new(0_u64);
        copy_tree(&from, &to, &[], &HashSet::new(), &|bytes| {
            *reported.lock().unwrap() += bytes
        })
        .await
        .unwrap();
        // Files already copied by an interrupted move are verified, not counted again.
        assert_eq!(*reported.lock().unwrap(), 8);
        assert_eq!(fs::read(to.join("new.txt")).await.unwrap(), b"new data");

        fs::write(to.join("same.txt"), b"different").await.unwrap();
        assert!(
            copy_tree(&from, &to, &[], &HashSet::new(), &|_| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(to.join("same.txt")).await.unwrap(), b"different");
    }
}
