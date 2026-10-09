//! Integration with the registered, official Minecraft for Windows application.
//! Minecraft and Microsoft Store retain authentication, licensing and imports.

use serde::Serialize;
use std::path::PathBuf;

mod catalog;
pub use catalog::import_catalog_file;

mod data;
pub use data::*;
#[cfg(any(windows, test))]
mod coordination;
#[cfg(any(windows, test))]
mod manage;
mod process;
#[cfg(any(windows, test))]
mod storage;
pub use process::{LaunchDiagnostic, ProcessStatus, StopOutcome};
#[cfg(any(windows, test))]
pub(crate) mod screenshots;
#[cfg(any(windows, test))]
mod workspace;
#[cfg(any(windows, test))]
mod world_packs;

mod validation;
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Serialize)]
pub struct ApplicationInfo {
    /// Windows package version, not a Java game version or compatibility tag.
    pub version: String,
    pub can_launch: bool,
}

#[derive(Debug, Serialize)]
pub struct BedrockStatus {
    pub supported: bool,
    pub game: Option<ApplicationInfo>,
    pub launcher: Option<ApplicationInfo>,
    pub game_running: bool,
    pub launch: LaunchDiagnostic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unsupported,
    NotInstalled,
    NeedsRepair,
    InvalidFile,
    FileTooLarge,
    DetectionFailed,
    LaunchFailed,
    ImportFailed,
    StoreFailed,
    DataUnavailable,
    InvalidPath,
    GameRunning,
    MissingDependency,
    Conflict,
    InsufficientSpace,
}

#[derive(Debug, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct BedrockError {
    pub code: ErrorCode,
    pub message: String,
}

impl BedrockError {
    fn new(code: ErrorCode, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
        }
    }
}

pub type Result<T> = std::result::Result<T, BedrockError>;

#[cfg(windows)]
fn data_roots() -> Result<Vec<DataRoot>> {
    windows::ensure_game()?;
    let (roaming, local) = user_data_dirs()?;
    workspace::discover(&roaming, &local).map_err(data_error)
}

#[cfg(windows)]
fn user_data_dirs() -> Result<(PathBuf, PathBuf)> {
    // Query Windows Known Folders; task runners can filter APPDATA variables.
    let roaming = dirs::data_dir().ok_or_else(|| {
        BedrockError::new(
            ErrorCode::DataUnavailable,
            "Windows data directory is unavailable",
        )
    })?;
    let local = dirs::data_local_dir().ok_or_else(|| {
        BedrockError::new(
            ErrorCode::DataUnavailable,
            "Windows data directory is unavailable",
        )
    })?;
    Ok((roaming, local))
}

#[cfg(windows)]
fn data_error(error: std::io::Error) -> BedrockError {
    BedrockError::new(
        if error.kind() == std::io::ErrorKind::InvalidInput {
            ErrorCode::InvalidPath
        } else {
            ErrorCode::DataUnavailable
        },
        error,
    )
}

#[cfg(windows)]
fn data_root(id: &str) -> Result<DataRoot> {
    data_roots()?
        .into_iter()
        .find(|root| root.id == id)
        .ok_or_else(|| {
            BedrockError::new(
                ErrorCode::InvalidPath,
                "Refresh detected Minecraft folders",
            )
        })
}

pub async fn get_workspace() -> Result<Workspace> {
    get_workspace_cached(true).await
}
pub async fn get_workspace_cached(refresh: bool) -> Result<Workspace> {
    #[cfg(windows)]
    {
        on_windows(move || {
            workspace::cached_snapshot(data_roots()?, refresh)
                .map_err(data_error)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = refresh;
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn read_file(root_id: String, path: String) -> Result<Document> {
    #[cfg(windows)]
    {
        on_windows(move || manage::read(&data_root(&root_id)?, &path)).await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, path);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}
pub async fn write_file(
    root_id: String,
    path: String,
    text: String,
    revision: String,
) -> Result<Document> {
    #[cfg(windows)]
    {
        on_windows(move || {
            manage::write(&data_root(&root_id)?, &path, &text, &revision)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, path, text, revision);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}
pub async fn delete_item(root_id: String, path: String) -> Result<Recovery> {
    #[cfg(windows)]
    {
        on_windows(move || {
            let roots = data_roots()?;
            let root =
                roots.iter().find(|r| r.id == root_id).ok_or_else(|| {
                    BedrockError::new(
                        ErrorCode::InvalidPath,
                        "Refresh detected folders",
                    )
                })?;
            manage::remove(&roots, root, &path)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, path);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}
pub async fn list_recoveries() -> Result<Vec<Recovery>> {
    #[cfg(windows)]
    {
        on_windows(move || {
            let mut result = Vec::new();
            for root in
                data_roots()?.iter().filter(|r| r.kind != RootKind::Logs)
            {
                result.extend(manage::recoveries(root)?);
                result.extend(world_packs::recoveries(root, false)?);
            }
            result.sort_by_key(|item| std::cmp::Reverse(item.saved_at));
            Ok(result)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}
pub async fn restore_item(root_id: String, id: String) -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(move || {
            let roots = data_roots()?;
            let root =
                roots.iter().find(|r| r.id == root_id).ok_or_else(|| {
                    BedrockError::new(
                        ErrorCode::InvalidPath,
                        "Refresh detected folders",
                    )
                })?;
            if id.starts_with("pack:") {
                world_packs::restore(root, &id)
            } else {
                manage::restore(&roots, root, &id)
            }
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, id);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn list_files(
    root_id: String,
    path: String,
) -> Result<DirectoryListing> {
    #[cfg(windows)]
    {
        on_windows(move || {
            workspace::list_files(&data_root(&root_id)?, &path)
                .map_err(data_error)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, path);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn read_log(root_id: String, path: String) -> Result<LogText> {
    #[cfg(windows)]
    {
        on_windows(move || {
            workspace::read_log(&data_root(&root_id)?, &path)
                .map_err(data_error)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, path);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn folder_path(root_id: String, path: String) -> Result<PathBuf> {
    #[cfg(windows)]
    {
        on_windows(move || {
            let path = workspace::resolve(&data_root(&root_id)?, &path)
                .map_err(data_error)?;
            if !path.is_dir() {
                return Err(BedrockError::new(
                    ErrorCode::InvalidPath,
                    "Select a Minecraft folder",
                ));
            }
            Ok(dunce::simplified(&path).to_path_buf())
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, path);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn open_updates() -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(windows::open_updates).await
    }
    #[cfg(not(windows))]
    {
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

#[cfg(windows)]
async fn on_windows<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tokio::task::spawn_blocking(move || {
        // Each blocking worker initializes and balances its own WinRT apartment.
        let _apartment = windows::Apartment::new()?;
        operation()
    })
    .await
    .map_err(|error| BedrockError::new(ErrorCode::LaunchFailed, error))?
}

pub async fn get_status() -> Result<BedrockStatus> {
    get_status_cached(true).await
}
pub async fn get_status_cached(refresh: bool) -> Result<BedrockStatus> {
    #[cfg(windows)]
    {
        on_windows(move || windows::get_status(refresh)).await
    }
    #[cfg(not(windows))]
    {
        let _ = refresh;
        Ok(BedrockStatus {
            supported: false,
            game: None,
            launcher: None,
            game_running: false,
            launch: process::status().launch,
        })
    }
}

pub async fn launch_game() -> Result<()> {
    #[cfg(windows)]
    {
        let result = on_windows(|| {
            let _lock = coordination::mutation()?;
            manage::require_closed(manage::game_running())?;
            process::requested();
            match windows::launch(windows::GAME_FAMILY) {
                Ok(()) => {
                    process::accepted();
                    Ok(())
                }
                Err(e) => {
                    process::failed(e.message.clone());
                    Err(e)
                }
            }
        })
        .await;
        if result.is_ok() {
            process::monitor_exit();
        }
        result
    }
    #[cfg(not(windows))]
    {
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn launch_official_launcher() -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(|| {
            let _lock = coordination::mutation()?;
            windows::launch(windows::LAUNCHER_FAMILY)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

/// Stop only the Bedrock game, leaving Java instances and the launcher alone.
pub async fn stop_game(
    force_token: Option<String>,
) -> Result<process::StopOutcome> {
    #[cfg(windows)]
    {
        on_windows(move || process::stop(force_token)).await
    }
    #[cfg(not(windows))]
    {
        let _ = force_token;
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn open_store() -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(windows::open_store).await
    }
    #[cfg(not(windows))]
    {
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn import_file(path: PathBuf) -> Result<()> {
    #[cfg(windows)]
    {
        let result = on_windows(move || {
            let path =
                validation::validate_import(&path).map_err(
                    |error| match error {
                        validation::ImportError::InvalidFile => {
                            BedrockError::new(
                                ErrorCode::InvalidFile,
                                "Select a local Bedrock archive",
                            )
                        }
                        validation::ImportError::FileTooLarge => {
                            BedrockError::new(
                                ErrorCode::FileTooLarge,
                                "Bedrock archive exceeds 2 GiB",
                            )
                        }
                    },
                )?;
            let _lock = coordination::mutation()?;
            process::requested();
            match windows::import_file(dunce::simplified(&path)) {
                Ok(()) => {
                    process::accepted();
                    Ok(())
                }
                Err(e) => {
                    process::failed(e.message.clone());
                    Err(e)
                }
            }
        })
        .await;
        if result.is_ok() {
            process::monitor_exit();
        }
        result
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

pub async fn get_process_status() -> Result<ProcessStatus> {
    Ok(process::status())
}
pub async fn list_recoveries_page(
    offset: usize,
    limit: usize,
) -> Result<RecoveryPage> {
    let items = list_recoveries().await?;
    let total = items.len();
    Ok(RecoveryPage {
        items: items
            .into_iter()
            .skip(offset)
            .take(limit.clamp(1, 200))
            .collect(),
        total,
        limited: false,
    })
}
pub async fn preview_recovery(
    root_id: String,
    id: String,
) -> Result<RecoveryPreview> {
    #[cfg(windows)]
    {
        on_windows(move || {
            let roots = data_roots()?;
            let root =
                roots.iter().find(|r| r.id == root_id).ok_or_else(|| {
                    BedrockError::new(
                        ErrorCode::InvalidPath,
                        "Refresh detected folders",
                    )
                })?;
            let mut all = manage::all_recoveries(root)?;
            all.extend(world_packs::recoveries(root, true)?);
            let recovery =
                all.into_iter().find(|r| r.id == id).ok_or_else(|| {
                    BedrockError::new(
                        ErrorCode::InvalidPath,
                        "Recovery no longer exists",
                    )
                })?;
            let check = if id.starts_with("pack:") {
                world_packs::preview(root, &id)
            } else {
                manage::preview(&roots, root, &id)
            };
            let conflicts =
                check.err().map(|e| vec![e.message]).unwrap_or_default();
            Ok(RecoveryPreview {
                recovery,
                can_restore: conflicts.is_empty(),
                conflicts,
            })
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, id);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}
pub async fn get_storage() -> Result<StorageSummary> {
    #[cfg(windows)]
    {
        let imports = crate::State::get()
            .await
            .map_err(|e| BedrockError::new(ErrorCode::DataUnavailable, e))?
            .directories
            .caches_dir()
            .join("bedrock-imports");
        on_windows(move || storage::summary(&data_roots()?, Some(&imports)))
            .await
    }
    #[cfg(not(windows))]
    {
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}
pub async fn remove_storage(root_id: String, id: String) -> Result<()> {
    #[cfg(windows)]
    {
        let imports = crate::State::get()
            .await
            .map_err(|e| BedrockError::new(ErrorCode::DataUnavailable, e))?
            .directories
            .caches_dir()
            .join("bedrock-imports");
        on_windows(move || {
            let _lock = coordination::mutation()?;
            manage::require_closed(manage::game_running())?;
            let roots = data_roots()?;
            let summary = storage::summary(&roots, Some(&imports))?;
            let selected = summary
                .entries
                .iter()
                .find(|entry| {
                    entry.root_id == root_id
                        && entry.id == id
                        && entry.removable
                })
                .ok_or_else(|| {
                    BedrockError::new(
                        ErrorCode::InvalidPath,
                        "Select an eligible recovery copy or completed import",
                    )
                })?;
            let path = if root_id == "imports" {
                if id.is_empty() || id.contains(['/', '\\', ':']) {
                    return Err(BedrockError::new(
                        ErrorCode::InvalidPath,
                        "Invalid import ID",
                    ));
                }
                let anchor = DataRoot {
                    id: "imports".into(),
                    path: imports,
                    kind: RootKind::User,
                };
                workspace::resolve(&anchor, &id).map_err(data_error)?
            } else {
                let root = roots.iter().find(|r| r.id == root_id).ok_or_else(
                    || {
                        BedrockError::new(
                            ErrorCode::InvalidPath,
                            "Refresh detected folders",
                        )
                    },
                )?;
                let relative = if id.starts_with("pack:") {
                    let parts: Vec<_> = id.split(':').collect();
                    if parts.len() != 3 {
                        return Err(BedrockError::new(
                            ErrorCode::InvalidPath,
                            "Invalid recovery ID",
                        ));
                    }
                    format!(
                        "minecraftWorlds/{}/.orbiont-pack-backups/{}",
                        parts[1], parts[2]
                    )
                } else {
                    format!(".orbiont-backups/{id}")
                };
                workspace::resolve(root, &relative).map_err(data_error)?
            };
            let _ = selected;
            storage::remove_tree(&path)
        })
        .await
    }
    #[cfg(not(windows))]
    {
        let _ = (root_id, id);
        Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ))
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn known_folders_without_appdata_environment() {
        const PROBE: &str = "ORBIONT_BEDROCK_FOLDER_PROBE";
        if std::env::var_os(PROBE).is_some() {
            assert!(std::env::var_os("APPDATA").is_none());
            assert!(std::env::var_os("LOCALAPPDATA").is_none());
            let (roaming, local) = user_data_dirs().unwrap();
            assert!(roaming.is_absolute() && roaming.is_dir());
            assert!(local.is_absolute() && local.is_dir());
            return;
        }
        // Isolate environment removal from all other parallel tests/threads.
        let probe = std::process::Command::new(
            std::env::current_exe().unwrap(),
        )
        .args([
            "--exact",
            "api::bedrock::tests::known_folders_without_appdata_environment",
            "--nocapture",
        ])
        .env_remove("APPDATA")
        .env_remove("LOCALAPPDATA")
        .env(PROBE, "1")
        .output()
        .unwrap();
        assert!(
            probe.status.success(),
            "{}{}",
            String::from_utf8_lossy(&probe.stdout),
            String::from_utf8_lossy(&probe.stderr)
        );
    }

    /// Read-only opt-in check against registered applications; never launches.
    #[tokio::test]
    #[ignore = "requires an installed official Minecraft for Windows package"]
    async fn detects_installed_official_game_without_launching() {
        let status = get_status().await.unwrap();
        assert!(status.supported);
        let game = status.game.expect("Minecraft for Windows is installed");
        assert!(game.can_launch);
        assert!(!game.version.is_empty());
        println!("Minecraft for Windows package: {}", game.version);
    }

    #[tokio::test]
    #[ignore = "reads documented data folders of the installed official game"]
    async fn reads_real_workspace_without_modifying_game_data() {
        let workspace = get_workspace().await.unwrap();
        assert!(!workspace.roots.is_empty());
        let root = workspace
            .roots
            .iter()
            .find(|root| root.kind != RootKind::Logs)
            .unwrap();
        list_files(root.id.clone(), String::new()).await.unwrap();
        assert!(
            list_files(root.id.clone(), "../outside".into())
                .await
                .is_err()
        );
        assert!(
            folder_path("arbitrary-root".into(), String::new())
                .await
                .is_err()
        );
        println!(
            "Detected data locations: {}; local content items: {}; incomplete: {}",
            workspace.roots.len(),
            workspace.items.len(),
            workspace.incomplete
        );
    }

    #[tokio::test]
    #[ignore = "opens the installed official Minecraft for Windows game"]
    async fn launches_installed_official_game() {
        launch_game().await.unwrap();
    }

    /// Windows acceptance is separate from the import result shown by Minecraft.
    #[tokio::test]
    #[ignore = "opens Minecraft with the local archive in BEDROCK_TEST_IMPORT"]
    async fn submits_selected_local_archive_to_official_game() {
        let path = std::env::var_os("BEDROCK_TEST_IMPORT")
            .expect("set BEDROCK_TEST_IMPORT to a local Bedrock archive");
        import_file(PathBuf::from(path)).await.unwrap();
    }
}
