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
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
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

#[derive(Clone, Copy)]
enum KeyringRead {
    Key([u8; KEY_LEN]),
    Missing,
    Unavailable,
    Invalid,
}

trait KeyringStore {
    fn read(&self) -> KeyringRead;
    fn write(&self, key: &[u8; KEY_LEN]) -> bool;
}

struct OsKeyring(Option<keyring::Entry>);

impl KeyringStore for OsKeyring {
    fn read(&self) -> KeyringRead {
        let Some(entry) = &self.0 else {
            return KeyringRead::Unavailable;
        };
        match entry.get_password() {
            Ok(encoded) => decode_key(&encoded)
                .map(KeyringRead::Key)
                .unwrap_or(KeyringRead::Invalid),
            Err(keyring::Error::NoEntry) => KeyringRead::Missing,
            Err(_) => KeyringRead::Unavailable,
        }
    }

    fn write(&self, key: &[u8; KEY_LEN]) -> bool {
        self.0.as_ref().is_some_and(|entry| {
            entry.set_password(&BASE64_STANDARD.encode(key)).is_ok()
        })
    }
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

async fn load_key(
    store: &impl KeyringStore,
    path: &Path,
    can_create: bool,
) -> crate::Result<[u8; KEY_LEN]> {
    let keyring = store.read();
    match keyring {
        KeyringRead::Invalid => {
            return Err(error("stored OS session key is malformed; retained"));
        }
        KeyringRead::Key(_)
        | KeyringRead::Missing
        | KeyringRead::Unavailable => {}
    }

    match tokio::fs::read_to_string(path).await {
        Ok(encoded) => {
            let key = decode_key(&encoded).ok_or_else(|| {
                error("fallback session key is malformed; retained")
            })?;
            if matches!(keyring, KeyringRead::Key(saved) if saved != key) {
                return Err(error(
                    "OS and fallback session keys differ; both retained",
                ));
            }
            // An unavailable store may still contain a different key. Only migrate
            // after a confirmed NoEntry, and remove the fallback after readback.
            if matches!(keyring, KeyringRead::Missing)
                && store.write(&key)
                && matches!(store.read(), KeyringRead::Key(saved) if saved == key)
            {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Ok(key);
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {
            return Err(error("fallback session key is unreadable; retained"));
        }
    }

    if let KeyringRead::Key(key) = keyring {
        return Ok(key);
    }

    // Callers must establish that no encrypted sessions exist before allowing
    // creation. Decryption never creates a replacement for a missing key.
    if !can_create {
        return Err(error(
            "session key is unavailable; saved sessions retained",
        ));
    }
    let key = new_key()?;
    if matches!(keyring, KeyringRead::Missing) && store.write(&key) {
        return Ok(key);
    }

    // Never truncate an existing fallback, including one created concurrently.
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path).await?;
    file.write_all(BASE64_STANDARD.encode(key).as_bytes())
        .await?;
    file.sync_all().await?;
    Ok(key)
}

async fn cipher(allow_create: bool) -> crate::Result<&'static LessSafeKey> {
    CIPHER
        .get_or_try_init(|| async {
            let can_create = if allow_create {
                let state = crate::State::get_if_initialized()
                    .ok_or_else(|| error("app database isn't ready"))?;
                can_create_key(&state.pool).await?
            } else {
                false
            };
            let store = OsKeyring(
                keyring::Entry::new(env!("ORBIONT_PRODUCT_NAME"), KEYRING_USER)
                    .ok(),
            );
            let key =
                load_key(&store, &fallback_key_path()?, can_create).await?;
            let key = UnboundKey::new(&AES_256_GCM, &key)
                .map_err(|_| error("invalid key"))?;
            Ok::<_, crate::Error>(LessSafeKey::new(key))
        })
        .await
}

async fn can_create_key(
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
) -> crate::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT EXISTS(SELECT 1 FROM minecraft_users WHERE access_token LIKE 'enc:%' OR refresh_token LIKE 'enc:%')",
    )
    .fetch_one(exec)
    .await? == 0)
}

/// Encrypts a token for storage.
pub async fn encrypt(plaintext: &str) -> crate::Result<String> {
    let cipher = cipher(true).await?;
    encrypt_with_cipher(cipher, plaintext)
}

fn encrypt_with_cipher(
    cipher: &LessSafeKey,
    plaintext: &str,
) -> crate::Result<String> {
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
        if stored.starts_with("enc:") {
            return Err(error("unsupported encrypted token version"));
        }
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

    let cipher = cipher(false).await?;
    decrypt_with_cipher(cipher, nonce, ciphertext)
}

fn decrypt_with_cipher(
    cipher: &LessSafeKey,
    nonce: Nonce,
    ciphertext: &[u8],
) -> crate::Result<StoredToken> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct FixtureStore {
        state: Cell<KeyringRead>,
        writes: Cell<usize>,
        write_ok: bool,
        readback_ok: bool,
    }

    impl FixtureStore {
        fn new(state: KeyringRead) -> Self {
            Self {
                state: Cell::new(state),
                writes: Cell::new(0),
                write_ok: true,
                readback_ok: true,
            }
        }
    }

    impl KeyringStore for FixtureStore {
        fn read(&self) -> KeyringRead {
            self.state.get()
        }

        fn write(&self, key: &[u8; KEY_LEN]) -> bool {
            self.writes.set(self.writes.get() + 1);
            if self.write_ok && self.readback_ok {
                self.state.set(KeyringRead::Key(*key));
            }
            self.write_ok
        }
    }

    fn fixture_cipher(key: &[u8; KEY_LEN]) -> LessSafeKey {
        LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).unwrap())
    }

    #[tokio::test]
    async fn keyring_outage_does_not_create_or_replace_key_and_can_recover() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALLBACK_KEY_FILE);
        let store = FixtureStore::new(KeyringRead::Unavailable);
        assert!(load_key(&store, &path, false).await.is_err());
        assert_eq!(store.writes.get(), 0);
        assert!(!path.exists());
        store.state.set(KeyringRead::Key([7; KEY_LEN]));
        assert_eq!(load_key(&store, &path, false).await.unwrap(), [7; KEY_LEN]);
        assert_eq!(store.writes.get(), 0);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn missing_key_is_created_only_when_explicitly_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALLBACK_KEY_FILE);
        let store = FixtureStore::new(KeyringRead::Missing);
        assert!(load_key(&store, &path, false).await.is_err());
        assert_eq!(store.writes.get(), 0);
        let key = load_key(&store, &path, true).await.unwrap();
        assert!(
            matches!(store.read(), KeyringRead::Key(saved) if saved == key)
        );
        assert_eq!(store.writes.get(), 1);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn confirmed_new_install_without_keyring_uses_durable_fallback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALLBACK_KEY_FILE);
        let store = FixtureStore::new(KeyringRead::Unavailable);
        let key = load_key(&store, &path, true).await.unwrap();
        let encoded = tokio::fs::read_to_string(&path).await.unwrap();
        assert_eq!(decode_key(&encoded), Some(key));
        assert_eq!(load_key(&store, &path, false).await.unwrap(), key);
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), encoded);
        assert_eq!(store.writes.get(), 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[tokio::test]
    async fn malformed_or_unreadable_keys_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALLBACK_KEY_FILE);
        tokio::fs::write(&path, b"malformed-fixture").await.unwrap();
        let store = FixtureStore::new(KeyringRead::Missing);
        assert!(load_key(&store, &path, true).await.is_err());
        assert_eq!(tokio::fs::read(&path).await.unwrap(), b"malformed-fixture");
        assert_eq!(store.writes.get(), 0);

        tokio::fs::write(&path, BASE64_STANDARD.encode([7; KEY_LEN]))
            .await
            .unwrap();
        store.state.set(KeyringRead::Invalid);
        assert!(load_key(&store, &path, true).await.is_err());
        assert_eq!(
            decode_key(&tokio::fs::read_to_string(&path).await.unwrap()),
            Some([7; KEY_LEN])
        );
        assert_eq!(store.writes.get(), 0);

        let unreadable = dir.path().join("directory-not-a-key");
        tokio::fs::create_dir(&unreadable).await.unwrap();
        store.state.set(KeyringRead::Missing);
        assert!(load_key(&store, &unreadable, true).await.is_err());
        assert!(unreadable.is_dir());
        assert_eq!(store.writes.get(), 0);
    }

    #[tokio::test]
    async fn fallback_migration_requires_no_entry_and_verified_readback() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALLBACK_KEY_FILE);
        let encoded = BASE64_STANDARD.encode([7; KEY_LEN]);
        for (write_ok, readback_ok, removed) in [
            (false, false, false),
            (true, false, false),
            (true, true, true),
        ] {
            tokio::fs::write(&path, &encoded).await.unwrap();
            let store = FixtureStore {
                write_ok,
                readback_ok,
                ..FixtureStore::new(KeyringRead::Missing)
            };
            assert_eq!(
                load_key(&store, &path, false).await.unwrap(),
                [7; KEY_LEN]
            );
            assert_eq!(path.exists(), !removed);
            if !removed {
                assert_eq!(
                    tokio::fs::read_to_string(&path).await.unwrap(),
                    encoded
                );
            }
        }
        tokio::fs::write(&path, &encoded).await.unwrap();
        let unavailable = FixtureStore::new(KeyringRead::Unavailable);
        assert_eq!(
            load_key(&unavailable, &path, false).await.unwrap(),
            [7; KEY_LEN]
        );
        assert_eq!(unavailable.writes.get(), 0);
        assert!(path.exists());
    }

    #[tokio::test]
    async fn conflicting_os_and_fallback_keys_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FALLBACK_KEY_FILE);
        let encoded = BASE64_STANDARD.encode([7; KEY_LEN]);
        tokio::fs::write(&path, &encoded).await.unwrap();
        let store = FixtureStore::new(KeyringRead::Key([8; KEY_LEN]));
        assert!(load_key(&store, &path, true).await.is_err());
        assert_eq!(store.writes.get(), 0);
        assert!(
            matches!(store.read(), KeyringRead::Key(key) if key == [8; KEY_LEN])
        );
        assert_eq!(tokio::fs::read_to_string(&path).await.unwrap(), encoded);
    }

    #[tokio::test]
    async fn database_with_encrypted_sessions_disallows_creation() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE minecraft_users(access_token TEXT, refresh_token TEXT)")
            .execute(&pool).await.unwrap();
        assert!(can_create_key(&pool).await.unwrap());
        sqlx::query("INSERT INTO minecraft_users VALUES ('legacy-fixture', 'legacy-refresh')")
            .execute(&pool).await.unwrap();
        assert!(can_create_key(&pool).await.unwrap());
        sqlx::query("INSERT INTO minecraft_users VALUES ('legacy-fixture', 'enc:v2:fixture')")
            .execute(&pool).await.unwrap();
        assert!(!can_create_key(&pool).await.unwrap());
        sqlx::query("DROP TABLE minecraft_users")
            .execute(&pool)
            .await
            .unwrap();
        assert!(can_create_key(&pool).await.is_err());
    }

    #[test]
    fn encrypted_fixture_survives_wrong_key_and_decrypts_with_original() {
        let original = fixture_cipher(&[7; KEY_LEN]);
        let wrong = fixture_cipher(&[8; KEY_LEN]);
        let encrypted =
            encrypt_with_cipher(&original, "fixture-token").unwrap();
        let payload = BASE64_STANDARD
            .decode(encrypted.strip_prefix(PREFIX).unwrap())
            .unwrap();
        let (nonce, ciphertext) = payload.split_at(NONCE_LEN);
        assert!(
            decrypt_with_cipher(
                &wrong,
                Nonce::try_assume_unique_for_key(nonce).unwrap(),
                ciphertext
            )
            .is_err()
        );
        let decrypted = decrypt_with_cipher(
            &original,
            Nonce::try_assume_unique_for_key(nonce).unwrap(),
            ciphertext,
        )
        .unwrap();
        assert_eq!(decrypted.value, "fixture-token");
        assert!(!decrypted.was_plaintext);
    }
}
