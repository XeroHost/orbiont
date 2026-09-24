//! Client for the Orbiont catalog backend (orbiont-catalog, a separate
//! service — see the build plan, Fase 3). Serves the modpack catalog, the
//! XeroHost server list, and a unified search proxy over three sources.

// The catalog's base URL is compile-time env config (ORBIONT_CATALOG_BASE_URL),
// not user input, and is plain http:// in local dev (https:// in prod) — so
// the shared REQWEST_CLIENT's `https_only(true)` would reject every request
// here in dev. INSECURE_REQWEST_CLIENT only lifts that scheme restriction;
// it doesn't weaken TLS/cert verification for the https:// case.
use crate::util::fetch::{self, INSECURE_REQWEST_CLIENT};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

fn base_url() -> &'static str {
    env!("ORBIONT_CATALOG_BASE_URL")
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSource {
    Xerohost,
    Modrinth,
    Curseforge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Modpack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub game_version: String,
    pub loader: String,
    pub download_url: String,
    pub hash: String,
    pub source: CatalogSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrbiontServer {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub icon: String,
    pub tags: Vec<String>,
    pub featured: bool,
    pub modpack_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub id: String,
    pub source: CatalogSource,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub downloads: Option<u64>,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub download_url: Option<String>,
    pub allow_mod_distribution: Option<bool>,
    pub page_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeCategory {
    pub id: u32,
    pub name: String,
    pub slug: String,
    pub icon_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CurseforgeCategoriesResponse {
    categories: Vec<CurseforgeCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeScreenshot {
    pub url: String,
    pub thumbnail_url: String,
    pub title: String,
}

/// Full detail for a single CurseForge mod/modpack — everything a search
/// result doesn't carry (full description, screenshots, authors), for the
/// search UI's "view content" detail view.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurseforgeModDetail {
    pub id: String,
    pub source: CatalogSource,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub icon: Option<String>,
    pub downloads: Option<u64>,
    pub game_versions: Vec<String>,
    pub categories: Vec<String>,
    pub authors: Vec<String>,
    pub screenshots: Vec<CurseforgeScreenshot>,
    pub download_url: Option<String>,
    pub allow_mod_distribution: Option<bool>,
    pub page_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchError {
    pub source: CatalogSource,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub index: u32,
    pub page_size: u32,
    pub total_count: Option<u64>,
    #[serde(default)]
    pub errors: Vec<SearchError>,
}

const CACHE_TTL: Duration = Duration::from_secs(60);

struct CacheEntry<T> {
    value: T,
    etag: Option<String>,
    fetched_at: Instant,
}

static MODPACKS_CACHE: LazyLock<RwLock<Option<CacheEntry<Vec<Modpack>>>>> =
    LazyLock::new(|| RwLock::new(None));
static SERVERS_CACHE: LazyLock<RwLock<Option<CacheEntry<Vec<OrbiontServer>>>>> =
    LazyLock::new(|| RwLock::new(None));
// Categories barely ever change, so this is keyed by project type ("modpack"
// / "mod") rather than sharing a single slot like the caches above.
const CATEGORIES_CACHE_TTL: Duration = Duration::from_secs(60 * 60);
static CURSEFORGE_CATEGORIES_CACHE: LazyLock<
    RwLock<HashMap<String, (Vec<CurseforgeCategory>, Instant)>>,
> = LazyLock::new(|| RwLock::new(HashMap::new()));

async fn fetch_cached<T>(
    path: &str,
    cache: &RwLock<Option<CacheEntry<T>>>,
) -> crate::Result<T>
where
    T: for<'de> Deserialize<'de> + Clone,
{
    {
        let guard = cache.read().await;
        if let Some(entry) = guard.as_ref()
            && entry.fetched_at.elapsed() < CACHE_TTL
        {
            return Ok(entry.value.clone());
        }
    }

    let existing_etag = {
        let guard = cache.read().await;
        guard.as_ref().and_then(|entry| entry.etag.clone())
    };

    let url = format!("{}{path}", base_url());
    let mut request = INSECURE_REQWEST_CLIENT.get(&url);
    if let Some(etag) = &existing_etag {
        request = request.header("If-None-Match", etag);
    }

    let response = request.send().await?;

    if response.status() == StatusCode::NOT_MODIFIED {
        let mut guard = cache.write().await;
        if let Some(entry) = guard.as_mut() {
            entry.fetched_at = Instant::now();
            return Ok(entry.value.clone());
        }
        // We sent an If-None-Match without a cached value, which shouldn't
        // happen; fall through and re-fetch unconditionally below.
    }

    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "Orbiont catalog request to {path} failed: {}",
            response.status()
        ))
        .into());
    }

    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let value: T = response.json().await?;

    *cache.write().await = Some(CacheEntry {
        value: value.clone(),
        etag,
        fetched_at: Instant::now(),
    });

    Ok(value)
}

/// GET /v1/modpacks, cached in-memory for [`CACHE_TTL`] and revalidated with
/// the backend's ETag afterwards.
pub async fn get_modpacks() -> crate::Result<Vec<Modpack>> {
    fetch_cached("/v1/modpacks", &MODPACKS_CACHE).await
}

/// GET /v1/servers, cached the same way as [`get_modpacks`].
pub async fn get_servers() -> crate::Result<Vec<OrbiontServer>> {
    fetch_cached("/v1/servers", &SERVERS_CACHE).await
}

/// GET /v1/search — not cached, since results are paginated and dynamic.
#[allow(clippy::too_many_arguments)]
pub async fn search(
    query: Option<&str>,
    source: Option<CatalogSource>,
    game_version: Option<&str>,
    project_type: Option<&str>,
    category_id: Option<u32>,
    mod_loader_type: Option<&str>,
    index: u32,
    page_size: u32,
) -> crate::Result<SearchResponse> {
    let mut url = reqwest::Url::parse(&format!("{}/v1/search", base_url()))
        .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))?;
    {
        let mut pairs = url.query_pairs_mut();
        if let Some(query) = query {
            pairs.append_pair("query", query);
        }
        if let Some(source) = source {
            let source_str = match source {
                CatalogSource::Xerohost => "xerohost",
                CatalogSource::Modrinth => "modrinth",
                CatalogSource::Curseforge => "curseforge",
            };
            pairs.append_pair("source", source_str);
        }
        if let Some(game_version) = game_version {
            pairs.append_pair("gameVersion", game_version);
        }
        if let Some(project_type) = project_type {
            pairs.append_pair("projectType", project_type);
        }
        if let Some(category_id) = category_id {
            pairs.append_pair("categoryId", &category_id.to_string());
        }
        if let Some(mod_loader_type) = mod_loader_type {
            pairs.append_pair("modLoaderType", mod_loader_type);
        }
        pairs.append_pair("index", &index.to_string());
        pairs.append_pair("pageSize", &page_size.to_string());
    }

    let response = INSECURE_REQWEST_CLIENT.get(url).send().await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "Orbiont catalog search failed: {}",
            response.status()
        ))
        .into());
    }

    Ok(response.json().await?)
}

/// GET /v1/curseforge/categories, cached in-memory per project type for
/// [`CATEGORIES_CACHE_TTL`] — CurseForge's own category taxonomy changes
/// rarely enough that a short-lived ETag revalidation like the other caches
/// isn't worth the extra round-trip.
pub async fn get_curseforge_categories(
    project_type: &str,
) -> crate::Result<Vec<CurseforgeCategory>> {
    {
        let cache = CURSEFORGE_CATEGORIES_CACHE.read().await;
        if let Some((categories, fetched_at)) = cache.get(project_type)
            && fetched_at.elapsed() < CATEGORIES_CACHE_TTL
        {
            return Ok(categories.clone());
        }
    }

    let mut url = reqwest::Url::parse(&format!(
        "{}/v1/curseforge/categories",
        base_url()
    ))
    .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("projectType", project_type);

    let response = INSECURE_REQWEST_CLIENT.get(url).send().await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "Orbiont catalog CurseForge categories request failed: {}",
            response.status()
        ))
        .into());
    }

    let body: CurseforgeCategoriesResponse = response.json().await?;

    CURSEFORGE_CATEGORIES_CACHE.write().await.insert(
        project_type.to_owned(),
        (body.categories.clone(), Instant::now()),
    );

    Ok(body.categories)
}

/// GET /v1/curseforge/mods/:modId — not cached, since this is only fetched
/// on-demand when the user opens a search result's detail view.
/// `mod_id` is the numeric CurseForge id (strip the "curseforge:" prefix
/// from a [`SearchResult::id`] before calling this).
pub async fn get_curseforge_mod_detail(
    mod_id: &str,
) -> crate::Result<CurseforgeModDetail> {
    let url = format!("{}/v1/curseforge/mods/{mod_id}", base_url());

    let response = INSECURE_REQWEST_CLIENT.get(url).send().await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "Orbiont catalog CurseForge mod detail request failed: {}",
            response.status()
        ))
        .into());
    }

    Ok(response.json().await?)
}

/// GET /v1/curseforge/api/{path} — the catalog's allowlisted, read-only
/// pass-through to the CurseForge API. Returns CurseForge's JSON untouched;
/// the frontend maps it onto the launcher's native data model, so CurseForge
/// stays a data source only. `path` is relative to the CurseForge API root
/// (e.g. "mods/search"); the catalog rejects anything off its allowlist.
pub async fn curseforge_api(
    path: &str,
    query: &[(String, String)],
) -> crate::Result<serde_json::Value> {
    let path = path.trim_start_matches('/');
    if path.is_empty()
        || !path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
        || path.contains("//")
    {
        return Err(crate::ErrorKind::InputError(format!(
            "Invalid CurseForge API path: {path}"
        ))
        .into());
    }

    let mut url = reqwest::Url::parse(&format!(
        "{}/v1/curseforge/api/{path}",
        base_url()
    ))
    .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))?;
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }

    let response = INSECURE_REQWEST_CLIENT.get(url).send().await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "CurseForge request to {path} failed: {}",
            response.status()
        ))
        .into());
    }

    Ok(response.json().await?)
}

/// Downloads a catalog modpack's .mrpack to a stable cache path (named by
/// modpack id, so repeat installs across servers that share a modpack don't
/// re-download it), verifying it against the catalog's declared hash.
/// Returns the local path, ready to hand to `CreatePackLocation::FromFile`.
pub async fn download_modpack_file(
    modpack: &Modpack,
) -> crate::Result<std::path::PathBuf> {
    let state = crate::State::get().await?;
    let dir = state.directories.caches_dir().join("orbiont-modpacks");
    crate::util::io::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.mrpack", sanitize_filename(&modpack.id)));

    let sha1 = modpack.hash.strip_prefix("sha1:");

    let bytes = fetch::fetch_advanced(
        Method::GET,
        &modpack.download_url,
        sha1,
        None,
        None,
        None,
        None,
        Some("orbiont_modpack"),
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    crate::util::io::write(&path, &bytes).await?;

    Ok(path)
}

fn sanitize_filename(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Downloads an arbitrary search-result file (a CurseForge `downloadUrl`, or
/// our own catalog's) to a temp cache path, ready for
/// `CreatePackLocation::FromFile`. Unlike [`download_modpack_file`], there's
/// no hash to verify here — search results don't carry one — so this is only
/// as trustworthy as the URL itself (CurseForge's CDN or our own).
pub async fn download_search_result_file(
    url: &str,
    file_name_hint: &str,
) -> crate::Result<std::path::PathBuf> {
    let state = crate::State::get().await?;
    let dir = state.directories.caches_dir().join("orbiont-search");
    crate::util::io::create_dir_all(&dir).await?;
    let path = dir.join(sanitize_filename(file_name_hint));

    let bytes = fetch::fetch_advanced(
        Method::GET,
        url,
        None,
        None,
        None,
        None,
        None,
        Some("orbiont_search_result"),
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    crate::util::io::write(&path, &bytes).await?;

    Ok(path)
}

/// Saves a manually-downloaded modpack file the user dropped onto the
/// launcher (the fallback for a CurseForge result whose author disabled
/// third-party distribution — see the build plan, Fase 4) to a temp cache
/// path, ready for `CreatePackLocation::FromFile`.
pub async fn save_dropped_file(
    bytes: &[u8],
    file_name_hint: &str,
) -> crate::Result<std::path::PathBuf> {
    let state = crate::State::get().await?;
    let dir = state.directories.caches_dir().join("orbiont-search");
    crate::util::io::create_dir_all(&dir).await?;
    let path = dir.join(sanitize_filename(file_name_hint));
    crate::util::io::write(&path, bytes).await?;
    Ok(path)
}
