use crate::api::Result;
use tauri::Runtime;
use tauri_plugin_dialog::DialogExt;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("files")
        .invoke_handler(tauri::generate_handler![
            file_extract_zip,
            file_save_as,
            file_read_dragged_file,
            file_list,
            file_read_document,
            file_write_document,
            file_list_recoveries,
            file_preview_recovery,
            file_restore_recovery,
            file_remove_recoveries,
            file_storage_summary,
            file_read,
            file_write,
            file_create_directory,
            file_rename,
            file_delete,
        ])
        .build()
}

#[tauri::command]
pub async fn file_read_dragged_file(path: String) -> Result<Vec<u8>> {
    let metadata = tokio::fs::metadata(&path).await?;
    if !metadata.is_file() {
        return Err(theseus::Error::from(theseus::ErrorKind::OtherError(
            "Dropped path is not a file".to_string(),
        ))
        .into());
    }

    const LIMIT: u64 = 64 * 1024 * 1024;
    if metadata.len() > LIMIT {
        return Err(theseus::Error::from(theseus::ErrorKind::InputError("Dropped file exceeds 64 MiB IPC limit; import larger packs by path".into())).into());
    }
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    tokio::fs::File::open(path)
        .await?
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > LIMIT {
        return Err(theseus::Error::from(theseus::ErrorKind::InputError(
            "Dropped file exceeds 64 MiB IPC limit".into(),
        ))
        .into());
    }
    Ok(bytes)
}

#[tauri::command]
pub async fn file_extract_zip(
    instance_id: &str,
    file_path: &str,
    override_conflicts: bool,
    dry_run: bool,
) -> Result<Option<theseus::instance::ExtractDryRunResult>> {
    Ok(theseus::instance::extract_instance_zip(
        instance_id,
        file_path,
        override_conflicts,
        dry_run,
    )
    .await?)
}

#[tauri::command]
pub async fn file_list(
    instance_id: &str,
    path: &str,
) -> Result<Vec<theseus::instance::InstanceFileItem>> {
    Ok(theseus::instance::list_instance_files(instance_id, path).await?)
}

#[tauri::command]
pub async fn file_read(instance_id: &str, path: &str) -> Result<Vec<u8>> {
    Ok(theseus::instance::read_instance_file(instance_id, path).await?)
}

#[tauri::command]
pub async fn file_write(
    instance_id: &str,
    path: &str,
    bytes: Vec<u8>,
    create_only: bool,
) -> Result<()> {
    Ok(theseus::instance::write_instance_file(
        instance_id,
        path,
        &bytes,
        create_only,
    )
    .await?)
}

#[tauri::command]
pub async fn file_create_directory(
    instance_id: &str,
    path: &str,
) -> Result<()> {
    Ok(theseus::instance::create_instance_directory(instance_id, path).await?)
}

#[tauri::command]
pub async fn file_rename(
    instance_id: &str,
    source: &str,
    destination: &str,
) -> Result<()> {
    Ok(theseus::instance::rename_instance_file(
        instance_id,
        source,
        destination,
    )
    .await?)
}

#[tauri::command]
pub async fn file_delete(
    instance_id: &str,
    path: &str,
    recursive: bool,
) -> Result<()> {
    Ok(
        theseus::instance::delete_instance_file(instance_id, path, recursive)
            .await?,
    )
}

#[tauri::command]
pub async fn file_save_as<R: Runtime>(
    app: tauri::AppHandle<R>,
    instance_id: &str,
    file_path: &str,
) -> Result<()> {
    let source = std::path::Path::new(file_path);
    let file_name = source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(&file_name)
        .save_file(|path| {
            let _ = tx.send(path);
        });

    if let Some(dest) = rx.await.unwrap_or(None) {
        let dest_path = std::path::PathBuf::try_from(dest).map_err(|e| {
            theseus::Error::from(theseus::ErrorKind::OtherError(format!(
                "Invalid save path: {e}"
            )))
        })?;
        theseus::instance::save_instance_file_as(
            instance_id,
            file_path,
            &dest_path,
        )
        .await?;
    }

    Ok(())
}

#[tauri::command]
pub async fn file_read_document(
    instance_id: &str,
    path: &str,
) -> Result<theseus::instance::FileDocument> {
    Ok(theseus::instance::read_instance_document(instance_id, path).await?)
}
#[tauri::command]
pub async fn file_write_document(
    instance_id: &str,
    path: &str,
    content: &str,
    expected_revision: &str,
) -> Result<theseus::instance::FileDocumentWrite> {
    Ok(theseus::instance::write_instance_document(
        instance_id,
        path,
        content,
        expected_revision,
    )
    .await?)
}
#[tauri::command]
pub async fn file_list_recoveries(
    instance_id: &str,
) -> Result<theseus::instance::FileRecoveries> {
    Ok(theseus::instance::list_instance_recoveries(instance_id).await?)
}
#[tauri::command]
pub async fn file_preview_recovery(
    instance_id: &str,
    recovery_id: &str,
) -> Result<theseus::instance::FileRecoveryPreview> {
    Ok(
        theseus::instance::preview_instance_recovery(instance_id, recovery_id)
            .await?,
    )
}
#[tauri::command]
pub async fn file_restore_recovery(
    instance_id: &str,
    recovery_id: &str,
    expected_revision: &str,
) -> Result<theseus::instance::FileDocumentWrite> {
    Ok(theseus::instance::restore_instance_recovery(
        instance_id,
        recovery_id,
        expected_revision,
    )
    .await?)
}
#[tauri::command]
pub async fn file_remove_recoveries(
    instance_id: &str,
    recovery_ids: Vec<String>,
) -> Result<()> {
    Ok(theseus::instance::remove_instance_recoveries(
        instance_id,
        &recovery_ids,
    )
    .await?)
}
#[tauri::command]
pub async fn file_storage_summary(
    instance_id: &str,
) -> Result<theseus::instance::FileStorageSummary> {
    Ok(theseus::instance::instance_storage_summary(instance_id).await?)
}
