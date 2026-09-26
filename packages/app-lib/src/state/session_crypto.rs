//! Encryption at rest for the Microsoft/Minecraft session tokens stored in
//! `minecraft_users` (AES-256-GCM).
//!
//! The key lives in the operating system's credential store (Windows
//! Credential Manager, macOS Keychain, Secret Service on Linux), so copying
//! `app.db` alone doesn't give anyone a usable session. When no credential
//! store is available (e.g. a Linux desktop without Secret Service), the key
//! falls back to a file next to the database; that still keeps tokens out of
//! the database itself, but is only as private as the user's profile folder.
//!
//! Stored values carry a version prefix. Values without it are sessions saved
//! before this existed: they're read as-is and encrypted on the next save.

use base64::Engine;
use base64::prelude::BASE64_STANDARD;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
use ring::rand::{SecureRandom, SystemRandom};
use std::path::PathBuf;
use tokio::sync::OnceCell;

const PREFIX: &str = "enc:v1:";
const KEY_LEN: usize = 32;
const KEYRING_USER: &str = "session-key";
const FALLBACK_KEY_FILE: &str = "session.key";

static CIPHER: OnceCell<LessSafeKey> = OnceCell::const_new();

fn error(message: impl Into<String>) -> crate::Error {
    crate::ErrorKind::OtherError(format!(
        "Session encryption: {}",
        message.into()
    ))
    .into()
}

fn keyring_entry() -> Option<keyring::Entry> {
    keyring::Entry::new(env!("ORBIONT_PRODUCT_NAME"), KEYRING_USER).ok()
}

fn fallback_key_path() -> crate::Result<PathBuf> {
    let dirs = crate::state::DirectoryInfo::global_handle_if_ready()
        .ok_or_else(|| error("app directories aren't ready"))?;
    Ok(dirs.settings_dir.join(FALLBACK_KEY_FILE))
}

fn decode_key(encoded: &str) -> Option<[u8; KEY_LEN]> {
    BASE64_STANDARD.decode(encoded.trim()).ok()?.try_into().ok()
}

fn new_key() -> crate::Result<[u8; KEY_LEN]> {
    let mut key = [0u8; KEY_LEN];
    SystemRandom::new()
        .fill(&mut key)
        .map_err(|_| error("no secure random source"))?;
    Ok(key)
}

async fn load_or_create_key() -> crate::Result<[u8; KEY_LEN]> {
    // 1. OS credential store.
    if let Some(entry) = keyring_entry() {
        match entry.get_password() {
            Ok(encoded) => {
                if let Some(key) = decode_key(&encoded) {
                    return Ok(key);
                }
                tracing::warn!("Stored session key is malformed; replacing it");
            }
            Err(keyring::Error::NoEntry) => {}
            Err(err) => {
                tracing::warn!(
                    "OS credential store unavailable ({err}); using the fallback key file"
                );
            }
        }
    }

    // 2. A key already in the fallback file (credential store unavailable
    //    now, or earlier when it was created).
    let path = fallback_key_path()?;
    if let Ok(encoded) = tokio::fs::read_to_string(&path).await
        && let Some(key) = decode_key(&encoded)
    {
        // Move it into the credential store when that works now.
        if let Some(entry) = keyring_entry()
            && entry.set_password(&BASE64_STANDARD.encode(key)).is_ok()
        {
            let _ = tokio::fs::remove_file(&path).await;
        }
        return Ok(key);
    }

    // 3. First run: create one.
    let key = new_key()?;
    let encoded = BASE64_STANDARD.encode(key);
    if let Some(entry) = keyring_entry()
        && entry.set_password(&encoded).is_ok()
    {
        return Ok(key);
    }
    tracing::warn!(
        "Storing the session key in {} (no OS credential store)",
        path.display()
    );
    crate::util::io::write(&path, encoded.as_bytes()).await?;
    Ok(key)
}

async fn cipher() -> crate::Result<&'static LessSafeKey> {
    CIPHER
        .get_or_try_init(|| async {
            let key = load_or_create_key().await?;
            let key = UnboundKey::new(&AES_256_GCM, &key)
                .map_err(|_| error("invalid key"))?;
            Ok::<_, crate::Error>(LessSafeKey::new(key))
        })
        .await
}

/// Encrypts a token for storage.
pub async fn encrypt(plaintext: &str) -> crate::Result<String> {
    let cipher = cipher().await?;
    let mut nonce = [0u8; NONCE_LEN];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| error("no secure random source"))?;

    let mut data = plaintext.as_bytes().to_vec();
    cipher
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::empty(),
            &mut data,
        )
        .map_err(|_| error("encryption failed"))?;

    let mut payload = nonce.to_vec();
    payload.extend_from_slice(&data);
    Ok(format!("{PREFIX}{}", BASE64_STANDARD.encode(payload)))
}

/// A token read from storage.
pub struct StoredToken {
    pub value: String,
    /// Saved before encryption existed; should be re-saved.
    pub was_plaintext: bool,
}

/// Decrypts a stored token. Fails when the value was encrypted with another
/// key (e.g. the OS credential store was wiped): that session can't be used.
pub async fn decrypt(stored: &str) -> crate::Result<StoredToken> {
    let Some(encoded) = stored.strip_prefix(PREFIX) else {
        return Ok(StoredToken {
            value: stored.to_string(),
            was_plaintext: true,
        });
    };

    let payload = BASE64_STANDARD
        .decode(encoded)
        .map_err(|_| error("malformed token"))?;
    if payload.len() < NONCE_LEN {
        return Err(error("malformed token"));
    }
    let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
    let nonce = Nonce::try_assume_unique_for_key(nonce)
        .map_err(|_| error("malformed token"))?;

    let cipher = cipher().await?;
    let mut data = ciphertext.to_vec();
    let plaintext = cipher
        .open_in_place(nonce, Aad::empty(), &mut data)
        .map_err(|_| {
            error("token can't be decrypted with this device's key")
        })?;

    Ok(StoredToken {
        value: String::from_utf8(plaintext.to_vec())
            .map_err(|_| error("malformed token"))?,
        was_plaintext: false,
    })
}
