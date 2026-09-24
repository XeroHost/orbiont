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

/// POST /v1/curseforge/api/{path} — the catalog's batch lookups
/// (`mods/files` with `fileIds`, `mods` with `modIds`), used to resolve a
/// whole modpack manifest in a couple of requests.
pub async fn curseforge_api_post(
    path: &str,
    body: &serde_json::Value,
) -> crate::Result<serde_json::Value> {
    if !matches!(path, "mods/files" | "mods") {
        return Err(crate::ErrorKind::InputError(format!(
            "Unsupported CurseForge batch path: {path}"
        ))
        .into());
    }

    let url = format!("{}/v1/curseforge/api/{path}", base_url());
    let response = INSECURE_REQWEST_CLIENT.post(url).json(body).send().await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "CurseForge batch request to {path} failed: {}",
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
