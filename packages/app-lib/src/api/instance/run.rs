use crate::server_address::ServerAddress;
use crate::state::{
    Credentials, ProcessMetadata, Settings, State,
    game_options_sync_is_enabled, load_game_option_preferences,
};
use crate::util::io::IOError;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub enum QuickPlayType {
    None,
    Singleplayer(String),
    Server(ServerAddress),
}

#[tracing::instrument]
pub async fn run(
    instance_id: &str,
    quick_play_type: QuickPlayType,
) -> crate::Result<ProcessMetadata> {
    let state = State::get().await?;
    if crate::state::instances::adapters::sqlite::instance_rows::is_instance_quarantined(
        instance_id,
        &state.pool,
    )
    .await?
    {
        return Err(crate::ErrorKind::InputError(
            "This instance has been quarantined".to_string(),
        )
        .into());
    }
    {
        let _priority =
            state.content_store.legacy_migration_priority.write().await;
        crate::state::instances::commands::migrate_legacy_content(
            instance_id,
            &state,
            false,
        )
        .await?;
    }
    let default_account = Credentials::get_default_credential(&state.pool)
        .await?
        .ok_or_else(|| crate::ErrorKind::NoCredentialsError.as_error())?;

    run_credentials(instance_id, &default_account, quick_play_type).await
}

#[tracing::instrument(skip(credentials))]
async fn run_credentials(
    instance_id: &str,
    credentials: &Credentials,
    quick_play_type: QuickPlayType,
) -> crate::Result<ProcessMetadata> {
    let state = State::get().await?;
    let settings = Settings::get(&state.pool).await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to run a nonexistent instance {instance_id}!"
            ))
        })?;
    let instance_sync_preferences =
        crate::state::instances::adapters::sqlite::instance_rows::get_instance_sync_preferences(
            instance_id,
            &state.pool,
        )
        .await?;
    let fullscreen_is_shared = game_options_sync_is_enabled(&state.pool)
        .await?
        && instance_sync_preferences.game_options
        && load_game_option_preferences(&state.pool)
            .await?
            .get("fullscreen")
            .is_some_and(|preference| preference.enabled);
    if crate::state::instances::adapters::sqlite::instance_rows::is_instance_quarantined(
        instance_id,
        &state.pool,
    )
    .await?
    {
        return Err(crate::ErrorKind::InputError(
            "This instance has been quarantined".to_string(),
        )
        .into());
    }

    let pre_launch_hook = context
        .launch_overrides
        .hooks
        .pre_launch
        .as_ref()
        .or(settings.hooks.pre_launch.as_ref())
        .filter(|hook_command| !hook_command.is_empty());

    let java_args = context
        .launch_overrides
        .extra_launch_args
        .clone()
        .unwrap_or(settings.extra_launch_args);

    let wrapper = context
        .launch_overrides
        .hooks
        .wrapper
        .clone()
        .or(settings.hooks.wrapper)
        .filter(|hook_command| !hook_command.is_empty());

    let env_args = context
        .launch_overrides
        .custom_env_vars
        .clone()
        .unwrap_or(settings.custom_env_vars);

    let post_exit_hook = context
        .launch_overrides
        .hooks
        .post_exit
        .clone()
        .or(settings.hooks.post_exit)
        .filter(|hook_command| !hook_command.is_empty());

    let memory = context.launch_overrides.memory.unwrap_or(settings.memory);
    let resolution = context
        .launch_overrides
        .game_resolution
        .unwrap_or(settings.game_resolution);
    let has_hook_commands = pre_launch_hook.is_some()
        || wrapper.is_some()
        || post_exit_hook.is_some();
    let full_path = if has_hook_commands {
        Some(crate::util::io::canonicalize(
            state
                .directories
                .instances_dir()
                .join(&context.instance.path),
        )?)
    } else {
        None
    };
    let hook_environment = if has_hook_commands {
        let full_path = full_path
            .as_ref()
            .expect("hooked launches always resolve their instance path");
        let java_version =
            crate::launcher::resolve_java_for_launch(&context).await?;

        Some(crate::launcher::hooks::HookEnvironment::from_current_env(
            &env_args,
            crate::launcher::hooks::HookVariables {
                instance_name: context.instance.name.clone(),
                instance_id: context.instance.path.clone(),
                instance_dir: full_path.to_string_lossy().to_string(),
                java_path: java_version.path.clone(),
                java_args: crate::launcher::hooks::build_hook_java_args(
                    &java_args,
                    memory,
                    &java_version,
                ),
            },
        ))
    } else {
        None
    };
    let launch_env_args = hook_environment
        .as_ref()
        .map_or_else(|| env_args.clone(), |env| env.injected_envs());

    if let (Some(hook), Some(hook_environment), Some(full_path)) = (
        pre_launch_hook,
        hook_environment.as_ref(),
        full_path.as_ref(),
    ) {
        let expanded_hook = hook_environment.expand(hook);
        let mut cmd = shlex::split(&expanded_hook)
            .ok_or_else(|| {
                crate::ErrorKind::LauncherError(format!(
                    "Invalid pre-launch command: {hook}",
                ))
            })?
            .into_iter();

        if let Some(command) = cmd.next() {
            let result = Command::new(command)
                .args(cmd)
                .envs(launch_env_args.iter().cloned())
                .current_dir(full_path)
                .spawn()
                .map_err(|e| IOError::with_path(e, full_path))?
                .wait()
                .await
                .map_err(IOError::from)?;

            if !result.success() {
                return Err(crate::ErrorKind::LauncherError(format!(
                    "Non-zero exit code for pre-launch hook: {}",
                    result.code().unwrap_or(-1)
                ))
                .as_error());
            }
        }
    }

    let wrapper = wrapper
        .map(|hook| {
            hook_environment
                .as_ref()
                .map_or(hook.clone(), |env| env.expand(&hook))
        })
        .filter(|hook_command| !hook_command.is_empty());
    let post_exit_hook = post_exit_hook
        .map(|hook| {
            hook_environment
                .as_ref()
                .map_or(hook.clone(), |env| env.expand(&hook))
        })
        .filter(|hook_command| !hook_command.is_empty());

    let mut mc_set_options: Vec<(String, String)> = vec![];
    if let Some(fullscreen) = context.launch_overrides.force_fullscreen {
        mc_set_options.push(("fullscreen".to_string(), fullscreen.to_string()));
    } else if settings.force_fullscreen && !fullscreen_is_shared {
        mc_set_options.push(("fullscreen".to_string(), "true".to_string()));
    }

    crate::minecraft_skins::flush_pending_skin_change().await?;
    crate::launcher::launch_minecraft(
        &java_args,
        &launch_env_args,
        &mc_set_options,
        &wrapper,
        &memory,
        &resolution,
        credentials,
        post_exit_hook,
        &context,
        quick_play_type,
    )
    .await
}

pub async fn kill(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    let processes =
        crate::api::process::get_by_instance_id(instance_id).await?;

    for process in processes {
        state.process_manager.kill(process.uuid).await?;
    }

    Ok(())
}

#[tracing::instrument]
pub async fn try_update_playtime_by_instance_id(
    instance_id: &str,
) -> crate::Result<()> {
    let state = State::get().await?;
    let context =
        crate::state::instances::commands::get_instance_launch_context(
            instance_id,
            &state.pool,
        )
        .await?
        .ok_or_else(|| {
            crate::ErrorKind::OtherError(format!(
                "Tried to update playtime for nonexistent instance {instance_id}!"
            ))
        })?;
    // Only local bookkeeping: folds the recent session into the instance's
    // total play time. Nothing is reported to any third-party service.
    crate::state::instances::commands::mark_instance_playtime_submitted(
        &context.instance.id,
        context.instance.recent_time_played,
        &state.pool,
    )
    .await
}
