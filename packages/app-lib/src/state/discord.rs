use std::sync::{Arc, atomic::AtomicBool};

use discord_rich_presence::{
    DiscordIpc, DiscordIpcClient,
    activity::{Activity, Assets},
};
use tokio::sync::RwLock;

use crate::State;

// The Discord application (Developer Portal) decides the name and icon shown
// on the user's profile. Empty until Orbiont has its own application, in which
// case Rich Presence stays off instead of showing another app's name.
const DISCORD_CLIENT_ID: &str = env!("DISCORD_CLIENT_ID");
// Key of the art asset uploaded to that application (Rich Presence > Art Assets).
const LARGE_IMAGE_KEY: &str = "orbiont-icon-cyan-512x";

pub struct DiscordGuard {
    client: Option<Arc<RwLock<DiscordIpcClient>>>,
    connected: Arc<AtomicBool>,
}

impl DiscordGuard {
    /// Initialize discord IPC client, and attempt to connect to it
    /// If it fails, it will still return a DiscordGuard, but the client will be unconnected
    pub fn init() -> crate::Result<DiscordGuard> {
        let client_id = DISCORD_CLIENT_ID.trim();
        let client = (!client_id.is_empty())
            .then(|| Arc::new(RwLock::new(DiscordIpcClient::new(client_id))));

        Ok(DiscordGuard {
            client,
            connected: Arc::new(AtomicBool::new(false)),
        })
    }

    /// If the client failed connecting during init(), this will check for connection and attempt to reconnect
    /// This MUST be called first in any client method that requires a connection, because those can PANIC if the client is not connected
    /// (No connection is different than a failed connection, the latter will not panic and can be retried)
    pub async fn retry_if_not_ready(&self) -> bool {
        let Some(client) = &self.client else {
            return false;
        };
        let mut client = client.write().await;
        if !self.connected.load(std::sync::atomic::Ordering::Relaxed) {
            if client.connect().is_ok() {
                self.connected
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            return false;
        }
        true
    }

    /// Set the activity to the given message
    /// First checks if discord is disabled, and if so, clear the activity instead
    pub async fn set_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Check if discord is disabled, and if so, clear the activity instead
        let state = State::get().await?;
        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            Ok(self.clear_activity(true).await?)
        } else {
            Ok(self.force_set_activity(msg, reconnect_if_fail).await?)
        }
    }

    /// Sets the activity to the given message, regardless of if discord is disabled or offline
    /// Should not be used except for in the above method, or if it is already known that discord is enabled (specifically for state initialization) and we are connected to the internet
    pub async fn force_set_activity(
        &self,
        msg: &str,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Attempt to connect if not connected. Do not continue if it fails, as the client.set_activity can panic if it never was connected
        if !self.retry_if_not_ready().await {
            return Ok(());
        }

        let Some(client) = &self.client else {
            return Ok(());
        };
        let activity = Activity::new()
            .state(msg)
            .assets(Assets::new().large_image(LARGE_IMAGE_KEY));

        // Attempt to set the activity
        // If the existing connection fails, attempt to reconnect and try again
        let mut client: tokio::sync::RwLockWriteGuard<'_, DiscordIpcClient> =
            client.write().await;
        let res = client.set_activity(activity.clone());

        if reconnect_if_fail {
            if let Err(_e) = res {
                client.reconnect()?;
                return Ok(client.set_activity(activity)?); // try again, but don't reconnect if it fails again
            }
        } else {
            res?;
        }

        Ok(())
    }

    /// Clear the activity entirely ('disabling' the RPC until the next set_activity)
    pub async fn clear_activity(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        // Attempt to connect if not connected. Do not continue if it fails, as the client.clear_activity can panic if it never was connected
        if !self.retry_if_not_ready().await {
            return Ok(());
        }

        let Some(client) = &self.client else {
            return Ok(());
        };
        // Attempt to clear the activity
        // If the existing connection fails, attempt to reconnect and try again
        let mut client = client.write().await;
        let res = client.clear_activity();

        if reconnect_if_fail {
            if res.is_err() {
                client.reconnect()?;
                return Ok(client.clear_activity()?); // try again, but don't reconnect if it fails again
            }
        } else {
            res?;
        }
        Ok(())
    }

    /// Clear the activity, but if there is a running profile, set the activity to that instead
    pub async fn clear_to_default(
        &self,
        reconnect_if_fail: bool,
    ) -> crate::Result<()> {
        let state = State::get().await?;

        let settings = crate::state::Settings::get(&state.pool).await?;
        if !settings.discord_rpc {
            return self.clear_activity(true).await;
        }

        let running_instances = state.process_manager.get_all();
        if let Some(existing_child) = running_instances.first() {
            self.set_activity(
                &format!("Playing {}", existing_child.instance_name),
                reconnect_if_fail,
            )
            .await?;
        } else {
            self.set_activity("Idling...", reconnect_if_fail).await?;
        }
        Ok(())
    }
}
