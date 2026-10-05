//! Read-only skin catalogs. Provider secrets are held by the facade, never by the app.
use crate::{ErrorKind, util::fetch::REQWEST_CLIENT};
use serde::{Deserialize, Serialize};
use std::{sync::LazyLock, time::Duration};

static TEXTURE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .user_agent(crate::launcher_user_agent())
        .build()
        .expect("valid skin texture client")
});

#[derive(Debug, Deserialize, Serialize)]
pub struct CatalogSkin {
    pub id: String,
    pub name: String,
    pub texture: String,
    pub model: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CatalogPage {
    pub provider: String,
    pub skins: Vec<CatalogSkin>,
    pub page: u32,
    pub next: Option<String>,
}

fn texture_allowed(value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    [
        env!("ORBIONT_MINECRAFT_TEXTURE_URL"),
        env!("ORBIONT_MCSTAT_TEXTURE_URL"),
    ]
    .iter()
    .any(|base| {
        let Ok(base) = reqwest::Url::parse(base) else {
            return false;
        };
        url.host_str() == base.host_str()
            && url.path().starts_with(base.path())
            && url.path().bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || b"/_-.".contains(&byte)
            })
    })
}

pub async fn list(
    provider: &str,
    query: &[(String, String)],
) -> crate::Result<CatalogPage> {
    if !["mineskin", "mcstat"].contains(&provider) {
        return Err(
            ErrorKind::OtherError("Unknown skin provider".into()).into()
        );
    }
    let params: Vec<_> = query
        .iter()
        .filter(|(key, value)| {
            ["page", "after", "search", "model", "sort", "tag"]
                .contains(&key.as_str())
                && value.len() <= 120
        })
        .collect();
    let response = REQWEST_CLIENT
        .get(format!(
            "{}/{provider}",
            env!("ORBIONT_SKIN_API_URL").trim_end_matches('/')
        ))
        .query(&params)
        .timeout(Duration::from_secs(20))
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("60")
            .to_owned();
        // Response bodies never become user-visible errors or logs (provider keys stay on the server).
        return Err(ErrorKind::OtherError(format!(
            "Skin catalog {provider}: {}; Retry-After: {retry_after}",
            status.as_u16()
        ))
        .into());
    }
    let mut page: CatalogPage = response.json().await?;
    page.skins.retain(|skin| texture_allowed(&skin.texture));
    page.skins.truncate(24);
    Ok(page)
}

pub async fn texture(url: &str) -> crate::Result<Vec<u8>> {
    if !texture_allowed(url) {
        return Err(ErrorKind::OtherError(
            "Skin texture URL is not allowed".into(),
        )
        .into());
    }
    let mut response =
        TEXTURE_CLIENT.get(url).send().await?.error_for_status()?;
    const MAX_BYTES: usize = 256 * 1024;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES as u64)
    {
        return Err(ErrorKind::InvalidSkinTexture.into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err(ErrorKind::InvalidSkinTexture.into());
        }
        bytes.extend_from_slice(&chunk);
    }
    // Only Minecraft PNG dimensions are accepted, before image decoding or normalization.
    if bytes.len() < 24
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != 64
        || ![32, 64]
            .contains(&u32::from_be_bytes(bytes[20..24].try_into().unwrap()))
    {
        return Err(ErrorKind::InvalidSkinTexture.into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_external_texture_targets_and_credentials() {
        assert!(texture_allowed(&format!(
            "{}abc123",
            env!("ORBIONT_MINECRAFT_TEXTURE_URL")
        )));
        assert!(texture_allowed(&format!(
            "{}published/example.png",
            env!("ORBIONT_MCSTAT_TEXTURE_URL")
        )));
        for url in [
            "http://textures.minecraft.net/texture/abc",
            "https://127.0.0.1/skin.png",
            "https://mcstat.org.evil.test/media/skins/a.png",
            "https://mcstat.org/media/skins/a.png?key=x",
            "https://user:password@mcstat.org/media/skins/a.png",
            "https://mcstat.org/api/v1/skins",
        ] {
            assert!(!texture_allowed(url), "{url}");
        }
    }
}
