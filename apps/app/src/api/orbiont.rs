use crate::api::Result;

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("orbiont")
        .invoke_handler(tauri::generate_handler![
            orbiont_curseforge_api,
            orbiont_curseforge_api_post,
            orbiont_convert_curseforge_pack,
            orbiont_download_curseforge_file,
            orbiont_import_curseforge_file,
        ])
        .build()
}

#[tauri::command]
pub async fn orbiont_import_curseforge_file(
    mod_id: u32,
    file_id: u32,
    path: std::path::PathBuf,
) -> Result<String> {
    Ok(
        theseus::orbiont::downloads::import_manual_file(mod_id, file_id, &path)
            .await?
            .to_string_lossy()
            .into_owned(),
    )
}

/// Read-only batch lookups, validated by the core and facade.
#[tauri::command]
pub async fn orbiont_curseforge_api_post(
    path: String,
    body: serde_json::Value,
) -> Result<serde_json::Value> {
    Ok(theseus::orbiont::curseforge_api_post(&path, &body).await?)
}

/// Allowlisted pass-through to the CurseForge facade. Returns raw JSON.
#[tauri::command]
pub async fn orbiont_curseforge_api(
    path: String,
    query: Option<Vec<(String, String)>>,
) -> Result<serde_json::Value> {
    Ok(
        theseus::orbiont::curseforge_api(&path, &query.unwrap_or_default())
            .await?,
    )
}

/// Converts a downloaded CurseForge modpack zip into an `.mrpack` for the
/// native installer (see `theseus::curseforge_pack`). Already-`.mrpack`
/// files come back unchanged.
#[tauri::command]
pub async fn orbiont_convert_curseforge_pack(
    path: std::path::PathBuf,
) -> Result<theseus::curseforge_pack::ConvertedPack> {
    Ok(theseus::curseforge_pack::convert_curseforge_pack(&path).await?)
}

/// Downloads a CurseForge file by id (resolved and verified in the core; see
/// `theseus::orbiont::download_curseforge_file`).
#[tauri::command]
pub async fn orbiont_download_curseforge_file(
    mod_id: u32,
    file_id: u32,
) -> Result<String> {
    let path =
        theseus::orbiont::download_curseforge_file(mod_id, file_id).await?;
    Ok(path.to_string_lossy().into_owned())
}
