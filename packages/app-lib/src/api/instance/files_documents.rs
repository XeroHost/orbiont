use super::{
    BACKUP_DIRECTORY, FileContext, fs, input, is_link, require_no_links,
};
use crate::State;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_RECOVERIES: usize = 1000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDocument {
    pub content: String,
    pub revision: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileDocumentWrite {
    pub revision: String,
    pub recovery_id: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecovery {
    pub id: String,
    pub path: String,
    pub created: u64,
    pub size: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecoveries {
    pub entries: Vec<FileRecovery>,
    pub incomplete: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecoveryPreview {
    pub content: String,
    pub revision: String,
    pub current_revision: String,
    pub path: String,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStorageSummary {
    pub data_bytes: u64,
    pub cache_bytes: u64,
    pub backup_bytes: u64,
    pub incomplete: bool,
}

fn revision(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
async fn read_bounded(path: &Path, limit: u64) -> crate::Result<Vec<u8>> {
    let file = fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(input("Document exceeds the UTF-8 editor limit"));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).await?;
    if bytes.len() as u64 > limit {
        return Err(input("Document exceeds the UTF-8 editor limit"));
    }
    Ok(bytes)
}
async fn document(path: &Path) -> crate::Result<FileDocument> {
    let bytes = read_bounded(path, MAX_DOCUMENT_BYTES).await?;
    let revision = revision(&bytes);
    let content = String::from_utf8(bytes)
        .map_err(|_| input("Document is not valid UTF-8"))?;
    Ok(FileDocument { content, revision })
}

async fn atomic_replace(
    path: &Path,
    bytes: &[u8],
    expected: Option<&str>,
) -> crate::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| input("Invalid document path"))?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    let mut file = fs::File::create(&temporary).await?;
    file.write_all(bytes).await?;
    file.sync_all().await?;
    drop(file);
    if let Some(expected) = expected
        && revision(&read_bounded(path, MAX_DOCUMENT_BYTES).await?) != expected
    {
        return Err(input("Document changed externally"));
    }
    temporary.persist(path).map_err(|error| error.error)?;
    #[cfg(unix)]
    {
        fs::File::open(parent).await?.sync_all().await?;
    }
    Ok(())
}

async fn backup_root(context: &FileContext) -> crate::Result<PathBuf> {
    let root = context.base.join(BACKUP_DIRECTORY);
    require_no_links(&context.base, &root).await?;
    fs::create_dir_all(&root).await?;
    require_no_links(&context.base, &root).await?;
    Ok(root)
}
fn recovery_id(id: &str) -> crate::Result<()> {
    let parsed =
        uuid::Uuid::parse_str(id).map_err(|_| input("Invalid recovery ID"))?;
    if parsed.to_string() != id {
        return Err(input("Invalid recovery ID"));
    }
    Ok(())
}
async fn recovery(
    context: &FileContext,
    id: &str,
) -> crate::Result<(FileRecovery, PathBuf)> {
    recovery_id(id)?;
    let root = context.base.join(BACKUP_DIRECTORY).join(id);
    require_no_links(&context.base, &root.join("metadata.json")).await?;
    require_no_links(&context.base, &root.join("original")).await?;
    let info: FileRecovery = serde_json::from_slice(
        &read_bounded(&root.join("metadata.json"), 16 * 1024).await?,
    )?;
    if info.id != id || info.size > MAX_DOCUMENT_BYTES {
        return Err(input("Invalid recovery metadata"));
    }
    context.resolve(&info.path, true).await?;
    Ok((info, root))
}
async fn save_document(
    context: &FileContext,
    path: &str,
    content: &str,
    expected: &str,
) -> crate::Result<FileDocumentWrite> {
    if content.len() as u64 > MAX_DOCUMENT_BYTES {
        return Err(input("Document exceeds the UTF-8 editor limit"));
    }
    let destination = context.resolve(path, true).await?.path().to_path_buf();
    let original = read_bounded(&destination, MAX_DOCUMENT_BYTES).await?;
    std::str::from_utf8(&original)
        .map_err(|_| input("Document is not valid UTF-8"))?;
    if revision(&original) != expected {
        return Err(input("Document changed externally"));
    }
    let root = backup_root(context).await?;
    let id = uuid::Uuid::new_v4().to_string();
    let backup = root.join(&id);
    fs::create_dir(&backup).await?;
    let info = FileRecovery {
        id: id.clone(),
        path: super::normalized(path).to_string(),
        size: original.len() as u64,
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    };
    // Both files are durable before the user document can be replaced.
    atomic_replace(&backup.join("original"), &original, None).await?;
    atomic_replace(
        &backup.join("metadata.json"),
        &serde_json::to_vec(&info)?,
        None,
    )
    .await?;
    context.resolve(path, true).await?;
    require_no_links(&context.base, &backup).await?;
    atomic_replace(&destination, content.as_bytes(), Some(expected)).await?;
    Ok(FileDocumentWrite {
        revision: revision(content.as_bytes()),
        recovery_id: id,
    })
}

pub async fn read_instance_document(
    instance_id: &str,
    path: &str,
) -> crate::Result<FileDocument> {
    let state = State::get().await?;
    let context = FileContext::new(&state, instance_id).await?;
    document(context.resolve(path, false).await?.path()).await
}
pub async fn write_instance_document(
    instance_id: &str,
    path: &str,
    content: &str,
    expected_revision: &str,
) -> crate::Result<FileDocumentWrite> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    save_document(
        &FileContext::new(&state, instance_id).await?,
        path,
        content,
        expected_revision,
    )
    .await
}
pub async fn list_instance_recoveries(
    instance_id: &str,
) -> crate::Result<FileRecoveries> {
    let state = State::get().await?;
    let context = FileContext::new(&state, instance_id).await?;
    list_recoveries(&context).await
}

async fn list_recoveries(
    context: &FileContext,
) -> crate::Result<FileRecoveries> {
    let root = context.base.join(BACKUP_DIRECTORY);
    require_no_links(&context.base, &root).await?;
    let mut output = FileRecoveries {
        entries: Vec::new(),
        incomplete: false,
    };
    let mut entries = match fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(output);
        }
        Err(error) => return Err(error.into()),
    };
    // Directory enumeration order is arbitrary. Scan every candidate while
    // retaining only the newest MAX_RECOVERIES entries in bounded memory.
    let mut newest = BTreeMap::new();
    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(_) => {
                output.incomplete = true;
                break;
            }
        };
        let id = entry.file_name().to_string_lossy().to_string();
        match recovery(context, &id).await {
            Ok((info, _)) => {
                newest.insert((Reverse(info.created), info.id.clone()), info);
                if newest.len() > MAX_RECOVERIES {
                    newest.pop_last();
                    output.incomplete = true;
                }
            }
            Err(_) => output.incomplete = true,
        }
    }
    output.entries = newest.into_values().collect();
    Ok(output)
}
async fn current_revision(
    context: &FileContext,
    path: &str,
) -> crate::Result<String> {
    let destination = context.resolve(path, true).await?.path().to_path_buf();
    if !fs::try_exists(&destination).await? {
        return Ok("missing".into());
    }
    Ok(document(&destination).await?.revision)
}
async fn restore_document(
    context: &FileContext,
    info: &FileRecovery,
    original: &str,
    expected: &str,
) -> crate::Result<FileDocumentWrite> {
    if expected != "missing" {
        return save_document(context, &info.path, original, expected).await;
    }
    let destination = context
        .resolve(&info.path, true)
        .await?
        .path()
        .to_path_buf();
    if fs::try_exists(&destination).await? {
        return Err(input("Document changed externally"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| input("Invalid document destination"))?;
    fs::create_dir_all(parent).await?;
    context.resolve(&info.path, true).await?;
    let temporary = tempfile::NamedTempFile::new_in(parent)?.into_temp_path();
    let mut file = fs::File::create(&temporary).await?;
    file.write_all(original.as_bytes()).await?;
    file.sync_all().await?;
    drop(file);
    context.resolve(&info.path, true).await?;
    temporary
        .persist_noclobber(&destination)
        .map_err(|_| input("Document changed externally"))?;
    #[cfg(unix)]
    {
        fs::File::open(parent).await?.sync_all().await?;
    }
    // No previous document existed; retain the selected original recovery.
    Ok(FileDocumentWrite {
        revision: revision(original.as_bytes()),
        recovery_id: info.id.clone(),
    })
}

pub async fn preview_instance_recovery(
    instance_id: &str,
    id: &str,
) -> crate::Result<FileRecoveryPreview> {
    let state = State::get().await?;
    let context = FileContext::new(&state, instance_id).await?;
    let (info, root) = recovery(&context, id).await?;
    let original = document(&root.join("original")).await?;
    let current = current_revision(&context, &info.path).await?;
    Ok(FileRecoveryPreview {
        content: original.content,
        revision: original.revision,
        current_revision: current,
        path: info.path,
    })
}
pub async fn restore_instance_recovery(
    instance_id: &str,
    id: &str,
    expected_revision: &str,
) -> crate::Result<FileDocumentWrite> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let context = FileContext::new(&state, instance_id).await?;
    let (info, root) = recovery(&context, id).await?;
    let original = document(&root.join("original")).await?;
    restore_document(&context, &info, &original.content, expected_revision)
        .await
}
pub async fn remove_instance_recoveries(
    instance_id: &str,
    ids: &[String],
) -> crate::Result<()> {
    if ids.len() > MAX_RECOVERIES {
        return Err(input("Too many recovery copies"));
    }
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let context = FileContext::new(&state, instance_id).await?;
    let mut roots = Vec::new();
    for id in ids {
        roots.push(recovery(&context, id).await?.1);
    }
    for root in roots {
        // No recursive deletion: never remove unexpected content or follow links.
        fs::remove_file(root.join("original")).await?;
        fs::remove_file(root.join("metadata.json")).await?;
        fs::remove_dir(root).await?;
    }
    Ok(())
}
pub async fn instance_storage_summary(
    instance_id: &str,
) -> crate::Result<FileStorageSummary> {
    let state = State::get().await?;
    let context = FileContext::new(&state, instance_id).await?;
    let mut result = storage_summary(&context.base).await?;
    let caches = state.directories.caches_dir();
    if fs::try_exists(&caches).await? {
        match storage_summary(&caches).await {
            Ok(summary) => {
                result.cache_bytes = summary.data_bytes + summary.backup_bytes;
                result.incomplete |= summary.incomplete;
            }
            Err(_) => result.incomplete = true,
        }
    }
    Ok(result)
}
async fn storage_summary(base: &Path) -> crate::Result<FileStorageSummary> {
    require_no_links(base, base).await?;
    let mut summary = FileStorageSummary::default();
    let mut pending = vec![(base.to_path_buf(), 0_u8, 0_usize)];
    let mut scanned = 0;
    while let Some((path, category, depth)) = pending.pop() {
        if depth > 64 {
            summary.incomplete = true;
            continue;
        }
        let Ok(mut entries) = fs::read_dir(&path).await else {
            summary.incomplete = true;
            continue;
        };
        while let Some(entry) = entries.next_entry().await? {
            scanned += 1;
            if scanned > 100_000 {
                summary.incomplete = true;
                return Ok(summary);
            }
            let Ok(metadata) = fs::symlink_metadata(entry.path()).await else {
                summary.incomplete = true;
                continue;
            };
            if is_link(&metadata) {
                summary.incomplete = true;
                continue;
            }
            let name = entry.file_name();
            let category = if depth == 0
                && name
                    .to_string_lossy()
                    .eq_ignore_ascii_case(BACKUP_DIRECTORY)
            {
                2
            } else {
                category
            };
            if metadata.is_dir() {
                pending.push((entry.path(), category, depth + 1));
            } else if metadata.is_file() {
                match category {
                    2 => summary.backup_bytes += metadata.len(),
                    1 => summary.cache_bytes += metadata.len(),
                    _ => summary.data_bytes += metadata.len(),
                }
            }
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(base: &Path) -> FileContext {
        FileContext {
            base: base.to_path_buf(),
            protected: vec!["resourcepacks/managed.zip".into()],
        }
    }

    async fn recovery_fixture(context: &FileContext, id: u128, created: u64) {
        let id = uuid::Uuid::from_u128(id).to_string();
        let root = backup_root(context).await.unwrap().join(&id);
        fs::create_dir(&root).await.unwrap();
        fs::write(root.join("original"), b"fixture").await.unwrap();
        fs::write(
            root.join("metadata.json"),
            serde_json::to_vec(&FileRecovery {
                id,
                path: "fixture.txt".into(),
                created,
                size: 7,
            })
            .unwrap(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn recoveries_return_newest_thousand_across_all_directory_entries() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        // More than 1000 valid copies: UUID order intentionally opposes time.
        for i in 1..=(MAX_RECOVERIES + 25) {
            recovery_fixture(&ctx, i as u128, (MAX_RECOVERIES + 26 - i) as u64)
                .await;
        }
        let result = list_recoveries(&ctx).await.unwrap();
        assert!(result.incomplete);
        assert_eq!(result.entries.len(), MAX_RECOVERIES);
        assert_eq!(result.entries.first().unwrap().created, 1025);
        assert_eq!(result.entries.last().unwrap().created, 26);
        for pair in result.entries.windows(2) {
            assert!(pair[0].created > pair[1].created);
        }
        // Add a newest copy after the first enumeration and an invalid candidate.
        recovery_fixture(&ctx, 2000, 5000).await;
        fs::create_dir(ctx.base.join(BACKUP_DIRECTORY).join("invalid-fixture"))
            .await
            .unwrap();
        let next = list_recoveries(&ctx).await.unwrap();
        assert!(next.incomplete);
        assert_eq!(next.entries.len(), MAX_RECOVERIES);
        assert_eq!(
            next.entries.first().unwrap().id,
            uuid::Uuid::from_u128(2000).to_string()
        );
        assert_eq!(next.entries.last().unwrap().created, 27);
    }

    #[tokio::test]
    async fn recoveries_break_date_ties_by_id_and_report_invalid_entries() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        assert!(!list_recoveries(&ctx).await.unwrap().incomplete);
        for id in [3, 1, 2] {
            recovery_fixture(&ctx, id, 42).await;
        }
        let complete = list_recoveries(&ctx).await.unwrap();
        assert!(!complete.incomplete);
        assert_eq!(
            complete
                .entries
                .iter()
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>(),
            [1, 2, 3]
                .into_iter()
                .map(|id| uuid::Uuid::from_u128(id).to_string())
                .collect::<Vec<_>>()
        );
        fs::write(
            ctx.base.join(BACKUP_DIRECTORY).join("unexpected"),
            b"fixture",
        )
        .await
        .unwrap();
        let partial = list_recoveries(&ctx).await.unwrap();
        assert!(partial.incomplete);
        assert_eq!(partial.entries.len(), 3);
    }
    #[tokio::test]
    async fn document_revision_backup_conflict_and_restore() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        fs::write(dir.path().join("config.txt"), b"before")
            .await
            .unwrap();
        let first = document(&dir.path().join("config.txt")).await.unwrap();
        let saved = save_document(&ctx, "config.txt", "after", &first.revision)
            .await
            .unwrap();
        let (_, root) = recovery(&ctx, &saved.recovery_id).await.unwrap();
        assert_eq!(fs::read(root.join("original")).await.unwrap(), b"before");
        fs::write(dir.path().join("config.txt"), b"external")
            .await
            .unwrap();
        assert!(
            save_document(&ctx, "config.txt", "lost", &saved.revision)
                .await
                .is_err()
        );
        assert_eq!(
            fs::read(dir.path().join("config.txt")).await.unwrap(),
            b"external"
        );
        let restored =
            save_document(&ctx, "config.txt", "before", &revision(b"external"))
                .await
                .unwrap();
        assert_ne!(restored.recovery_id, saved.recovery_id);
        assert_eq!(
            fs::read(dir.path().join("config.txt")).await.unwrap(),
            b"before"
        );
    }
    #[tokio::test]
    async fn document_limits_paths_and_failed_replace_preserve_original() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        for path in [
            "../outside",
            ".orbiont-file-backups/foo",
            "mods/mod.jar",
            "resourcepacks/managed.zip",
        ] {
            assert!(ctx.resolve(path, true).await.is_err());
        }
        fs::write(dir.path().join("a.txt"), b"original")
            .await
            .unwrap();
        assert!(
            atomic_replace(&dir.path().join("a.txt"), b"wrong", Some("stale"))
                .await
                .is_err()
        );
        assert_eq!(
            fs::read(dir.path().join("a.txt")).await.unwrap(),
            b"original"
        );
        assert!(
            save_document(
                &ctx,
                "a.txt",
                &"a".repeat(MAX_DOCUMENT_BYTES as usize + 1),
                &revision(b"original")
            )
            .await
            .is_err()
        );
        fs::write(dir.path().join("invalid"), [0xff]).await.unwrap();
        assert!(document(&dir.path().join("invalid")).await.is_err());
        assert!(recovery(&ctx, "../../outside").await.is_err());
    }
    #[tokio::test]
    async fn restore_missing_document_never_clobbers_a_new_external_file() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        fs::write(dir.path().join("config"), b"before")
            .await
            .unwrap();
        let saved =
            save_document(&ctx, "config", "after", &revision(b"before"))
                .await
                .unwrap();
        let (info, root) = recovery(&ctx, &saved.recovery_id).await.unwrap();
        let original = document(&root.join("original")).await.unwrap();
        fs::remove_file(dir.path().join("config")).await.unwrap();
        assert_eq!(current_revision(&ctx, "config").await.unwrap(), "missing");
        fs::write(dir.path().join("config"), b"external")
            .await
            .unwrap();
        assert!(
            restore_document(&ctx, &info, &original.content, "missing")
                .await
                .is_err()
        );
        assert_eq!(
            fs::read(dir.path().join("config")).await.unwrap(),
            b"external"
        );
        fs::remove_file(dir.path().join("config")).await.unwrap();
        restore_document(&ctx, &info, &original.content, "missing")
            .await
            .unwrap();
        assert_eq!(
            fs::read(dir.path().join("config")).await.unwrap(),
            b"before"
        );
    }
    #[tokio::test]
    async fn oversized_read_and_corrupted_recovery_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let file = fs::File::create(dir.path().join("large")).await.unwrap();
        file.set_len(MAX_DOCUMENT_BYTES + 1).await.unwrap();
        drop(file);
        assert!(document(&dir.path().join("large")).await.is_err());
        fs::write(dir.path().join("config"), b"original")
            .await
            .unwrap();
        let write =
            save_document(&ctx, "config", "new", &revision(b"original"))
                .await
                .unwrap();
        let (_, root) = recovery(&ctx, &write.recovery_id).await.unwrap();
        let invalid = FileRecovery {
            id: write.recovery_id.clone(),
            path: "../outside".into(),
            created: 0,
            size: 8,
        };
        fs::write(
            root.join("metadata.json"),
            serde_json::to_vec(&invalid).unwrap(),
        )
        .await
        .unwrap();
        assert!(recovery(&ctx, &write.recovery_id).await.is_err());
        assert_eq!(fs::read(dir.path().join("config")).await.unwrap(), b"new");
    }
    #[tokio::test]
    async fn storage_separates_backups_without_claiming_installed_files_are_cache()
     {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("mod.jar"), b"123").await.unwrap();
        fs::create_dir(dir.path().join(BACKUP_DIRECTORY))
            .await
            .unwrap();
        fs::write(dir.path().join(BACKUP_DIRECTORY).join("copy"), b"12")
            .await
            .unwrap();
        let summary = storage_summary(dir.path()).await.unwrap();
        assert_eq!(
            (
                summary.data_bytes,
                summary.cache_bytes,
                summary.backup_bytes
            ),
            (3, 0, 2)
        );
        assert!(!summary.incomplete);
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn blocked_replace_keeps_original_and_durable_recovery() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let path = dir.path().join("config");
        fs::write(&path, b"original").await.unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(3)
            .open(&path)
            .unwrap();
        assert!(
            save_document(
                &ctx,
                "config",
                "replacement",
                &revision(b"original")
            )
            .await
            .is_err()
        );
        assert_eq!(fs::read(&path).await.unwrap(), b"original");
        drop(lock);
        let mut entries = fs::read_dir(dir.path().join(BACKUP_DIRECTORY))
            .await
            .unwrap();
        let backup = entries.next_entry().await.unwrap().unwrap();
        assert_eq!(
            fs::read(backup.path().join("original")).await.unwrap(),
            b"original"
        );
        assert!(entries.next_entry().await.unwrap().is_none());
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn junction_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let link = dir.path().join("link");
        let result = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(&link)
            .arg(outside.path())
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(
            context(dir.path())
                .resolve("link/test", true)
                .await
                .is_err()
        );
        std::fs::remove_dir(link).unwrap();
    }
}
