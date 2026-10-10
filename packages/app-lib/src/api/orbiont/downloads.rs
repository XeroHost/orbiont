//! CurseForge file transport: complete downloads via the facade, never automatic redirects.
use super::{
    curseforge_api, curseforge_api_url, is_local_dev_url,
    send_curseforge_response,
};
use crate::util::fetch::{self, DownloadedFile, FetchProgressFn};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Duration,
};

static DOWNLOAD_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .https_only(!is_local_dev_url(curseforge_api_url()))
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(60))
        .user_agent(crate::launcher_user_agent())
        .build()
        .expect("valid download client")
});
static DOWNLOAD_SLOTS: tokio::sync::Semaphore =
    tokio::sync::Semaphore::const_new(2);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownload {
    pub project_id: u32,
    pub file_id: u32,
    pub file_name: String,
    pub file_size: u64,
    pub sha1: String,
    pub page_url: Option<String>,
}

#[derive(Deserialize)]
struct CfFile {
    id: u32,
    #[serde(rename = "modId")]
    mod_id: u32,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileLength")]
    file_length: u64,
    hashes: Vec<CfHash>,
}
#[derive(Deserialize)]
struct CfHash {
    value: String,
    algo: u8,
}

fn input(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::InputError(message.into()).into()
}

pub(crate) fn download_url(project: u32, file: u32) -> crate::Result<String> {
    if project == 0 || file == 0 {
        return Err(input("Invalid CurseForge project/file ID"));
    }
    Ok(format!(
        "{}/mods/{project}/files/{file}/download",
        curseforge_api_url().trim_end_matches('/')
    ))
}

/// Match the configured origin and complete path, never a prefix or a manifest-supplied CDN.
pub(crate) fn download_ids(url: &str) -> Option<(u32, u32)> {
    let base = reqwest::Url::parse(curseforge_api_url()).ok()?;
    let url = reqwest::Url::parse(url).ok()?;
    if url.origin() != base.origin()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let root = format!("{}/mods/", base.path().trim_end_matches('/'));
    let parts: Vec<_> = url.path().strip_prefix(&root)?.split('/').collect();
    if parts.len() != 4 || parts[1] != "files" || parts[3] != "download" {
        return None;
    }
    if !parts[0].bytes().all(|b| b.is_ascii_digit())
        || !parts[2].bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let project = parts[0].parse().ok()?;
    let file = parts[2].parse().ok()?;
    (project != 0 && file != 0).then_some((project, file))
}

pub(crate) fn is_facade_url(url: &str) -> bool {
    reqwest::Url::parse(curseforge_api_url())
        .ok()
        .zip(reqwest::Url::parse(url).ok())
        .is_some_and(|(base, url)| base.origin() == url.origin())
}

pub(crate) fn official_page(url: &str, file_id: u32) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    (parsed.scheme() == "https"
        && parsed.host_str() == Some("www.curseforge.com")
        && parsed.port().is_none()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && (parsed.path().starts_with("/minecraft/")
            || parsed.path().starts_with("/minecraft-bedrock/"))
        && parsed.path().ends_with(&format!("/files/{file_id}")))
    .then(|| parsed.to_string())
}

async fn metadata(
    project_id: u32,
    file_id: u32,
) -> crate::Result<ManualDownload> {
    download_url(project_id, file_id)?;
    let response =
        curseforge_api(&format!("mods/{project_id}/files/{file_id}"), &[])
            .await?;
    let file: CfFile = serde_json::from_value(response["data"].clone())?;
    if file.id != file_id || file.mod_id != project_id {
        return Err(input("CurseForge file/project mismatch"));
    }
    let sha1 = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .ok_or_else(|| input("CurseForge file has no SHA-1"))?
        .value
        .to_ascii_lowercase();
    crate::state::content_store::validate_digest(&sha1, 40)?;
    if file.file_name.is_empty()
        || file.file_name.contains(['/', '\\'])
        || matches!(file.file_name.as_str(), "." | "..")
    {
        return Err(input("Invalid CurseForge filename"));
    }
    Ok(ManualDownload {
        project_id,
        file_id,
        file_name: file.file_name,
        file_size: file.file_length,
        sha1,
        page_url: None,
    })
}

pub(crate) fn manual_error(file: &ManualDownload) -> crate::Error {
    input(format!(
        "CURSEFORGE_MANUAL_DOWNLOAD:{}",
        serde_json::to_string(file).expect("serializable metadata")
    ))
}

fn cache_path(state: &crate::State, file: &ManualDownload) -> PathBuf {
    // IDs and a verified digest determine the directory; filename is reduced to safe ASCII.
    let name: String = file
        .file_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    state
        .directories
        .caches_dir()
        .join("orbiont-curseforge-files")
        .join(file.project_id.to_string())
        .join(file.file_id.to_string())
        .join(&file.sha1)
        .join(name.trim_start_matches('.'))
}

async fn cached(
    file: &ManualDownload,
    staging: &Path,
) -> crate::Result<Option<DownloadedFile>> {
    let Some(state) = crate::State::get_if_initialized() else {
        return Ok(None);
    };
    let path = cache_path(&state, file);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(None);
    }
    // Recheck local bytes. A damaged cache is never installed.
    Ok(
        fetch::stage_curseforge_file(
            &path,
            &file.sha1,
            file.file_size,
            staging,
        )
        .await
        .ok(),
    )
}

async fn fetch_download(
    url: &str,
    file: &ManualDownload,
    client: &reqwest::Client,
    progress: Option<&mut FetchProgressFn<'_>>,
    staging: &Path,
) -> crate::Result<DownloadedFile> {
    let _slot =
        crate::install::control::download_step(DOWNLOAD_SLOTS.acquire())
            .await??;
    let response =
        send_curseforge_response(client.get(url), "file download").await?;
    if response.status() == reqwest::StatusCode::SEE_OTHER {
        let page = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|url| official_page(url, file.file_id))
            .ok_or_else(|| {
                input("Invalid CurseForge manual-download destination")
            })?;
        let mut manual = file.clone();
        manual.page_url = Some(page);
        return Err(manual_error(&manual));
    }
    fetch::read_curseforge_file_response(
        response,
        &file.sha1,
        file.file_size,
        progress,
        Some(staging),
    )
    .await
}

pub(crate) async fn download_pack_file(
    project_id: u32,
    file_id: u32,
    file_name: &str,
    sha1: &str,
    size: u64,
    progress: Option<&mut FetchProgressFn<'_>>,
    staging: &Path,
) -> crate::Result<DownloadedFile> {
    let sha1 = sha1.to_ascii_lowercase();
    crate::state::content_store::validate_digest(&sha1, 40)?;
    let file = ManualDownload {
        project_id,
        file_id,
        file_name: file_name.into(),
        file_size: size,
        sha1,
        page_url: None,
    };
    if let Some(cached) = cached(&file, staging).await? {
        return Ok(cached);
    }
    fetch_download(
        &download_url(project_id, file_id)?,
        &file,
        &DOWNLOAD_CLIENT,
        progress,
        staging,
    )
    .await
}

pub async fn download_file(
    project_id: u32,
    file_id: u32,
) -> crate::Result<PathBuf> {
    let file = metadata(project_id, file_id).await?;
    let state = crate::State::get().await?;
    let staging = state.directories.store_staging_dir();
    let path = cache_path(&state, &file);
    if cached(&file, &staging).await?.is_some() {
        return Ok(path);
    }
    let downloaded = fetch_download(
        &download_url(project_id, file_id)?,
        &file,
        &DOWNLOAD_CLIENT,
        None,
        &staging,
    )
    .await?;
    downloaded.copy_to(&path, &state.io_semaphore).await?;
    Ok(path)
}

/// A browser-downloaded file can satisfy a restricted dependency only after API hash/size verification.
pub async fn import_manual_file(
    project_id: u32,
    file_id: u32,
    source: &Path,
) -> crate::Result<PathBuf> {
    let file = metadata(project_id, file_id).await?;
    let state = crate::State::get().await?;
    let downloaded = fetch::stage_curseforge_file(
        source,
        &file.sha1,
        file.file_size,
        &state.directories.store_staging_dir(),
    )
    .await?;
    let path = cache_path(&state, &file);
    downloaded.copy_to(&path, &state.io_semaphore).await?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn file() -> ManualDownload {
        ManualDownload {
            project_id: 10,
            file_id: 100,
            file_name: "mod.jar".into(),
            file_size: 3,
            sha1: "a9993e364706816aba3e25717850c26c9cd0d89d".into(),
            page_url: None,
        }
    }

    async fn server(
        response: Vec<u8>,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!(
            "http://{}/mods/10/files/100/download",
            listener.local_addr().unwrap()
        );
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut headers = Vec::new();
            while !headers.windows(4).any(|value| value == b"\r\n\r\n") {
                let mut bytes = [0; 2048];
                let amount = socket.read(&mut bytes).await.unwrap();
                assert!(amount > 0);
                headers.extend_from_slice(&bytes[..amount]);
            }
            socket.write_all(&response).await.unwrap();
            String::from_utf8(headers).unwrap()
        });
        (url, task)
    }

    #[test]
    fn only_exact_facade_routes_and_official_file_pages_are_accepted() {
        let url = download_url(10, 100).unwrap();
        assert_eq!(download_ids(&url), Some((10, 100)));
        for value in [
            format!("{url}?url=evil"),
            format!("{url}#evil"),
            format!("{url}/other"),
            url.replace("/files/100/", "/files/0/"),
        ] {
            assert!(download_ids(&value).is_none());
        }
        assert!(
            download_ids("https://edge.forgecdn.net/files/100/mod.jar")
                .is_none()
        );
        let official =
            "https://www.curseforge.com/minecraft/mc-mods/mod/files/100";
        assert_eq!(official_page(official, 100).as_deref(), Some(official));
        let bedrock = "https://www.curseforge.com/minecraft-bedrock/addons/pack/files/100";
        assert_eq!(official_page(bedrock, 100).as_deref(), Some(bedrock));
        for value in [
            official
                .replace("www.curseforge.com", "www.curseforge.com.evil.test"),
            format!("{official}?url=evil"),
            official.replace("/100", "/101"),
            official.replace("https:", "http:"),
        ] {
            assert!(official_page(&value, 100).is_none());
        }
    }

    #[tokio::test]
    async fn complete_downloads_are_verified_and_failed_streams_leave_no_files()
    {
        let _rate = super::super::RATE_TEST_LOCK.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        for (headers, body, success) in [
            (
                "Content-Type: application/java-archive\r\nContent-Length: 3",
                "abc",
                true,
            ),
            (
                "Content-Type: application/java-archive\r\nContent-Length: 3",
                "xyz",
                false,
            ),
            ("Content-Type: text/html\r\nContent-Length: 3", "abc", false),
            (
                "Content-Type: application/zip\r\nContent-Length: 4",
                "abcd",
                false,
            ),
            (
                "Content-Type: application/zip\r\nContent-Length: 3",
                "ab",
                false,
            ),
        ] {
            *super::super::NEXT_REQUEST.lock().await = None;
            let (url, server) = server(format!("HTTP/1.1 200 OK\r\n{headers}\r\nConnection: close\r\n\r\n{body}").into_bytes()).await;
            let result =
                fetch_download(&url, &file(), &client, None, dir.path()).await;
            assert_eq!(
                result.is_ok(),
                success,
                "{headers} / {body}: {result:?}"
            );
            if let Ok(download) = result {
                assert_eq!(
                    tokio::fs::read(download.path()).await.unwrap(),
                    b"abc"
                );
            }
            let request = server.await.unwrap();
            assert!(!request.to_lowercase().contains("x-api-key"));
            assert!(!request.to_lowercase().contains("range:"));
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn restricted_files_return_the_official_page_without_following_redirects()
     {
        let _rate = super::super::RATE_TEST_LOCK.lock().await;
        let dir = tempfile::tempdir().unwrap();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        for location in [
            "https://www.curseforge.com/minecraft/mc-mods/mod/files/100",
            "https://evil.test/files/100",
        ] {
            *super::super::NEXT_REQUEST.lock().await = None;
            let (url, server) = server(format!("HTTP/1.1 303 See Other\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()).await;
            let error =
                fetch_download(&url, &file(), &client, None, dir.path())
                    .await
                    .unwrap_err();
            assert_eq!(
                error.to_string().contains("CURSEFORGE_MANUAL_DOWNLOAD:"),
                location.contains("www.curseforge.com")
            );
            server.await.unwrap();
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
    }

    #[tokio::test]
    async fn manual_files_must_match_both_hash_and_size_and_are_staged_separately()
     {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("staging");
        std::fs::create_dir(&staging).unwrap();
        let source = dir.path().join("download.jar");
        for (bytes, valid) in [
            (b"xyz".as_slice(), false),
            (b"ab".as_slice(), false),
            (b"abc".as_slice(), true),
        ] {
            std::fs::write(&source, bytes).unwrap();
            let result = fetch::stage_curseforge_file(
                &source,
                &file().sha1,
                3,
                &staging,
            )
            .await;
            assert_eq!(result.is_ok(), valid);
            if let Ok(download) = result {
                assert_ne!(download.path(), source);
                assert_eq!(std::fs::read(download.path()).unwrap(), b"abc");
            }
            assert_eq!(std::fs::read_dir(&staging).unwrap().count(), 0);
            assert_eq!(std::fs::read(&source).unwrap(), bytes);
        }
    }

    #[tokio::test]
    #[ignore = "requires the deployed facade and consumes real provider quota"]
    async fn deployed_facade_download_and_restricted_file_match_the_contract() {
        let dir = tempfile::tempdir().unwrap();
        let real = metadata(306612, 9009719).await.unwrap();
        let downloaded = fetch_download(
            &download_url(real.project_id, real.file_id).unwrap(),
            &real,
            &DOWNLOAD_CLIENT,
            None,
            dir.path(),
        )
        .await
        .unwrap();
        assert_eq!(downloaded.size, 2621408);
        assert_eq!(
            fetch::sha1_file_async(downloaded.path()).await.unwrap().1,
            "a0788bb2492cd2737c7eeb9f3e9cd7ae3788d1da"
        );
        let restricted = metadata(1457558, 7602716).await.unwrap();
        let error = fetch_download(
            &download_url(restricted.project_id, restricted.file_id).unwrap(),
            &restricted,
            &DOWNLOAD_CLIENT,
            None,
            dir.path(),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("CURSEFORGE_MANUAL_DOWNLOAD:"));
    }
}
