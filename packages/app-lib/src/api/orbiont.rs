//! Client for the CurseForge facade, which holds the API key so the launcher never does.

use crate::util::fetch::{INSECURE_REQWEST_CLIENT, REQWEST_CLIENT};
use std::{sync::LazyLock, time::Duration};
use tokio::{sync::Mutex, time::Instant};

pub mod downloads;

// One queue covers catalog queries, batch lookups and native installers.
// This is a per-launcher pace, not a replacement for the VPS's global quota.
const REQUEST_INTERVAL: Duration = Duration::from_millis(1250);
static NEXT_REQUEST: LazyLock<Mutex<Option<Instant>>> =
    LazyLock::new(|| Mutex::new(None));
/// Tests that reset or time NEXT_REQUEST run one at a time.
#[cfg(test)]
pub(crate) static RATE_TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn retry_after_delay(
    value: Option<&str>,
    now: chrono::DateTime<chrono::Utc>,
) -> Duration {
    let seconds = value
        .and_then(|value| {
            value.parse::<u64>().ok().or_else(|| {
                chrono::DateTime::parse_from_rfc2822(value)
                    .ok()
                    .map(|date| {
                        date.signed_duration_since(now).num_seconds().max(0)
                            as u64
                    })
            })
        })
        .unwrap_or(60);
    Duration::from_secs(seconds.max(1))
}

async fn send_curseforge(
    request: reqwest::RequestBuilder,
    path: &str,
) -> crate::Result<serde_json::Value> {
    Ok(send_curseforge_response(request, path)
        .await?
        .json()
        .await?)
}

pub(crate) async fn send_curseforge_response(
    request: reqwest::RequestBuilder,
    path: &str,
) -> crate::Result<reqwest::Response> {
    let mut next =
        crate::install::control::download_step(NEXT_REQUEST.lock()).await?;
    for attempt in 0..2 {
        if let Some(deadline) = *next {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining > Duration::from_secs(120) {
                return Err(crate::ErrorKind::OtherError(format!(
                    "CurseForge request to {path} failed: 429 Too Many Requests; Retry-After: {} seconds",
                    remaining.as_secs() + 1
                )).into());
            }
            crate::install::control::download_step(tokio::time::sleep_until(
                deadline,
            ))
            .await?;
        }
        *next = Some(Instant::now() + REQUEST_INTERVAL);
        let response = crate::install::control::download_step(
            request
                .try_clone()
                .ok_or_else(|| {
                    crate::ErrorKind::OtherError(
                        "Cannot clone CurseForge lookup".into(),
                    )
                })?
                .send(),
        )
        .await??;
        let status = response.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let delay = retry_after_delay(
                response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|header| header.to_str().ok()),
                chrono::Utc::now(),
            );
            *next = Some(Instant::now() + delay.max(REQUEST_INTERVAL));
            // Respect long server cooldowns without keeping this action pending indefinitely.
            if attempt == 0 && delay <= Duration::from_secs(120) {
                continue;
            }
            return Err(crate::ErrorKind::OtherError(format!(
                "CurseForge request to {path} failed: 429 Too Many Requests; Retry-After: {} seconds",
                delay.as_secs()
            )).into());
        }
        if !status.is_success() && status != reqwest::StatusCode::SEE_OTHER {
            return Err(crate::ErrorKind::OtherError(format!(
                "CurseForge request to {path} failed: {status}"
            ))
            .into());
        }
        return Ok(response);
    }
    unreachable!()
}

/// Product name, for text the Tauri shell shows before the UI is up.
pub const PRODUCT_NAME: &str = env!("ORBIONT_PRODUCT_NAME");
/// Support address, for the same kind of early error messages.
pub const SUPPORT_EMAIL: &str = env!("ORBIONT_SUPPORT_EMAIL");
/// Deep-link scheme the app registers (`orbiont://...`).
pub const DEEP_LINK_SCHEME: &str = env!("ORBIONT_DEEP_LINK_SCHEME");

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

/// GET {ORBIONT_CURSEFORGE_API_URL}/{path} — the facade's allowlisted, read-only
/// pass-through to the CurseForge API. Returns CurseForge's JSON untouched;
/// the frontend maps it onto the launcher's native data model. `path` is
/// relative to the CurseForge API root (e.g. "mods/search"); the facade
/// rejects anything off its allowlist.
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

    send_curseforge(client_for(url.as_str()).get(url), path).await
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
    send_curseforge(client_for(&url).post(url).json(body), path).await
}

/// Downloads a verified file through the facade. API keys stay on the VPS.
pub async fn download_curseforge_file(
    mod_id: u32,
    file_id: u32,
) -> crate::Result<std::path::PathBuf> {
    downloads::download_file(mod_id, file_id).await
}

#[cfg(test)]
mod rate_limit_tests {
    use super::*;

    #[test]
    fn accepts_seconds_and_http_dates_without_shortening_cooldown() {
        let now = chrono::DateTime::parse_from_rfc2822(
            "Thu, 01 Oct 2026 12:00:00 GMT",
        )
        .unwrap()
        .with_timezone(&chrono::Utc);
        assert_eq!(retry_after_delay(Some("17"), now), Duration::from_secs(17));
        assert_eq!(
            retry_after_delay(Some("Thu, 01 Oct 2026 12:01:30 GMT"), now),
            Duration::from_secs(90)
        );
        assert_eq!(
            retry_after_delay(Some("3600"), now),
            Duration::from_secs(3600)
        );
        assert_eq!(retry_after_delay(Some("0"), now), Duration::from_secs(1));
        assert_eq!(
            retry_after_delay(Some("invalid"), now),
            Duration::from_secs(60)
        );
        assert_eq!(retry_after_delay(None, now), Duration::from_secs(60));
    }

    #[tokio::test]
    async fn requests_share_cooldown_and_retry_only_once() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let _rate = RATE_TEST_LOCK.lock().await;
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let mut received = Vec::new();
            for status in [429, 200, 200, 429, 429] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut bytes = [0; 2048];
                let mut headers = Vec::new();
                while !headers.windows(4).any(|value| value == b"\r\n\r\n") {
                    let amount = socket.read(&mut bytes).await.unwrap();
                    assert!(amount > 0, "request closed before HTTP headers");
                    headers.extend_from_slice(&bytes[..amount]);
                    assert!(headers.len() <= 16 * 1024);
                }
                received.push(Instant::now());
                socket.write_all(format!("HTTP/1.1 {status} Response\r\nRetry-After: 1\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").as_bytes()).await.unwrap();
            }
            received
        });
        let first = send_curseforge(client_for(&url).get(&url), "test");
        let second = send_curseforge(
            client_for(&url).post(&url).json(&serde_json::json!({})),
            "test",
        );
        let (first, second) = tokio::join!(first, second);
        assert!(first.is_ok() && second.is_ok());
        let error = send_curseforge(client_for(&url).get(&url), "test")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("429"));
        let times = server.await.unwrap();
        for pair in times.windows(2) {
            assert!(
                pair[1].duration_since(pair[0]) >= Duration::from_millis(1200)
            );
        }
        *NEXT_REQUEST.lock().await =
            Some(Instant::now() + Duration::from_secs(3600));
        // A new action during a long cooldown returns promptly without sending HTTP.
        let blocked = tokio::time::timeout(
            Duration::from_millis(100),
            send_curseforge(client_for(&url).get(&url), "test"),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(blocked.to_string().contains("429"));
        *NEXT_REQUEST.lock().await = None;
    }
}
