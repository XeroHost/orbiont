use crate::api::Result;
use theseus::orbiont::{Modpack, OrbiontServer};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("orbiont")
        .invoke_handler(tauri::generate_handler![
            orbiont_get_modpacks,
            orbiont_get_servers,
            orbiont_curseforge_api,
            orbiont_convert_curseforge_pack,
            orbiont_download_modpack,
            orbiont_download_search_result,
        ])
        .build()
}

/// GET /v1/modpacks from the Orbiont catalog backend.
#[tauri::command]
pub async fn orbiont_get_modpacks() -> Result<Vec<Modpack>> {
    Ok(theseus::orbiont::get_modpacks().await?)
}

/// GET /v1/servers from the Orbiont catalog backend.
#[tauri::command]
pub async fn orbiont_get_servers() -> Result<Vec<OrbiontServer>> {
    Ok(theseus::orbiont::get_servers().await?)
}

/// GET /v1/curseforge/api/{path} — allowlisted pass-through to the CurseForge
/// API via the catalog (which holds the key). Returns CurseForge's raw JSON.
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

/// Downloads a catalog modpack's .mrpack to a local cache path, ready to
/// install via `install_create_modpack_instance` with a `fromFile` location.
#[tauri::command]
pub async fn orbiont_download_modpack(modpack: Modpack) -> Result<String> {
    let path = theseus::orbiont::download_modpack_file(&modpack).await?;
    Ok(path.to_string_lossy().into_owned())
}

/// Downloads a search result's file (e.g. a CurseForge `downloadUrl`) to a
/// local cache path, for results that don't come from our own catalog and so
/// have no hash to verify against.
#[tauri::command]
pub async fn orbiont_download_search_result(
    url: String,
    file_name_hint: String,
) -> Result<String> {
    let path =
        theseus::orbiont::download_search_result_file(&url, &file_name_hint)
            .await?;
    Ok(path.to_string_lossy().into_owned())
}
