use super::{ApplicationInfo, BedrockError, BedrockStatus, ErrorCode, Result};
use ::windows::{
    ApplicationModel::{Core::AppListEntry, Package},
    Foundation::Uri,
    Management::Deployment::PackageManager,
    Storage::StorageFile,
    System::{Launcher, LauncherOptions},
    Win32::System::WinRT::{
        RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
    },
    core::HSTRING,
};
use std::{
    path::Path,
    sync::Mutex,
    time::{Duration, Instant},
};

pub(super) const GAME_FAMILY: &str = "Microsoft.MinecraftUWP_8wekyb3d8bbwe";
pub(super) const LAUNCHER_FAMILY: &str =
    "Microsoft.4297127D64EC6_8wekyb3d8bbwe";

pub(super) struct Apartment;
impl Apartment {
    pub(super) fn new() -> Result<Self> {
        // SAFETY: initialization and uninitialization take place on this worker.
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(|error| {
            BedrockError::new(ErrorCode::DetectionFailed, error)
        })?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: a successful RoInitialize is balanced on its original thread.
        unsafe { RoUninitialize() };
    }
}

fn find(
    family: &str,
) -> ::windows::core::Result<Option<(Package, Option<AppListEntry>)>> {
    let packages = PackageManager::new()?
        .FindPackagesByUserSecurityIdPackageFamilyName(
            &HSTRING::new(),
            &HSTRING::from(family),
        )?;
    for package in packages {
        // Only a registered retail package for the current user is considered.
        if package.IsResourcePackage()? || package.IsFramework()? {
            continue;
        }
        let entries = package.GetAppListEntriesAsync()?.get()?;
        return Ok(Some((package, entries.into_iter().next())));
    }
    Ok(None)
}

fn info(family: &str) -> ::windows::core::Result<Option<ApplicationInfo>> {
    let Some((package, entry)) = find(family)? else {
        return Ok(None);
    };
    let version = package.Id()?.Version()?;
    Ok(Some(ApplicationInfo {
        version: format!(
            "{}.{}.{}.{}",
            version.Major, version.Minor, version.Build, version.Revision
        ),
        can_launch: entry.is_some() && package.Status()?.VerifyIsOK()?,
    }))
}

type InstallationCache =
    Option<(Instant, Option<ApplicationInfo>, Option<ApplicationInfo>)>;
static INSTALLATION: Mutex<InstallationCache> = Mutex::new(None);
pub(super) fn get_status(refresh: bool) -> Result<BedrockStatus> {
    let mut cache = INSTALLATION
        .lock()
        .map_err(|e| BedrockError::new(ErrorCode::DetectionFailed, e))?;
    if refresh
        || cache.as_ref().is_none_or(|(time, _, _)| {
            time.elapsed() >= Duration::from_secs(60)
        })
    {
        let game = info(GAME_FAMILY)
            .map_err(|e| BedrockError::new(ErrorCode::DetectionFailed, e))?;
        let launcher = info(LAUNCHER_FAMILY)
            .map_err(|e| BedrockError::new(ErrorCode::DetectionFailed, e))?;
        *cache = Some((Instant::now(), game, launcher));
    }
    let (_, game, launcher) = cache.as_ref().unwrap();
    let process = super::process::status();
    Ok(BedrockStatus {
        supported: true,
        game: game.clone(),
        launcher: launcher.clone(),
        game_running: process.game_running,
        launch: process.launch,
    })
}

fn ready_entry(family: &str) -> Result<AppListEntry> {
    let (package, entry) = find(family)
        .map_err(|error| BedrockError::new(ErrorCode::DetectionFailed, error))?
        .ok_or_else(|| {
            BedrockError::new(
                ErrorCode::NotInstalled,
                "Official application is not installed",
            )
        })?;
    if !package
        .Status()
        .and_then(|status| status.VerifyIsOK())
        .map_err(|error| BedrockError::new(ErrorCode::DetectionFailed, error))?
    {
        return Err(BedrockError::new(
            ErrorCode::NeedsRepair,
            "Repair the official installation",
        ));
    }
    entry.ok_or_else(|| {
        BedrockError::new(ErrorCode::NeedsRepair, "No registered launch entry")
    })
}

pub(super) fn launch(family: &str) -> Result<()> {
    let entry = ready_entry(family)?;
    let launched = entry
        .LaunchAsync()
        .and_then(|operation| operation.get())
        .map_err(|error| BedrockError::new(ErrorCode::LaunchFailed, error))?;
    if !launched {
        return Err(BedrockError::new(
            ErrorCode::LaunchFailed,
            "Windows rejected the launch",
        ));
    }
    Ok(())
}

pub(super) fn ensure_game() -> Result<()> {
    let status = get_status(false)?;
    match status.game {
        Some(game) if game.can_launch => Ok(()),
        Some(_) => Err(BedrockError::new(
            ErrorCode::NeedsRepair,
            "Repair the official installation",
        )),
        None => Err(BedrockError::new(
            ErrorCode::NotInstalled,
            "Official application is not installed",
        )),
    }
}

pub(super) fn open_updates() -> Result<()> {
    let url = env!("ORBIONT_BEDROCK_UPDATES_URI");
    let parsed = url::Url::parse(url)
        .map_err(|error| BedrockError::new(ErrorCode::StoreFailed, error))?;
    if parsed.scheme() != "ms-windows-store"
        || parsed.host_str() != Some("downloadsandupdates")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return Err(BedrockError::new(
            ErrorCode::StoreFailed,
            "Configure the official Microsoft Store updates URI",
        ));
    }
    let launched = Uri::CreateUri(&HSTRING::from(url))
        .and_then(|uri| Launcher::LaunchUriAsync(&uri))
        .and_then(|operation| operation.get());
    match launched {
        Ok(true) => Ok(()),
        _ => open_store(),
    }
}

pub(super) fn import_file(path: &Path) -> Result<()> {
    ready_entry(GAME_FAMILY)?;
    let deliver = || -> ::windows::core::Result<bool> {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(
            path.as_os_str(),
        ))?
        .get()?;
        let options = LauncherOptions::new()?;
        options.SetTargetApplicationPackageFamilyName(&HSTRING::from(
            GAME_FAMILY,
        ))?;
        Launcher::LaunchFileWithOptionsAsync(&file, &options)?.get()
    };
    if !deliver()
        .map_err(|error| BedrockError::new(ErrorCode::ImportFailed, error))?
    {
        return Err(BedrockError::new(
            ErrorCode::ImportFailed,
            "Windows rejected the file activation",
        ));
    }
    Ok(())
}

pub(super) fn open_store() -> Result<()> {
    let native_url = env!("ORBIONT_BEDROCK_STORE_URI");
    let native = url::Url::parse(native_url)
        .map_err(|error| BedrockError::new(ErrorCode::StoreFailed, error))?;
    let url = env!("ORBIONT_BEDROCK_STORE_URL");
    let parsed = url::Url::parse(url)
        .map_err(|error| BedrockError::new(ErrorCode::StoreFailed, error))?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some("apps.microsoft.com")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
    {
        return Err(BedrockError::new(
            ErrorCode::StoreFailed,
            "Configure the official Microsoft Store HTTPS URL",
        ));
    }
    let product = parsed
        .path_segments()
        .and_then(|mut segments| segments.next_back());
    let query: Vec<_> = native.query_pairs().collect();
    if native.scheme() != "ms-windows-store"
        || native.host_str() != Some("pdp")
        || !native.username().is_empty()
        || native.password().is_some()
        || native.port().is_some()
        || native.fragment().is_some()
        || !matches!(native.path(), "" | "/")
        || query.len() != 1
        || query[0].0 != "ProductId"
        || Some(query[0].1.as_ref()) != product
    {
        return Err(BedrockError::new(
            ErrorCode::StoreFailed,
            "Configure the official Minecraft Store URI",
        ));
    }
    if matches!(
        Uri::CreateUri(&HSTRING::from(native_url))
            .and_then(|uri| Launcher::LaunchUriAsync(&uri))
            .and_then(|operation| operation.get()),
        Ok(true)
    ) {
        return Ok(());
    }
    let opened = Uri::CreateUri(&HSTRING::from(url))
        .and_then(|uri| Launcher::LaunchUriAsync(&uri))
        .and_then(|operation| operation.get())
        .map_err(|error| BedrockError::new(ErrorCode::StoreFailed, error))?;
    if !opened {
        return Err(BedrockError::new(
            ErrorCode::StoreFailed,
            "Windows could not open Microsoft Store",
        ));
    }
    Ok(())
}
