use crate::State;
use crate::state::content_store;
use crate::state::content_store::{ReadableContent, content_file_path, input};
use crate::state::instances::adapters::sqlite::{content_rows, instance_rows};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::fs;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstanceFileItem {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub path: String,
    pub modified: u64,
    pub created: u64,
    pub size: Option<u64>,
    pub count: Option<usize>,
    pub read_only: bool,
}

fn normalized(path: &str) -> &str {
    path.trim_start_matches('/')
}

const BACKUP_DIRECTORY: &str = ".orbiont-file-backups";
const MAX_CHILDREN: usize = 1000;

struct FileContext {
    base: PathBuf,
    protected: Vec<String>,
}

fn is_link(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

async fn require_no_links(
    base: &Path,
    destination: &Path,
) -> crate::Result<()> {
    let relative = destination.strip_prefix(base)?;
    let mut current = base.to_path_buf();
    for component in
        std::iter::once(None).chain(relative.components().map(Some))
    {
        if let Some(component) = component {
            current.push(component);
        }
        match fs::symlink_metadata(&current).await {
            Ok(metadata) if is_link(&metadata) => {
                return Err(input(
                    "Files cannot access links or reparse points",
                ));
            }
            Ok(metadata) if current != destination && !metadata.is_dir() => {
                return Err(input("File parent is not a directory"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

impl FileContext {
    async fn new(state: &State, instance_id: &str) -> crate::Result<Self> {
        let instance =
            instance_rows::get_instance_by_id(instance_id, &state.pool)
                .await?
                .ok_or_else(|| input("Unknown instance"))?;
        let base = state
            .content_store
            .instance_path(&instance.path, "")
            .await?;
        require_no_links(&state.directories.instances_dir(), &base).await?;
        let bindings =
            content_store::instance_storage(&state.pool, instance_id).await?;
        let files =
            content_rows::get_instance_files(instance_id, &state.pool).await?;
        let mut protected = Vec::new();
        for file in files.iter().filter(|file| {
            bindings.iter().any(|binding| binding.file_id == file.id)
        }) {
            protected.push(file.relative_path.to_lowercase());
            protected.push(content_file_path(file).to_lowercase());
        }
        Ok(Self { base, protected })
    }

    async fn resolve(
        &self,
        path: &str,
        writing: bool,
    ) -> crate::Result<ReadableContent> {
        let path = normalized(path);
        if path.is_empty() && writing {
            return Err(input(
                "The instance directory cannot be changed in the Files tab",
            ));
        }
        if !path.is_empty() {
            crate::util::archive::validate_archive_path(path)?;
        }
        let first = path.split('/').next().unwrap_or_default();
        if first.eq_ignore_ascii_case(BACKUP_DIRECTORY) {
            return Err(input("Recovery copies are protected"));
        }
        if writing {
            let lower = path.to_lowercase();
            let prefix = format!("{lower}/");
            if first.eq_ignore_ascii_case("mods")
                || self.protected.iter().any(|managed| {
                    managed == &lower || managed.starts_with(&prefix)
                })
            {
                return Err(input(
                    "Managed content is read-only in Files. Use the Content tab",
                ));
            }
        }
        let destination = self.base.join(path);
        require_no_links(&self.base, &destination).await?;
        Ok(ReadableContent::Local(destination))
    }
}

async fn resolve(
    state: &State,
    instance_id: &str,
    path: &str,
    writing: bool,
) -> crate::Result<ReadableContent> {
    FileContext::new(state, instance_id)
        .await?
        .resolve(path, writing)
        .await
}

#[path = "files_documents.rs"]
mod documents;
pub use documents::*;

async fn count_children(path: &Path) -> crate::Result<Option<usize>> {
    let mut children = fs::read_dir(path).await?;
    let mut count = 0;
    while count <= MAX_CHILDREN && children.next_entry().await?.is_some() {
        count += 1;
    }
    Ok((count <= MAX_CHILDREN).then_some(count))
}

pub async fn list_instance_files(
    instance_id: &str,
    path: &str,
) -> crate::Result<Vec<InstanceFileItem>> {
    let state = State::get().await?;
    let context = FileContext::new(&state, instance_id).await?;
    let directory = context.resolve(path, false).await?;
    let mut entries = fs::read_dir(directory.path()).await?;
    let mut output = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let relative = if normalized(path).is_empty() {
            name.clone()
        } else {
            format!("{}/{name}", normalized(path))
        };
        if name.eq_ignore_ascii_case(BACKUP_DIRECTORY) {
            continue;
        }
        let file_type = entry.file_type().await?;
        let resolved = context.resolve(&relative, false).await;
        let metadata = match &resolved {
            Ok(content) => fs::metadata(content.path()).await.ok(),
            Err(_) => None,
        };
        let read_only = context.resolve(&relative, true).await.is_err();
        let count = if resolved.is_ok()
            && file_type.is_dir()
            && !file_type.is_symlink()
        {
            count_children(&entry.path()).await?
        } else {
            None
        };
        output.push(InstanceFileItem {
            name,
            path: relative,
            kind: if file_type.is_dir() {
                "directory"
            } else if resolved.is_err() && file_type.is_symlink() {
                "symlink"
            } else {
                "file"
            }
            .to_string(),
            modified: metadata
                .as_ref()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |time| time.as_secs()),
            created: metadata
                .as_ref()
                .and_then(|metadata| metadata.created().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |time| time.as_secs()),
            size: metadata
                .as_ref()
                .filter(|metadata| metadata.is_file())
                .map(|metadata| metadata.len()),
            count,
            read_only,
        });
    }
    Ok(output)
}

pub async fn read_instance_file(
    instance_id: &str,
    path: &str,
) -> crate::Result<Vec<u8>> {
    let state = State::get().await?;
    let content = resolve(&state, instance_id, path, false).await?;
    Ok(fs::read(content.path()).await?)
}

pub async fn validate_instance_file_write(
    instance_id: &str,
    path: &str,
) -> crate::Result<PathBuf> {
    let state = State::get().await?;
    Ok(resolve(&state, instance_id, path, true)
        .await?
        .path()
        .to_path_buf())
}

pub async fn write_instance_file(
    instance_id: &str,
    path: &str,
    bytes: &[u8],
    create_only: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let destination = resolve(&state, instance_id, path, true)
        .await?
        .path()
        .to_path_buf();
    if create_only && fs::symlink_metadata(&destination).await.is_ok() {
        return Err(input("A file already exists at this path"));
    }
    let parent = destination
        .parent()
        .ok_or_else(|| input("Invalid file destination"))?;
    fs::create_dir_all(parent).await?;
    let parent = parent.to_path_buf();
    let temporary = tokio::task::spawn_blocking(move || {
        tempfile::NamedTempFile::new_in(parent)
            .map(|file| file.into_temp_path())
    })
    .await??;
    fs::write(&temporary, bytes).await?;
    fs::File::options()
        .write(true)
        .open(&temporary)
        .await?
        .sync_all()
        .await?;
    resolve(&state, instance_id, path, true).await?;
    tokio::task::spawn_blocking(move || {
        if create_only {
            temporary.persist_noclobber(destination)
        } else {
            temporary.persist(destination)
        }
        .map_err(|error| error.error)
    })
    .await??;
    Ok(())
}

pub async fn create_instance_directory(
    instance_id: &str,
    path: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let destination = resolve(&state, instance_id, path, true).await?;
    fs::create_dir(destination.path()).await?;
    Ok(())
}

pub async fn rename_instance_file(
    instance_id: &str,
    source: &str,
    destination: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let source = resolve(&state, instance_id, source, true).await?;
    let destination = resolve(&state, instance_id, destination, true).await?;
    if fs::symlink_metadata(destination.path()).await.is_ok() {
        return Err(input("The destination already exists"));
    }
    fs::rename(source.path(), destination.path()).await?;
    Ok(())
}

pub async fn delete_instance_file(
    instance_id: &str,
    path: &str,
    recursive: bool,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _instance = state.lock_instance_content(instance_id).await;
    let _files = state.content_store.files_lock.lock().await;
    let path = resolve(&state, instance_id, path, true).await?;
    if fs::symlink_metadata(path.path()).await?.is_dir() {
        if recursive {
            fs::remove_dir_all(path.path()).await?;
        } else {
            fs::remove_dir(path.path()).await?;
        }
    } else {
        fs::remove_file(path.path()).await?;
    }
    Ok(())
}

pub async fn save_instance_file_as(
    instance_id: &str,
    source: &str,
    destination: &Path,
) -> crate::Result<()> {
    let state = State::get().await?;
    let _files = state.content_store.files_lock.lock().await;
    let parent = destination
        .parent()
        .ok_or_else(|| input("Invalid save destination"))?;
    let canonical_parent = fs::canonicalize(parent).await?;
    let store = fs::canonicalize(state.directories.store_dir()).await?;
    let profiles = fs::canonicalize(state.directories.instances_dir()).await?;
    if canonical_parent.starts_with(&store)
        || canonical_parent.starts_with(&profiles)
    {
        return Err(input(
            "Save a copy outside the store and instance directories",
        ));
    }
    if fs::symlink_metadata(destination)
        .await
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(input("Cannot save over a symbolic link"));
    }
    let content = resolve(&state, instance_id, source, false).await?;
    let temporary =
        tempfile::NamedTempFile::new_in(&canonical_parent)?.into_temp_path();
    fs::copy(content.path(), &temporary).await?;
    fs::File::options()
        .write(true)
        .open(&temporary)
        .await?
        .sync_all()
        .await?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

#[path = "files_zip.rs"]
mod archive_files;
pub use archive_files::*;

#[cfg(test)]
mod listing_tests {
    use super::*;
    #[tokio::test]
    async fn child_count_stops_and_marks_unknown_past_budget() {
        let directory = tempfile::tempdir().unwrap();
        for index in 0..=MAX_CHILDREN {
            fs::write(directory.path().join(index.to_string()), b"")
                .await
                .unwrap();
        }
        assert_eq!(count_children(directory.path()).await.unwrap(), None);
        fs::remove_file(directory.path().join(MAX_CHILDREN.to_string()))
            .await
            .unwrap();
        assert_eq!(
            count_children(directory.path()).await.unwrap(),
            Some(MAX_CHILDREN)
        );
    }
}
