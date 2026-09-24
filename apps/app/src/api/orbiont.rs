use crate::api::Result;
use theseus::orbiont::{
    CatalogSource, CurseforgeCategory, CurseforgeModDetail, Modpack,
    OrbiontServer, SearchResponse,
};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("orbiont")
        .invoke_handler(tauri::generate_handler![
            orbiont_get_modpacks,
            orbiont_get_servers,
            orbiont_search,
            orbiont_get_curseforge_categories,
            orbiont_get_curseforge_mod_detail,
            orbiont_curseforge_api,
            orbiont_download_modpack,
            orbiont_download_search_result,
            orbiont_save_dropped_file,
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

/// GET /v1/search — unified proxy over the XeroHost catalog, CurseForge, and
/// Modrinth. `source` is one of "all" (default), "xerohost", "curseforge",
/// "modrinth".
#[tauri::command]
pub async fn orbiont_search(
    query: Option<String>,
    source: Option<String>,
    game_version: Option<String>,
    project_type: Option<String>,
    category_id: Option<u32>,
    mod_loader_type: Option<String>,
    index: Option<u32>,
    page_size: Option<u32>,
) -> Result<SearchResponse> {
    let source = match source.as_deref() {
        None | Some("all") => None,
        Some("xerohost") => Some(CatalogSource::Xerohost),
        Some("curseforge") => Some(CatalogSource::Curseforge),
        Some("modrinth") => Some(CatalogSource::Modrinth),
        Some(other) => {
            return Err(theseus::Error::from(theseus::ErrorKind::InputError(
                format!("Unknown source: {other}"),
            ))
            .into());
        }
    };

    Ok(theseus::orbiont::search(
        query.as_deref(),
        source,
        game_version.as_deref(),
        project_type.as_deref(),
        category_id,
        mod_loader_type.as_deref(),
        index.unwrap_or(0),
        page_size.unwrap_or(20),
    )
    .await?)
}

/// GET /v1/curseforge/categories — CurseForge's own category list for a
/// project type ("modpack" or "mod"), for the search UI's category filter.
#[tauri::command]
pub async fn orbiont_get_curseforge_categories(
    project_type: String,
) -> Result<Vec<CurseforgeCategory>> {
    Ok(theseus::orbiont::get_curseforge_categories(&project_type).await?)
}

/// GET /v1/curseforge/mods/:modId — full detail for a single CurseForge
/// mod/modpack (description, screenshots, authors), for a search result's
/// "view content" detail view. `mod_id` is the numeric CurseForge id, not
/// the "curseforge:123" prefixed `SearchResult.id`.
#[tauri::command]
pub async fn orbiont_get_curseforge_mod_detail(
    mod_id: String,
) -> Result<CurseforgeModDetail> {
    Ok(theseus::orbiont::get_curseforge_mod_detail(&mod_id).await?)
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

/// Saves a modpack file the user dropped onto the launcher to a local cache
/// path, for search results where the author disabled third-party
/// distribution and the user had to download the file manually.
#[tauri::command]
pub async fn orbiont_save_dropped_file(
    bytes: Vec<u8>,
    file_name_hint: String,
) -> Result<String> {
    let path =
        theseus::orbiont::save_dropped_file(&bytes, &file_name_hint).await?;
    Ok(path.to_string_lossy().into_owned())
}
