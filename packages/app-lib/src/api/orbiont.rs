//! Clients for Orbiont's backend (the orbiont-catalog repo, two services):
//! the catalog (XeroHost modpacks and servers) and the CurseForge facade,
//! which holds the CurseForge API key so the launcher never does.

use crate::util::fetch::{self, INSECURE_REQWEST_CLIENT, REQWEST_CLIENT};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Product name, for text the Tauri shell shows before the UI is up.
pub const PRODUCT_NAME: &str = env!("ORBIONT_PRODUCT_NAME");
/// Support address, for the same kind of early error messages.
pub const SUPPORT_EMAIL: &str = env!("ORBIONT_SUPPORT_EMAIL");

fn base_url() -> &'static str {
    env!("ORBIONT_CATALOG_BASE_URL")
}

fn curseforge_api_url() -> &'static str {
    env!("ORBIONT_CURSEFORGE_API_URL")
}

/// Plain `http://` is only accepted for a backend on this machine (local
/// development). Everything else goes through the https-only client.
fn is_local_dev_url(url: &str) -> bool {
    ["http://localhost:", "http://127.0.0.1:", "http://[::1]:"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

fn client_for(url: &str) -> &'static reqwest::Client {
    if is_local_dev_url(url) {
        &INSECURE_REQWEST_CLIENT
    } else {
        &REQWEST_CLIENT
    }
}

/// Where CurseForge serves files from. Download URLs must be https and on one
/// of these hosts, whatever the API says.
const CURSEFORGE_CDN_HOSTS: &[&str] = &[
    "edge.forgecdn.net",
    "mediafilez.forgecdn.net",
    "media.forgecdn.net",
];

pub(crate) fn is_curseforge_cdn_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| {
        url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| CURSEFORGE_CDN_HOSTS.contains(&host))
    })
}

fn is_sha1_hex(value: &str) -> bool {
    value.len() == 40 && value.chars().all(|c| c.is_ascii_hexdigit())
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
    let mut request = client_for(&url).get(&url);
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

/// GET {ORBIONT_CURSEFORGE_API_URL}/{path} — the facade's allowlisted, read-only
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

    let mut url =
        reqwest::Url::parse(&format!("{}/{path}", curseforge_api_url()))
            .map_err(|error| crate::ErrorKind::OtherError(error.to_string()))?;
    if !query.is_empty() {
        url.query_pairs_mut().extend_pairs(query);
    }

    let response = client_for(url.as_str()).get(url).send().await?;
    if !response.status().is_success() {
        return Err(crate::ErrorKind::OtherError(format!(
            "CurseForge request to {path} failed: {}",
            response.status()
        ))
        .into());
    }

    Ok(response.json().await?)
}

/// POST {ORBIONT_CURSEFORGE_API_URL}/{path} — the facade's batch lookups
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

    let url = format!("{}/{path}", curseforge_api_url());
    let response = client_for(&url).post(url).json(body).send().await?;
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
/// modpack id, so repeat installs don't re-download it). The modpack is looked
/// up in the catalog by id — the UI can't hand over its own URL or hash — and
/// it must have a SHA-1 and an https download URL; the file is verified
/// against that hash. Returns the local path for `CreatePackLocation::FromFile`.
pub async fn download_modpack_file(
    modpack_id: &str,
) -> crate::Result<std::path::PathBuf> {
    let modpack = get_modpacks()
        .await?
        .into_iter()
        .find(|modpack| modpack.id == modpack_id)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Modpack {modpack_id} isn't in the catalog"
            ))
        })?;

    let sha1 = modpack
        .hash
        .strip_prefix("sha1:")
        .filter(|hash| is_sha1_hex(hash))
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Modpack {} has no valid SHA-1 in the catalog",
                modpack.name
            ))
        })?;
    if !modpack.download_url.starts_with("https://")
        && !is_local_dev_url(&modpack.download_url)
    {
        return Err(crate::ErrorKind::InputError(format!(
            "Modpack {} must be downloaded over https",
            modpack.name
        ))
        .into());
    }

    let state = crate::State::get().await?;
    let dir = state.directories.caches_dir().join("orbiont-modpacks");
    crate::util::io::create_dir_all(&dir).await?;
    let path = dir.join(format!("{}.mrpack", sanitize_filename(&modpack.id)));

    let bytes = fetch::fetch_advanced(
        Method::GET,
        &modpack.download_url,
        Some(sha1),
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
    let name: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    // No hidden files or "..": the name always stays inside its cache folder.
    name.trim_start_matches('.').to_string()
}

#[derive(Deserialize)]
struct CurseforgeFileResponse {
    data: CurseforgeFile,
}

#[derive(Deserialize)]
struct CurseforgeFile {
    #[serde(rename = "modId")]
    mod_id: u32,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(default)]
    hashes: Vec<CurseforgeFileHash>,
}

#[derive(Deserialize)]
struct CurseforgeFileHash {
    value: String,
    algo: u8,
}

/// Downloads a CurseForge file to a cache path. The file is resolved through
/// the facade by id (the UI can't pass a URL), must come from CurseForge's CDN
/// over https, and is verified against CurseForge's SHA-1.
pub async fn download_curseforge_file(
    mod_id: u32,
    file_id: u32,
) -> crate::Result<std::path::PathBuf> {
    let response =
        curseforge_api(&format!("mods/{mod_id}/files/{file_id}"), &[]).await?;
    let file = serde_json::from_value::<CurseforgeFileResponse>(response)?.data;
    if file.mod_id != mod_id {
        return Err(crate::ErrorKind::InputError(format!(
            "CurseForge file {file_id} doesn't belong to project {mod_id}"
        ))
        .into());
    }

    let url = file.download_url.ok_or_else(|| {
        crate::ErrorKind::InputError(format!(
            "{} can only be downloaded from CurseForge's website",
            file.file_name
        ))
    })?;
    if !is_curseforge_cdn_url(&url) {
        return Err(crate::ErrorKind::InputError(format!(
            "Refusing to download {} from an unexpected host",
            file.file_name
        ))
        .into());
    }
    let sha1 = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1 && is_sha1_hex(&hash.value))
        .map(|hash| hash.value.to_ascii_lowercase())
        .ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "CurseForge gave no SHA-1 for {}",
                file.file_name
            ))
        })?;

    let state = crate::State::get().await?;
    let dir = state
        .directories
        .caches_dir()
        .join("orbiont-curseforge-files")
        .join(file_id.to_string());
    crate::util::io::create_dir_all(&dir).await?;
    let path = dir.join(sanitize_filename(&file.file_name));

    let bytes = fetch::fetch_advanced(
        Method::GET,
        &url,
        Some(&sha1),
        None,
        None,
        None,
        None,
        Some("orbiont_curseforge_file"),
        &state.fetch_semaphore,
        &state.pool,
    )
    .await?;

    crate::util::io::write(&path, &bytes).await?;

    Ok(path)
}
