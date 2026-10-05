use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use theseus::bedrock::{
    BedrockError, BedrockStatus, DirectoryListing, Document, ErrorCode,
    LogText, Recovery, Result, Workspace, WorldTarget,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("bedrock")
        .invoke_handler(tauri::generate_handler![
            get_status,
            launch_game,
            stop_game,
            launch_official_launcher,
            open_store,
            import_file,
            import_catalog_file,
            get_workspace,
            list_files,
            read_log,
            open_folder,
            open_updates,
            read_file,
            write_file,
            delete_item,
            list_recoveries,
            restore_item
        ])
        .build()
}

#[tauri::command]
pub async fn read_file(root_id: String, path: String) -> Result<Document> {
    theseus::bedrock::read_file(root_id, path).await
}
#[tauri::command]
pub async fn write_file(
    root_id: String,
    path: String,
    text: String,
    revision: String,
) -> Result<Document> {
    theseus::bedrock::write_file(root_id, path, text, revision).await
}
#[tauri::command]
pub async fn delete_item(root_id: String, path: String) -> Result<Recovery> {
    theseus::bedrock::delete_item(root_id, path).await
}
#[tauri::command]
pub async fn list_recoveries() -> Result<Vec<Recovery>> {
    theseus::bedrock::list_recoveries().await
}
#[tauri::command]
pub async fn restore_item(root_id: String, id: String) -> Result<()> {
    theseus::bedrock::restore_item(root_id, id).await
}

#[tauri::command]
pub async fn get_workspace<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Workspace> {
    let workspace = theseus::bedrock::get_workspace().await?;
    for icon in workspace
        .items
        .iter()
        .filter_map(|item| item.icon_path.as_ref())
    {
        app.asset_protocol_scope()
            .allow_file(icon)
            .map_err(|error| BedrockError {
                code: ErrorCode::DataUnavailable,
                message: error.to_string(),
            })?;
    }
    Ok(workspace)
}
#[tauri::command]
pub async fn list_files(
    root_id: String,
    path: String,
) -> Result<DirectoryListing> {
    theseus::bedrock::list_files(root_id, path).await
}
#[tauri::command]
pub async fn read_log(root_id: String, path: String) -> Result<LogText> {
    theseus::bedrock::read_log(root_id, path).await
}
#[tauri::command]
pub async fn open_folder<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    root_id: String,
    path: String,
) -> Result<()> {
    let path = theseus::bedrock::folder_path(root_id, path).await?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|error| BedrockError {
            code: ErrorCode::DataUnavailable,
            message: error.to_string(),
        })
}
#[tauri::command]
pub async fn open_updates() -> Result<()> {
    theseus::bedrock::open_updates().await
}

#[tauri::command]
pub async fn get_status() -> Result<BedrockStatus> {
    theseus::bedrock::get_status().await
}

#[tauri::command]
pub async fn launch_game() -> Result<()> {
    theseus::bedrock::launch_game().await
}

#[tauri::command]
pub async fn stop_game() -> Result<()> {
    theseus::bedrock::stop_game().await
}

#[tauri::command]
pub async fn launch_official_launcher() -> Result<()> {
    theseus::bedrock::launch_official_launcher().await
}

#[tauri::command]
pub async fn open_store() -> Result<()> {
    theseus::bedrock::open_store().await
}

#[tauri::command]
pub async fn import_file(path: std::path::PathBuf) -> Result<()> {
    theseus::bedrock::import_file(path).await
}

#[tauri::command]
pub async fn import_catalog_file(
    project_id: u32,
    file_id: u32,
    path: std::path::PathBuf,
    target: Option<WorldTarget>,
) -> Result<usize> {
    theseus::bedrock::import_catalog_file(project_id, file_id, path, target)
        .await
}
