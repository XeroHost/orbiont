//! Integration with the registered, official Minecraft for Windows application.
//! Minecraft and Microsoft Store retain authentication, licensing and imports.

use serde::Serialize;
use std::path::PathBuf;

mod catalog;
pub use catalog::import_catalog_file;

mod data;
pub use data::*;
#[cfg(any(windows, test))]
mod manage;
#[cfg(any(windows, test))]
pub(crate) mod screenshots;
#[cfg(any(windows, test))]
mod workspace;
#[cfg(any(windows, test))]
mod world_packs;

mod validation;
#[cfg(windows)]
mod windows;

#[derive(Debug, Serialize)]
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
    #[cfg(windows)]
    {
        on_windows(|| workspace::snapshot(data_roots()?).map_err(data_error))
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
            manage::restore(&roots, root, &id)
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
    #[cfg(windows)]
    {
        on_windows(windows::get_status).await
    }
    #[cfg(not(windows))]
    {
        Ok(BedrockStatus {
            supported: false,
            game: None,
            launcher: None,
            game_running: false,
        })
    }
}

pub async fn launch_game() -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(|| {
            manage::require_closed(manage::game_running())?;
            windows::launch(windows::GAME_FAMILY)
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

pub async fn launch_official_launcher() -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(|| windows::launch(windows::LAUNCHER_FAMILY)).await
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
pub async fn stop_game() -> Result<()> {
    #[cfg(windows)]
    {
        on_windows(manage::stop_game).await
    }
    #[cfg(not(windows))]
    {
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
        on_windows(move || {
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
                                "Bedrock archive exceeds 8 GiB",
                            )
                        }
                    },
                )?;
            windows::import_file(dunce::simplified(&path))
        })
        .await
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
