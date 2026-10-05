//! User-supplied OptiFine installers and per-instance launch components.
use crate::data::ModLoader;
use crate::state::content_store::input;
use crate::util::io;
use daedalus::minecraft::{Argument, ArgumentType, Library, VersionInfo};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const DOWNLOADS_URL: &str = env!("ORBIONT_OPTIFINE_DOWNLOADS_URL");
pub(crate) const REFERENCE_PATH: &str = ".orbiont/optifine.json";
const MAX_INSTALLER_BYTES: u64 = 128 * 1024 * 1024;
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptifineReference {
    pub minecraft_version: String,
    pub version: String,
    pub installer_sha256: String,
}

impl OptifineReference {
    pub fn validate(&self) -> crate::Result<()> {
        let safe = |value: &str| {
            !value.is_empty()
                && value.len() <= 100
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || b"._-".contains(&byte)
                })
        };
        if !safe(&self.minecraft_version)
            || !safe(&self.version)
            || self.installer_sha256.len() != 64
            || !self
                .installer_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(input("Invalid OptiFine component metadata"));
        }
        Ok(())
    }

    pub fn check_compatibility(
        &self,
        game: &str,
        loader: ModLoader,
    ) -> crate::Result<()> {
        self.validate()?;
        if loader != ModLoader::Vanilla {
            return Err(input(
                "OptiFine currently requires a Vanilla instance",
            ));
        }
        if self.minecraft_version != game {
            return Err(input(format!(
                "OptiFine {} requires Minecraft {}, selected {game}",
                self.version, self.minecraft_version
            )));
        }
        Ok(())
    }
}

// Parse field ConstantValue attributes, rather than trusting the filename or
// the order of UTF-8 strings in the constant pool. Inspection executes no Java.
fn class_constants(bytes: &[u8]) -> crate::Result<HashMap<String, String>> {
    let malformed = || input("Unrecognized OptiFine class metadata");
    let mut cursor = Cursor::new(bytes);
    fn read<const N: usize>(
        cursor: &mut Cursor<&[u8]>,
    ) -> crate::Result<[u8; N]> {
        let mut value = [0; N];
        cursor
            .read_exact(&mut value)
            .map_err(|_| input("Truncated OptiFine class metadata"))?;
        Ok(value)
    }
    fn u16(cursor: &mut Cursor<&[u8]>) -> crate::Result<u16> {
        Ok(u16::from_be_bytes(read(cursor)?))
    }
    fn skip(cursor: &mut Cursor<&[u8]>, count: u64) -> crate::Result<()> {
        let end = cursor
            .position()
            .checked_add(count)
            .filter(|end| *end <= cursor.get_ref().len() as u64)
            .ok_or_else(|| input("Truncated OptiFine class metadata"))?;
        cursor.set_position(end);
        Ok(())
    }
    if read::<4>(&mut cursor)? != [0xca, 0xfe, 0xba, 0xbe] {
        return Err(malformed());
    }
    skip(&mut cursor, 4)?;
    let count = u16(&mut cursor)? as usize;
    let mut utf8 = HashMap::new();
    let mut strings = HashMap::new();
    let mut index = 1;
    while index < count {
        match read::<1>(&mut cursor)?[0] {
            1 => {
                let size = u16(&mut cursor)? as usize;
                let mut value = vec![0; size];
                cursor.read_exact(&mut value).map_err(|_| malformed())?;
                utf8.insert(
                    index as u16,
                    String::from_utf8_lossy(&value).into_owned(),
                );
            }
            8 => {
                strings.insert(index as u16, u16(&mut cursor)?);
            }
            7 | 16 | 19 | 20 => skip(&mut cursor, 2)?,
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => skip(&mut cursor, 4)?,
            5 | 6 => {
                skip(&mut cursor, 8)?;
                index += 1;
            }
            15 => skip(&mut cursor, 3)?,
            _ => return Err(malformed()),
        }
        index += 1;
    }
    skip(&mut cursor, 6)?;
    let interfaces = u16(&mut cursor)?;
    skip(&mut cursor, interfaces as u64 * 2)?;
    let fields = u16(&mut cursor)?;
    let mut result = HashMap::new();
    for _ in 0..fields {
        skip(&mut cursor, 2)?;
        let name = u16(&mut cursor)?;
        skip(&mut cursor, 2)?;
        let attributes = u16(&mut cursor)?;
        for _ in 0..attributes {
            let attribute = u16(&mut cursor)?;
            let size = u32::from_be_bytes(read(&mut cursor)?) as u64;
            if utf8
                .get(&attribute)
                .is_some_and(|name| name == "ConstantValue")
                && size == 2
            {
                let value = u16(&mut cursor)?;
                if let Some(value) =
                    strings.get(&value).and_then(|value| utf8.get(value))
                    && let Some(name) = utf8.get(&name)
                {
                    result.insert(name.clone(), value.clone());
                }
            } else {
                skip(&mut cursor, size)?;
            }
        }
    }
    Ok(result)
}

fn zip_entry(
    bytes: &[u8],
    name: &str,
    limit: u64,
) -> crate::Result<Option<Vec<u8>>> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| input("Invalid OptiFine JAR"))?;
    let mut entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(_) => return Err(input("Invalid OptiFine JAR entry")),
    };
    if entry.size() > limit {
        return Err(input("OptiFine JAR entry exceeds its size limit"));
    }
    let mut data = Vec::new();
    entry.by_ref().take(limit + 1).read_to_end(&mut data)?;
    if data.len() as u64 > limit {
        return Err(input("OptiFine JAR entry exceeds its size limit"));
    }
    Ok(Some(data))
}

fn inspect_bytes(bytes: &[u8]) -> crate::Result<OptifineReference> {
    if bytes.len() as u64 > MAX_INSTALLER_BYTES {
        return Err(input("OptiFine installer exceeds 128 MiB"));
    }
    let mut config = None;
    for name in [
        "Config.class",
        "net/optifine/Config.class",
        "notch/net/optifine/Config.class",
    ] {
        if let Some(bytes) = zip_entry(bytes, name, 2 * 1024 * 1024)? {
            config = Some(class_constants(&bytes)?);
            break;
        }
    }
    let config =
        config.ok_or_else(|| input("This JAR is not an OptiFine installer"))?;
    let field = |key: &str| {
        config
            .get(key)
            .cloned()
            .ok_or_else(|| input("Incomplete OptiFine class metadata"))
    };
    let reference = OptifineReference {
        minecraft_version: field("MC_VERSION")?,
        version: format!("{}_{}", field("OF_EDITION")?, field("OF_RELEASE")?),
        installer_sha256: format!("{:x}", Sha256::digest(bytes)),
    };
    reference.validate()?;
    if zip_entry(bytes, "optifine/Patcher.class", 2 * 1024 * 1024)?.is_none()
        && zip_entry(bytes, "optifine/OptiFineTweaker.class", 2 * 1024 * 1024)?
            .is_none()
    {
        return Err(input(
            "OptiFine installer does not contain a supported patcher or tweaker",
        ));
    }
    Ok(reference)
}

async fn read_installer(path: &Path) -> crate::Result<Vec<u8>> {
    let file = tokio::fs::File::open(path).await?;
    if !file.metadata().await?.is_file() {
        return Err(input("Select an OptiFine JAR file"));
    }
    let mut bytes = Vec::new();
    tokio::io::AsyncReadExt::read_to_end(
        &mut tokio::io::AsyncReadExt::take(file, MAX_INSTALLER_BYTES + 1),
        &mut bytes,
    )
    .await?;
    if bytes.len() as u64 > MAX_INSTALLER_BYTES {
        return Err(input("OptiFine installer exceeds 128 MiB"));
    }
    Ok(bytes)
}

pub async fn inspect_installer(
    path: PathBuf,
) -> crate::Result<OptifineReference> {
    let bytes = read_installer(&path).await?;
    tokio::task::spawn_blocking(move || inspect_bytes(&bytes)).await?
}

pub async fn get_instance_reference(
    instance_id: &str,
) -> crate::Result<Option<OptifineReference>> {
    read_reference(&crate::instance::get_full_path(instance_id).await?).await
}

pub(crate) async fn read_reference(
    instance_path: &Path,
) -> crate::Result<Option<OptifineReference>> {
    let path = instance_path.join(REFERENCE_PATH);
    match tokio::fs::File::open(&path).await {
        Ok(file) => {
            let mut bytes = Vec::new();
            tokio::io::AsyncReadExt::read_to_end(
                &mut tokio::io::AsyncReadExt::take(file, 4097),
                &mut bytes,
            )
            .await?;
            if bytes.len() > 4096 {
                return Err(input("Invalid OptiFine component metadata"));
            }
            let reference: OptifineReference = serde_json::from_slice(&bytes)?;
            reference.validate()?;
            Ok(Some(reference))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub(crate) async fn write_reference(
    instance_path: &Path,
    reference: Option<&OptifineReference>,
) -> crate::Result<()> {
    let path = instance_path.join(REFERENCE_PATH);
    if let Some(reference) = reference {
        reference.validate()?;
        io::create_dir_all(path.parent().unwrap()).await?;
        io::write(path, serde_json::to_vec_pretty(reference)?).await?;
    } else if let Err(error) = tokio::fs::remove_file(path).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        return Err(error.into());
    }
    Ok(())
}

fn library_path(
    libraries: &Path,
    artifact: &str,
    reference: &OptifineReference,
) -> PathBuf {
    libraries
        .join("optifine")
        .join(artifact)
        .join(&reference.installer_sha256)
        .join(format!("{artifact}-{}.jar", reference.installer_sha256))
}

pub async fn cache_installer(
    path: &Path,
    game: &str,
    loader: ModLoader,
    expected: Option<&OptifineReference>,
) -> crate::Result<OptifineReference> {
    let bytes = read_installer(path).await?;
    let reference = inspect_bytes(&bytes)?;
    reference.check_compatibility(game, loader)?;
    if expected.is_some_and(|expected| expected != &reference) {
        return Err(input(
            "Select the exact OptiFine installer required by this Orbpack",
        ));
    }
    let state = crate::State::get().await?;
    let cached = library_path(
        &state.directories.libraries_dir(),
        "orbiont-installer",
        &reference,
    );
    io::create_dir_all(cached.parent().unwrap()).await?;
    io::write(cached, &bytes).await?;
    Ok(reference)
}

pub async fn require_cached(
    reference: &OptifineReference,
) -> crate::Result<()> {
    reference.validate()?;
    let state = crate::State::get().await?;
    let path = library_path(
        &state.directories.libraries_dir(),
        "orbiont-installer",
        reference,
    );
    let info = inspect_installer(path).await.map_err(|_| {
        input("Select the official OptiFine JAR required by this Orbpack")
    })?;
    if info != *reference {
        return Err(input(
            "Cached OptiFine installer failed its integrity check",
        ));
    }
    Ok(())
}

pub(crate) async fn is_optifine_binary(path: &Path) -> crate::Result<bool> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(path)?;
        let Ok(archive) = zip::ZipArchive::new(file) else {
            return Ok(false);
        };
        let found = archive.file_names().any(|name| {
            name == "optifine/OptiFineTweaker.class"
                || name == "optifine/Patcher.class"
        });
        Ok(found)
    })
    .await?
}

#[derive(Serialize, Deserialize)]
struct ArtifactHashes {
    optifine: String,
    wrapper: String,
}

async fn digest_file(path: &Path) -> crate::Result<String> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    loop {
        let size = file.read(&mut buffer).await?;
        if size == 0 {
            break;
        }
        hasher.update(&buffer[..size]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub(crate) async fn ensure_installed(
    instance_path: &Path,
    game: &str,
    loader: ModLoader,
    client: &Path,
    java: &Path,
    repairing: bool,
) -> crate::Result<Option<OptifineReference>> {
    let Some(reference) = read_reference(instance_path).await? else {
        return Ok(None);
    };
    reference.check_compatibility(game, loader)?;
    let _guard = INSTALL_LOCK.lock().await;
    let state = crate::State::get().await?;
    ensure_artifacts(
        &state.directories.libraries_dir(),
        &reference,
        client,
        java,
        repairing,
    )
    .await?;
    Ok(Some(reference))
}

async fn ensure_artifacts(
    libraries: &Path,
    reference: &OptifineReference,
    client: &Path,
    java: &Path,
    repairing: bool,
) -> crate::Result<()> {
    let patched = library_path(libraries, "orbiont", reference);
    let wrapper = library_path(libraries, "orbiont-wrapper", reference);
    let hashes_path = patched.with_extension("json");
    if !repairing
        && let Ok(hashes) = tokio::fs::read(&hashes_path).await
        && let Ok(hashes) = serde_json::from_slice::<ArtifactHashes>(&hashes)
        && digest_file(&patched)
            .await
            .is_ok_and(|value| value == hashes.optifine)
        && digest_file(&wrapper)
            .await
            .is_ok_and(|value| value == hashes.wrapper)
    {
        return Ok(());
    }
    let installer = library_path(libraries, "orbiont-installer", reference);
    let bytes = read_installer(&installer).await.map_err(|_| input("OptiFine installer is missing from the local cache; select its official JAR again"))?;
    if inspect_bytes(&bytes)? != *reference {
        return Err(input(
            "Cached OptiFine installer failed its integrity check",
        ));
    }
    io::create_dir_all(patched.parent().unwrap()).await?;
    io::create_dir_all(wrapper.parent().unwrap()).await?;
    let work = tempfile::tempdir_in(patched.parent().unwrap())?;
    let output_path = work.path().join("optifine.jar");
    if zip_entry(&bytes, "optifine/Patcher.class", 2 * 1024 * 1024)?.is_some() {
        let mut command = tokio::process::Command::new(java);
        command
            .args(["-Xmx512M", "-cp"])
            .arg(&installer)
            .arg("optifine.Patcher")
            .arg(client)
            .arg(&installer)
            .arg(&output_path)
            .current_dir(work.path())
            .kill_on_drop(true);
        #[cfg(target_os = "windows")]
        command.creation_flags(0x08000000);
        let output =
            tokio::time::timeout(Duration::from_secs(180), command.output())
                .await
                .map_err(|_| input("OptiFine patcher timed out"))??;
        if !output.status.success() {
            return Err(input(format!(
                "OptiFine patcher failed: {}",
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(2000)
                    .collect::<String>()
            )));
        }
    } else {
        io::write(&output_path, &bytes).await?;
    }
    let output = read_installer(&output_path).await?;
    if zip_entry(&output, "optifine/OptiFineTweaker.class", 2 * 1024 * 1024)?
        .is_none()
    {
        return Err(input(
            "OptiFine patcher did not produce a supported launch component",
        ));
    }
    let wrapper_bytes = if let Some(version) =
        zip_entry(&bytes, "launchwrapper-of.txt", 128)?
    {
        let version = String::from_utf8_lossy(&version).trim().to_string();
        if version.is_empty()
            || !version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
        {
            return Err(input("Invalid OptiFine launchwrapper version"));
        }
        zip_entry(
            &bytes,
            &format!("launchwrapper-of-{version}.jar"),
            10 * 1024 * 1024,
        )?
        .ok_or_else(|| input("OptiFine launchwrapper is missing"))?
    } else if let Some(wrapper) =
        zip_entry(&bytes, "launchwrapper-2.0.jar", 10 * 1024 * 1024)?
    {
        wrapper
    } else {
        let mut response = reqwest::Client::new()
            .get(env!("ORBIONT_OPTIFINE_LEGACY_WRAPPER_URL"))
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;
        if response
            .content_length()
            .is_some_and(|size| size > 10 * 1024 * 1024)
        {
            return Err(input("Launchwrapper exceeds its size limit"));
        }
        let mut wrapper = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if wrapper.len() + chunk.len() > 10 * 1024 * 1024 {
                return Err(input("Launchwrapper exceeds its size limit"));
            }
            wrapper.extend_from_slice(&chunk);
        }
        wrapper
    };
    if zip_entry(
        &wrapper_bytes,
        "net/minecraft/launchwrapper/Launch.class",
        2 * 1024 * 1024,
    )?
    .is_none()
    {
        return Err(input("Invalid OptiFine launchwrapper"));
    }
    io::write(&patched, &output).await?;
    io::write(&wrapper, &wrapper_bytes).await?;
    io::write(
        hashes_path,
        serde_json::to_vec(&ArtifactHashes {
            optifine: format!("{:x}", Sha256::digest(&output)),
            wrapper: format!("{:x}", Sha256::digest(&wrapper_bytes)),
        })?,
    )
    .await?;
    Ok(())
}

pub(crate) fn apply_launch_patch(
    version: &mut VersionInfo,
    reference: &OptifineReference,
) {
    version.main_class = "net.minecraft.launchwrapper.Launch".into();
    let has_modern_game = version
        .arguments
        .as_ref()
        .is_some_and(|args| args.contains_key(&ArgumentType::Game));
    if let Some(legacy) = version
        .minecraft_arguments
        .as_mut()
        .filter(|_| !has_modern_game)
    {
        legacy.push_str(" --tweakClass optifine.OptiFineTweaker");
    } else {
        let args = version
            .arguments
            .get_or_insert_with(HashMap::new)
            .entry(ArgumentType::Game)
            .or_default();
        args.extend([
            Argument::Normal("--tweakClass".into()),
            Argument::Normal("optifine.OptiFineTweaker".into()),
        ]);
    }
    for artifact in ["orbiont", "orbiont-wrapper"] {
        version.libraries.insert(
            0,
            Library {
                name: format!(
                    "optifine:{artifact}:{}",
                    reference.installer_sha256
                ),
                downloads: None,
                extract: None,
                url: None,
                natives: None,
                rules: None,
                checksums: None,
                include_in_classpath: true,
                downloadable: false,
            },
        );
    }
}

#[cfg(test)]
mod tests;
