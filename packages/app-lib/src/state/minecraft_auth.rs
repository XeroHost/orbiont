use crate::ErrorKind;
use crate::state::session_crypto;
// Every sign-in and Minecraft services request is https: the https-only client
// refuses to follow anything else (e.g. a redirect to plain http).
use crate::util::fetch::REQWEST_CLIENT as AUTH_CLIENT;
use base64::Engine;
use base64::prelude::BASE64_URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, TimeZone, Utc};
use dashmap::DashMap;
use heck::ToTitleCase;
use rand::Rng;
use reqwest::header::HeaderMap;
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::json;
use sha2::Digest;
use std::borrow::Cow;
use std::collections::HashMap;
use std::future::Future;
use std::hash::{BuildHasherDefault, DefaultHasher};
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::{Handle, RuntimeFlavor};
use tokio::sync::Mutex;
use tokio::task;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum MinecraftAuthStep {
    GetOAuthToken,
    RefreshOAuthToken,
    XboxUserAuthenticate,
    XstsAuthorize,
    MinecraftToken,
    MinecraftEntitlements,
    MinecraftProfile,
}

#[derive(thiserror::Error, Debug)]
pub enum MinecraftAuthenticationError {
    #[error(
        "Failed to deserialize response to JSON during step {step:?}: {source}. Status Code: {status_code} Body: {raw}"
    )]
    DeserializeResponse {
        step: MinecraftAuthStep,
        raw: String,
        #[source]
        source: serde_json::Error,
        status_code: StatusCode,
    },
    #[error("Request failed during step {step:?}: {source}")]
    Request {
        step: MinecraftAuthStep,
        #[source]
        source: reqwest::Error,
    },
    #[error("Error reading user hash")]
    NoUserHash,
    #[error("This Microsoft account does not own Minecraft: Java Edition")]
    NoMinecraftLicense,
}

#[derive(Deserialize)]
struct OAuthErrorResponse {
    error: String,
}

impl MinecraftAuthenticationError {
    fn is_invalid_grant(&self) -> bool {
        matches!(
            self,
            Self::DeserializeResponse {
                step: MinecraftAuthStep::RefreshOAuthToken,
                raw,
                status_code: StatusCode::BAD_REQUEST,
                ..
            } if serde_json::from_str::<OAuthErrorResponse>(raw)
                .is_ok_and(|response| matches!(
                    response.error.as_str(),
                    "invalid_grant" | "unauthorized_client"
                ))
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftLoginFlow {
    pub verifier: String,
    pub state: String,
    pub auth_request_uri: String,
    /// Where Microsoft sends the browser back with `?code=` (or `?error=`).
    pub redirect_uri: String,
}

/// Builds the Microsoft sign-in URL (authorization code flow with PKCE, for a
/// public client: there is no client secret).
#[tracing::instrument]
pub async fn login_begin() -> crate::Result<MinecraftLoginFlow> {
    let verifier = generate_oauth_challenge();
    let challenge =
        BASE64_URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(&verifier));
    let state = generate_oauth_challenge();

    let auth_request_uri = Url::parse_with_params(
        MICROSOFT_AUTHORIZE_URL,
        &[
            ("client_id", MICROSOFT_CLIENT_ID),
            ("response_type", "code"),
            ("redirect_uri", AUTH_REPLY_URL),
            ("scope", REQUESTED_SCOPE),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", state.as_str()),
            ("prompt", "select_account"),
        ],
    )
    .map_err(|error| {
        ErrorKind::OtherError(format!("Invalid Microsoft sign-in URL: {error}"))
    })?
    .to_string();

    Ok(MinecraftLoginFlow {
        verifier,
        state,
        auth_request_uri,
        redirect_uri: AUTH_REPLY_URL.to_string(),
    })
}

#[tracing::instrument]
pub async fn login_finish(
    code: &str,
    flow: MinecraftLoginFlow,
    exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
) -> crate::Result<Credentials> {
    let oauth_token = oauth_token(code, &flow.verifier).await?;
    let minecraft_token =
        minecraft_token_from_microsoft(&oauth_token.value.access_token).await?;

    minecraft_entitlements(&minecraft_token.access_token).await?;

    let mut credentials = Credentials {
        offline_profile: MinecraftProfile::default(),
        access_token: minecraft_token.access_token,
        refresh_token: oauth_token.value.refresh_token,
        expires: oauth_token.date
            + Duration::seconds(oauth_token.value.expires_in as i64),
        active: true,
    };

    // During login, we need to fetch the online profile at least once to get the
    // player UUID and name to use for the offline profile, in order for that offline
    // profile to make sense. It's also important to modify the returned credentials
    // object, as otherwise continued usage of it will skip the profile cache due to
    // the dummy UUID
    let online_profile = credentials
        .online_profile()
        .await
        .ok_or(io::Error::other("Failed to fetch player profile"))?;
    credentials.offline_profile = MinecraftProfile {
        id: online_profile.id,
        name: online_profile.name.clone(),
        ..credentials.offline_profile
    };

    credentials.upsert(exec).await?;

    Ok(credentials)
}

#[derive(Deserialize, Debug)]
pub struct Credentials {
    /// The offline profile of the user these credentials are for.
    ///
    /// Such a profile can only be relied upon to have a proper player UUID, which is
    /// never changed. A potentially stale username may be available, but no other data
    /// such as skins or capes is available.
    #[serde(rename = "profile")]
    pub offline_profile: MinecraftProfile,
    pub access_token: String,
    pub refresh_token: String,
    pub expires: DateTime<Utc>,
    pub active: bool,
}

/// An entry in the player profile cache, keyed by player UUID.
pub(super) enum ProfileCacheEntry {
    /// A cached profile that is valid, even though it may be stale.
    Hit(Arc<MinecraftProfile>),
    /// A negative profile fetch result due to an authentication error,
    /// from which we're recovering by holding off from repeatedly
    /// attempting to fetch the profile until the token is refreshed
    /// or some time has passed.
    AuthErrorBackoff {
        likely_expired_token: String,
        last_attempt: Instant,
    },
}

/// A thread-safe cache of online profiles, used to avoid fetching the
/// same profile multiple times as long as they don't get too stale.
///
/// The cache has to be static because credential objects are short lived
/// and disposable, and in the future several threads may be interested in
/// profile data.
pub(super) static PROFILE_CACHE: Mutex<
    HashMap<Uuid, ProfileCacheEntry, BuildHasherDefault<DefaultHasher>>,
> = Mutex::const_new(HashMap::with_hasher(BuildHasherDefault::new()));

const ONLINE_PROFILE_CACHE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(60);
const ONLINE_PROFILE_LIVE_STATE_MAX_AGE: std::time::Duration =
    std::time::Duration::from_secs(5);
const ONLINE_PROFILE_AUTH_ERROR_BACKOFF: std::time::Duration =
    std::time::Duration::from_secs(60);

#[derive(Debug, Clone, Copy)]
enum OnlineProfileCacheIntent {
    NormalRead,
    LiveStateRead,
    RefreshFromMojang,
}

impl OnlineProfileCacheIntent {
    fn max_age(self) -> std::time::Duration {
        match self {
            Self::NormalRead => ONLINE_PROFILE_CACHE_MAX_AGE,
            Self::LiveStateRead => ONLINE_PROFILE_LIVE_STATE_MAX_AGE,
            Self::RefreshFromMojang => std::time::Duration::ZERO,
        }
    }

    fn can_use_stale_on_fetch_error(self) -> bool {
        matches!(self, Self::LiveStateRead)
    }
}

impl Credentials {
    /// Refreshes the authentication tokens for this user if they are expired, or
    /// very close to expiration.
    async fn refresh(
        &mut self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<()> {
        // Use a margin of 5 minutes to give e.g. Minecraft and potentially
        // other operations that depend on a fresh token 5 minutes to complete
        // from now, and deal with some classes of clock skew
        if self.expires > Utc::now() + Duration::minutes(5) {
            return Ok(());
        }

        let oauth_token = oauth_refresh(&self.refresh_token).await?;
        let minecraft_token =
            minecraft_token_from_microsoft(&oauth_token.value.access_token)
                .await?;

        self.access_token = minecraft_token.access_token;
        self.refresh_token = oauth_token.value.refresh_token;
        self.expires = oauth_token.date
            + Duration::seconds(oauth_token.value.expires_in as i64);

        self.upsert(exec).await?;

        Ok(())
    }

    /// Returns online profile data when the cached copy is still recent enough.
    #[tracing::instrument(skip(self))]
    pub async fn online_profile(&self) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::NormalRead,
        )
        .await
    }

    /// Returns profile data recent enough for skin and cape state.
    ///
    /// Reuses a profile read from the last few seconds so opening the skins page
    /// does not send several identical Mojang requests.
    #[tracing::instrument(skip(self))]
    pub async fn online_profile_fresh(&self) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::LiveStateRead,
        )
        .await
    }

    /// Fetches the online profile from Mojang after a skin or cape change.
    #[tracing::instrument(skip(self))]
    pub async fn refresh_online_profile(
        &self,
    ) -> Option<Arc<MinecraftProfile>> {
        self.online_profile_with_cache_intent(
            OnlineProfileCacheIntent::RefreshFromMojang,
        )
        .await
    }

    async fn online_profile_with_cache_intent(
        &self,
        cache_intent: OnlineProfileCacheIntent,
    ) -> Option<Arc<MinecraftProfile>> {
        let max_age = cache_intent.max_age();
        let stale_profile = {
            let mut profile_cache = PROFILE_CACHE.lock().await;
            let mut remove_cached_entry = false;

            let stale_profile = if let Some(cache_entry) =
                profile_cache.get(&self.offline_profile.id)
            {
                match cache_entry {
                    ProfileCacheEntry::Hit(profile)
                        if profile.is_fresh(max_age) =>
                    {
                        return Some(Arc::clone(profile));
                    }
                    ProfileCacheEntry::Hit(profile) => {
                        Some(Arc::clone(profile))
                    }
                    // Auth errors must be handled with a backoff strategy because it
                    // has been experimentally found that Mojang quickly rate limits
                    // the profile data endpoint on repeated attempts with bad auth
                    ProfileCacheEntry::AuthErrorBackoff {
                        likely_expired_token,
                        last_attempt,
                    } if &self.access_token != likely_expired_token
                        || Instant::now()
                            .saturating_duration_since(*last_attempt)
                            > ONLINE_PROFILE_AUTH_ERROR_BACKOFF =>
                    {
                        remove_cached_entry = true;
                        None
                    }
                    ProfileCacheEntry::AuthErrorBackoff { .. } => {
                        return None;
                    }
                }
            } else {
                None
            };

            if remove_cached_entry {
                profile_cache.remove(&self.offline_profile.id);
            }

            stale_profile
        };

        match minecraft_profile(&self.access_token).await {
            Ok(profile) => {
                let profile = Arc::new(profile);
                let cache_entry = ProfileCacheEntry::Hit(Arc::clone(&profile));

                let mut profile_cache = PROFILE_CACHE.lock().await;
                if self.offline_profile.id != profile.id {
                    profile_cache.remove(&self.offline_profile.id);
                }
                profile_cache.insert(profile.id, cache_entry);

                Some(profile)
            }
            Err(
                err @ MinecraftAuthenticationError::DeserializeResponse {
                    status_code: StatusCode::UNAUTHORIZED,
                    ..
                },
            ) => {
                tracing::warn!(
                    "Failed to fetch online profile for UUID {} likely due to stale credentials, backing off: {err}",
                    self.offline_profile.id
                );

                let mut profile_cache = PROFILE_CACHE.lock().await;
                profile_cache.insert(
                    self.offline_profile.id,
                    ProfileCacheEntry::AuthErrorBackoff {
                        likely_expired_token: self.access_token.clone(),
                        last_attempt: Instant::now(),
                    },
                );

                None
            }
            Err(err) => {
                tracing::warn!(
                    "Failed to fetch online profile for UUID {}: {err}",
                    self.offline_profile.id
                );

                if cache_intent.can_use_stale_on_fetch_error() {
                    stale_profile
                } else {
                    None
                }
            }
        }
    }

    /// Attempts to fetch the online profile for this user if possible, and if that fails
    /// falls back to the known offline profile data.
    ///
    /// See also the [`online_profile`](Self::online_profile) method.
    pub async fn maybe_online_profile(
        &self,
    ) -> MaybeOnlineMinecraftProfile<'_> {
        let online_profile = self.online_profile().await;
        online_profile.map_or_else(
            || MaybeOnlineMinecraftProfile::Offline(&self.offline_profile),
            MaybeOnlineMinecraftProfile::Online,
        )
    }

    /// Like [`get_active`](Self::get_active), but enforces credentials to be
    /// successfully refreshed unless the network is unreachable or times out.
    #[tracing::instrument]
    pub async fn get_default_credential(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Credentials>> {
        let credentials = Self::get_active(exec).await?;

        if let Some(mut creds) = credentials {
            let res = creds.refresh(exec).await;

            match res {
                Ok(_) => Ok(Some(creds)),
                Err(err) => {
                    if let ErrorKind::MinecraftAuthenticationError(
                        MinecraftAuthenticationError::Request {
                            ref source,
                            ..
                        },
                    ) = *err.raw
                        && (source.is_connect() || source.is_timeout())
                    {
                        return Ok(Some(creds));
                    }

                    if matches!(
                        &*err.raw,
                        ErrorKind::MinecraftAuthenticationError(source)
                            if source.is_invalid_grant()
                    ) {
                        Self::remove(creds.offline_profile.id, exec).await?;

                        if let Some((_, mut user)) =
                            Self::get_all(exec).await?.into_iter().next()
                        {
                            user.active = true;
                            user.upsert(exec).await?;
                        }

                        return Ok(None);
                    }

                    Err(err)
                }
            }
        } else {
            Ok(None)
        }
    }

    /// Fetches the currently selected credentials from the database, attempting
    /// to refresh them if they are expired.
    pub async fn get_active(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Self>> {
        let res = sqlx::query!(
            "
            SELECT
                uuid, active, username, access_token, refresh_token, expires
            FROM minecraft_users
            WHERE active = TRUE
            "
        )
        .fetch_optional(exec)
        .await?;

        let Some(x) = res else {
            return Ok(None);
        };
        let Some(mut credentials) = Self::from_stored(
            &x.uuid,
            x.username,
            &x.access_token,
            &x.refresh_token,
            x.expires,
            x.active == 1,
            exec,
        )
        .await?
        else {
            return Ok(None);
        };
        credentials.refresh(exec).await.ok();
        Ok(Some(credentials))
    }

    pub async fn get_all(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<DashMap<Uuid, Self>> {
        let rows = sqlx::query!(
            "
            SELECT
                uuid, active, username, access_token, refresh_token, expires
            FROM minecraft_users
            "
        )
        .fetch_all(exec)
        .await?;

        let all = DashMap::new();
        for x in rows {
            let Some(mut credentials) = Self::from_stored(
                &x.uuid,
                x.username,
                &x.access_token,
                &x.refresh_token,
                x.expires,
                x.active == 1,
                exec,
            )
            .await?
            else {
                continue;
            };
            credentials.refresh(exec).await.ok();
            all.insert(credentials.offline_profile.id, credentials);
        }

        Ok(all)
    }

    /// Every stored token, decrypted, without refreshing anything. Only for
    /// scrubbing them out of logs and support reports.
    pub async fn stored_tokens(
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Vec<String>> {
        use sqlx::Row;

        let rows = sqlx::query(
            "SELECT access_token, refresh_token FROM minecraft_users",
        )
        .fetch_all(exec)
        .await?;

        let mut tokens = Vec::with_capacity(rows.len() * 2);
        for row in rows {
            let pair: [String; 2] =
                [row.try_get("access_token")?, row.try_get("refresh_token")?];
            for stored in pair {
                if let Ok(token) = session_crypto::decrypt(&stored).await {
                    tokens.push(token.value);
                }
            }
        }
        Ok(tokens)
    }

    /// Builds credentials from a stored row, decrypting its tokens. A session
    /// saved before encryption is re-saved encrypted; one that can't be
    /// decrypted (another device's key) is dropped, so that account just has
    /// to sign in again.
    #[allow(clippy::too_many_arguments)]
    async fn from_stored(
        uuid: &str,
        username: String,
        access_token: &str,
        refresh_token: &str,
        expires: i64,
        active: bool,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<Option<Self>> {
        let id = Uuid::parse_str(uuid).unwrap_or_default();
        let (access, refresh) = match (
            session_crypto::decrypt(access_token).await,
            session_crypto::decrypt(refresh_token).await,
        ) {
            (Ok(access), Ok(refresh)) => (access, refresh),
            (Err(err), _) | (_, Err(err)) => {
                tracing::warn!(
                    "Dropping the saved session of {username}: {err}"
                );
                Self::remove(id, exec).await?;
                return Ok(None);
            }
        };

        let credentials = Self {
            offline_profile: MinecraftProfile {
                id,
                name: username,
                ..MinecraftProfile::default()
            },
            access_token: access.value,
            refresh_token: refresh.value,
            expires: Utc
                .timestamp_opt(expires, 0)
                .single()
                .unwrap_or_else(Utc::now),
            active,
        };
        if access.was_plaintext || refresh.was_plaintext {
            credentials.store(exec).await?;
        }
        Ok(Some(credentials))
    }

    pub async fn upsert(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<()> {
        if self.active {
            sqlx::query!(
                "
                UPDATE minecraft_users
                SET active = FALSE
                ",
            )
            .execute(exec)
            .await?;
        }

        self.store(exec).await
    }

    /// Writes this row with its tokens encrypted (see `session_crypto`).
    async fn store(
        &self,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite> + Copy,
    ) -> crate::Result<()> {
        let profile = self.maybe_online_profile().await;
        let expires = self.expires.timestamp();
        let uuid = profile.id.as_hyphenated().to_string();
        let access_token = session_crypto::encrypt(&self.access_token).await?;
        let refresh_token =
            session_crypto::encrypt(&self.refresh_token).await?;

        sqlx::query!(
            "
            INSERT INTO minecraft_users (uuid, active, username, access_token, refresh_token, expires)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (uuid) DO UPDATE SET
                active = $2,
                username = $3,
                access_token = $4,
                refresh_token = $5,
                expires = $6
            ",
            uuid,
            self.active,
            profile.name,
            access_token,
            refresh_token,
            expires,
        )
            .execute(exec)
            .await?;

        Ok(())
    }

    pub async fn remove(
        uuid: Uuid,
        exec: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> crate::Result<()> {
        let uuid = uuid.as_hyphenated().to_string();

        sqlx::query!(
            "
            DELETE FROM minecraft_users WHERE uuid = $1
            ",
            uuid,
        )
        .execute(exec)
        .await?;

        Ok(())
    }
}

impl Serialize for Credentials {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Opportunistically hydrate the profile with its online data if possible for frontend
        // consumption, transparently handling all the possible Tokio runtime states the current
        // thread may be in the most efficient way
        let profile = match Handle::try_current().ok() {
            Some(runtime)
                if runtime.runtime_flavor() == RuntimeFlavor::CurrentThread =>
            {
                runtime.block_on(self.maybe_online_profile())
            }
            Some(runtime) => task::block_in_place(|| {
                runtime.block_on(self.maybe_online_profile())
            }),
            None => tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_or_else(
                    |_| {
                        MaybeOnlineMinecraftProfile::Offline(
                            &self.offline_profile,
                        )
                    },
                    |runtime| runtime.block_on(self.maybe_online_profile()),
                ),
        };

        // Tokens stay in the core: the UI only needs who is signed in.
        let mut ser = serializer.serialize_struct("Credentials", 3)?;
        ser.serialize_field("profile", &*profile)?;
        ser.serialize_field("expires", &self.expires)?;
        ser.serialize_field("active", &self.active)?;
        ser.end()
    }
}

const MICROSOFT_CLIENT_ID: &str = env!("MICROSOFT_CLIENT_ID");
// Personal Microsoft accounts only: Minecraft can't be owned by work/school ones.
const MICROSOFT_AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const MICROSOFT_TOKEN_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const AUTH_REPLY_URL: &str =
    "https://login.microsoftonline.com/common/oauth2/nativeclient";
const REQUESTED_SCOPE: &str = "XboxLive.signin offline_access";
pub const MINECRAFT_SERVICES_USER_AGENT: &str = concat!(
    env!("ORBIONT_PRODUCT_NAME"),
    " (",
    env!("ORBIONT_SUPPORT_EMAIL"),
    "; ",
    env!("ORBIONT_SITE_URL"),
    ")"
);

pub struct RequestWithDate<T> {
    pub date: DateTime<Utc>,
    pub value: T,
}

// flow steps
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct XboxToken {
    pub issue_instant: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub token: String,
    pub display_claims: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct OAuthToken {
    // pub token_type: String,
    pub expires_in: u64,
    // pub scope: String,
    pub access_token: String,
    pub refresh_token: String,
    // pub user_id: String,
    // pub foci: String,
}

#[tracing::instrument]
async fn oauth_token(
    code: &str,
    verifier: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("code", code);
    query.insert("code_verifier", verifier);
    query.insert("grant_type", "authorization_code");
    query.insert("redirect_uri", AUTH_REPLY_URL);
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        AUTH_CLIENT
            .post(MICROSOFT_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::GetOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::GetOAuthToken,
        }
    })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::GetOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

#[tracing::instrument]
async fn oauth_refresh(
    refresh_token: &str,
) -> Result<RequestWithDate<OAuthToken>, MinecraftAuthenticationError> {
    let mut query = HashMap::new();
    query.insert("client_id", MICROSOFT_CLIENT_ID);
    query.insert("refresh_token", refresh_token);
    query.insert("grant_type", "refresh_token");
    query.insert("scope", REQUESTED_SCOPE);

    let res = auth_retry(|| {
        AUTH_CLIENT
            .post(MICROSOFT_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&query)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::RefreshOAuthToken,
    })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::RefreshOAuthToken,
        }
    })?;

    let body = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step: MinecraftAuthStep::RefreshOAuthToken,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value: body,
    })
}

/// Microsoft access token -> Xbox Live user token -> XSTS token -> Minecraft
/// access token (the flow for launchers with their own, Mojang-approved
/// Microsoft client ID).
async fn minecraft_token_from_microsoft(
    microsoft_access_token: &str,
) -> Result<MinecraftToken, MinecraftAuthenticationError> {
    let user_token = xbox_user_authenticate(microsoft_access_token).await?;
    let xsts_token = xsts_authorize(&user_token.value.token).await?;
    login_with_xbox(xsts_token.value).await
}

#[tracing::instrument(skip(microsoft_access_token))]
async fn xbox_user_authenticate(
    microsoft_access_token: &str,
) -> Result<RequestWithDate<XboxToken>, MinecraftAuthenticationError> {
    post_json(
        "https://user.auth.xboxlive.com/user/authenticate",
        json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={microsoft_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        }),
        MinecraftAuthStep::XboxUserAuthenticate,
    )
    .await
}

#[tracing::instrument(skip(user_token))]
async fn xsts_authorize(
    user_token: &str,
) -> Result<RequestWithDate<XboxToken>, MinecraftAuthenticationError> {
    // On failure Xbox answers 401 with an `XErr` code (no Xbox profile, child
    // account, region...), which ends up in the error for the UI to explain.
    post_json(
        "https://xsts.auth.xboxlive.com/xsts/authorize",
        json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [user_token],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        }),
        MinecraftAuthStep::XstsAuthorize,
    )
    .await
}

#[derive(Deserialize)]
struct MinecraftToken {
    // pub username: String,
    pub access_token: String,
    // pub token_type: String,
    // pub expires_in: u64,
}

#[tracing::instrument(skip(xsts_token))]
async fn login_with_xbox(
    xsts_token: XboxToken,
) -> Result<MinecraftToken, MinecraftAuthenticationError> {
    let uhs = xsts_token
        .display_claims
        .get("xui")
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("uhs"))
        .and_then(|x| x.as_str().map(String::from))
        .ok_or_else(|| MinecraftAuthenticationError::NoUserHash)?;

    let res = post_json::<MinecraftToken>(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        json!({
            "identityToken": format!("XBL3.0 x={uhs};{}", xsts_token.token),
        }),
        MinecraftAuthStep::MinecraftToken,
    )
    .await?;

    Ok(res.value)
}

#[derive(
    sqlx::Type, Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq,
)]
#[serde(rename_all = "UPPERCASE")]
#[sqlx(rename_all = "UPPERCASE")]
pub enum MinecraftSkinVariant {
    /// The classic player model, with arms that are 4 pixels wide.
    Classic,
    /// The slim player model, with arms that are 3 pixels wide.
    Slim,
    /// The player model is unknown.
    #[serde(other)]
    Unknown, // Defensive handling of unexpected Mojang API return values to
             // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum MinecraftCharacterExpressionState {
    /// This expression is selected for being displayed ingame.
    ///
    /// At the moment, at most one expression can be selected at a time.
    Active,
    /// This expression is not selected for being displayed ingame.
    Inactive,
    /// The expression selection status is unknown.
    #[serde(other)]
    Unknown, // Defensive handling of unexpected Mojang API return values to
             // prevent breaking the entire profile parsing
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftSkin {
    /// The UUID of this skin object.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this UUID
    /// changes every time the player changes their skin, even if the skin
    /// texture is the same as before.
    pub id: Uuid,
    /// The selection state of the skin.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this
    /// is always `ACTIVE`, as only a single skin representing the current
    /// skin is returned.
    pub state: MinecraftCharacterExpressionState,
    /// The URL to the skin texture.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint the file
    /// name for this URL is a hash of the skin texture, so that different
    /// players using the same skin texture will share a texture URL.
    pub url: Arc<Url>,
    /// A hash of the skin texture.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this
    /// is always set and the same as the file name of the skin texture URL.
    #[serde(
        default, // Defensive handling of unexpected Mojang API return values to
                 // prevent breaking the entire profile parsing
        rename = "textureKey"
    )]
    pub texture_key: Option<Arc<str>>,
    /// The player model variant this skin is for.
    pub variant: MinecraftSkinVariant,
    /// User-friendly name for the skin.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint this is
    /// only set if the player has not set a custom skin, and this skin object
    /// is therefore the default skin for the player's UUID.
    #[serde(
        default,
        rename = "alias",
        deserialize_with = "normalize_skin_alias_case"
    )]
    pub name: Option<String>,
}

impl MinecraftSkin {
    /// Robustly computes the texture key for this skin, falling back to its
    /// URL file name and finally to the skin UUID when necessary.
    pub fn texture_key(&self) -> Arc<str> {
        self.texture_key.as_ref().cloned().unwrap_or_else(|| {
            self.url
                .path_segments()
                .and_then(|mut path_segments| {
                    path_segments.next_back().map(String::from)
                })
                .unwrap_or_else(|| self.id.as_simple().to_string())
                .into()
        })
    }
}

fn normalize_skin_alias_case<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    // Skin aliases have been spotted to be returned in all caps, so make sure
    // they are normalized to a prettier title case
    Ok(<Option<Cow<'_, str>>>::deserialize(deserializer)?
        .map(|alias| alias.to_title_case()))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MinecraftCape {
    /// The UUID of the cape.
    pub id: Uuid,
    /// The selection state of the cape.
    pub state: MinecraftCharacterExpressionState,
    /// The URL to the cape texture.
    pub url: Arc<Url>,
    /// The user-friendly name for the cape.
    #[serde(rename = "alias")]
    pub name: Arc<str>,
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct MinecraftProfile {
    /// The UUID of the player.
    #[serde(default)]
    pub id: Uuid,
    /// The username of the player.
    pub name: String,
    /// The skins the player is known to have.
    ///
    /// As of 2025-04-08, in the production Mojang profile endpoint every
    /// player has a single skin.
    pub skins: Vec<MinecraftSkin>,
    /// The capes the player is known to have.
    pub capes: Vec<MinecraftCape>,
    /// The instant when the profile was fetched. See also [Self::is_fresh].
    #[serde(skip)]
    pub fetch_time: Option<Instant>,
}

impl MinecraftProfile {
    /// Checks whether the profile data is fresh (i.e., highly likely to be
    /// up-to-date because it was fetched recently) or stale. If it is not
    /// known when this profile data has been fetched from Mojang servers (i.e.,
    /// `fetch_time` is `None`), the profile is considered stale.
    ///
    /// This can be used to determine if the profile data should be fetched again
    /// from the Mojang API: the vanilla launcher was seen refreshing profile
    /// data every 60 seconds when re-entering the skin selection screen, and
    /// external applications may change this data at any time.
    fn is_fresh(&self, max_age: std::time::Duration) -> bool {
        self.fetch_time.is_some_and(|last_profile_fetch_time| {
            Instant::now().saturating_duration_since(last_profile_fetch_time)
                < max_age
        })
    }

    /// Returns the currently selected skin for this profile.
    pub fn current_skin(&self) -> crate::Result<&MinecraftSkin> {
        Ok(self
            .skins
            .iter()
            .find(|skin| {
                skin.state == MinecraftCharacterExpressionState::Active
            })
            // There should always be one active skin, even when the player uses their default skin
            .ok_or_else(|| {
                ErrorKind::OtherError("No active skin found".into())
            })?)
    }

    /// Returns the currently selected cape for this profile.
    pub fn current_cape(&self) -> Option<&MinecraftCape> {
        self.capes.iter().find(|cape| {
            cape.state == MinecraftCharacterExpressionState::Active
        })
    }
}

pub enum MaybeOnlineMinecraftProfile<'profile> {
    /// An online profile, fetched from the Mojang API.
    Online(Arc<MinecraftProfile>),
    /// An offline profile, which has not been fetched from the Mojang API.
    Offline(&'profile MinecraftProfile),
}

impl Deref for MaybeOnlineMinecraftProfile<'_> {
    type Target = MinecraftProfile;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Online(profile) => profile,
            Self::Offline(profile) => profile,
        }
    }
}

#[tracing::instrument(skip(token))]
async fn minecraft_profile(
    token: &str,
) -> Result<MinecraftProfile, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        AUTH_CLIENT
            .get("https://api.minecraftservices.com/minecraft/profile")
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .bearer_auth(token)
            // Profiles may be refreshed periodically in response to user actions,
            // so we want each refresh to be fast
            .timeout(std::time::Duration::from_secs(10))
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request {
        source,
        step: MinecraftAuthStep::MinecraftProfile,
    })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftProfile,
        }
    })?;

    let mut profile =
        serde_json::from_str::<MinecraftProfile>(&text).map_err(|source| {
            MinecraftAuthenticationError::DeserializeResponse {
                source,
                raw: text,
                step: MinecraftAuthStep::MinecraftProfile,
                status_code: status,
            }
        })?;
    profile.fetch_time = Some(Instant::now());

    tracing::debug!(
        "Successfully fetched Minecraft profile for {}",
        profile.name
    );

    Ok(profile)
}

/// Licenses Mojang reports for an account that can play Java Edition (bought
/// or through Game Pass).
const MINECRAFT_LICENSES: &[&str] = &["product_minecraft", "game_minecraft"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEntitlements {
    #[serde(default)]
    items: Vec<MinecraftEntitlement>,
}

#[derive(Deserialize)]
struct MinecraftEntitlement {
    name: String,
}

#[tracing::instrument]
async fn minecraft_entitlements(
    token: &str,
) -> Result<MinecraftEntitlements, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
		AUTH_CLIENT
			.get(format!("https://api.minecraftservices.com/entitlements/license?requestId={}", Uuid::new_v4()))
			.header("Accept", "application/json")
			.header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
			.bearer_auth(token)
			.send()
	})
    .await.map_err(|source| MinecraftAuthenticationError::Request { source, step: MinecraftAuthStep::MinecraftEntitlements })?;

    let status = res.status();
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request {
            source,
            step: MinecraftAuthStep::MinecraftEntitlements,
        }
    })?;

    let entitlements: MinecraftEntitlements = serde_json::from_str(&text)
        .map_err(|source| {
            MinecraftAuthenticationError::DeserializeResponse {
                source,
                raw: text,
                step: MinecraftAuthStep::MinecraftEntitlements,
                status_code: status,
            }
        })?;

    // Premium only: the account must actually hold a Minecraft license.
    if !entitlements
        .items
        .iter()
        .any(|item| MINECRAFT_LICENSES.contains(&item.name.as_str()))
    {
        return Err(MinecraftAuthenticationError::NoMinecraftLicense);
    }

    Ok(entitlements)
}

// auth utils
#[tracing::instrument(skip(reqwest_request))]
async fn auth_retry<F>(
    reqwest_request: impl Fn() -> F,
) -> Result<reqwest::Response, reqwest::Error>
where
    F: Future<Output = Result<Response, reqwest::Error>>,
{
    const RETRY_COUNT: usize = 5; // Does command 9 times
    const RETRY_WAIT: std::time::Duration =
        std::time::Duration::from_millis(250);

    let mut resp = reqwest_request().await;
    for i in 0..RETRY_COUNT {
        match &resp {
            Ok(_) => {
                break;
            }
            Err(err) => {
                if err.is_connect() || err.is_timeout() {
                    if i < RETRY_COUNT - 1 {
                        tracing::debug!(
                            "Request failed with connect error, retrying...",
                        );
                        tokio::time::sleep(RETRY_WAIT).await;
                        resp = reqwest_request().await;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    resp
}

#[tracing::instrument(skip(body))]
async fn post_json<T: DeserializeOwned>(
    url: &str,
    body: serde_json::Value,
    step: MinecraftAuthStep,
) -> Result<RequestWithDate<T>, MinecraftAuthenticationError> {
    let res = auth_retry(|| {
        AUTH_CLIENT
            .post(url)
            .header("Accept", "application/json")
            .header("User-Agent", MINECRAFT_SERVICES_USER_AGENT)
            .json(&body)
            .send()
    })
    .await
    .map_err(|source| MinecraftAuthenticationError::Request { source, step })?;

    let status = res.status();
    let current_date = get_date_header(res.headers());
    let text = res.text().await.map_err(|source| {
        MinecraftAuthenticationError::Request { source, step }
    })?;

    let value = serde_json::from_str(&text).map_err(|source| {
        MinecraftAuthenticationError::DeserializeResponse {
            source,
            raw: text,
            step,
            status_code: status,
        }
    })?;

    Ok(RequestWithDate {
        date: current_date,
        value,
    })
}

#[tracing::instrument]
fn get_date_header(headers: &HeaderMap) -> DateTime<Utc> {
    headers
        .get(reqwest::header::DATE)
        .and_then(|x| x.to_str().ok())
        .and_then(|x| DateTime::parse_from_rfc2822(x).ok())
        .map_or(Utc::now(), |x| x.with_timezone(&Utc))
}

#[tracing::instrument]
fn generate_oauth_challenge() -> String {
    let mut rng = rand::thread_rng();

    let bytes: Vec<u8> = (0..64).map(|_| rng.r#gen::<u8>()).collect();
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
