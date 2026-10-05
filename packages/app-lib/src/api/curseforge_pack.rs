//! Converts a CurseForge modpack zip (`manifest.json` + overrides) into an
//! `.mrpack`, so CurseForge modpacks install through the one native modpack
//! installer instead of a second, provider-specific one.
//!
//! The manifest only lists `projectID`/`fileID` pairs; they're resolved with
//! CurseForge's batch lookups through the catalog proxy (two requests for the
//! whole pack). Files whose author disabled third-party distribution have no
//! download URL: they remain required in the index and are reported back so
//! the UI can request and verify an official browser download before installing.

use crate::api::orbiont::{curseforge_api_post, downloads};
use async_zip::tokio::read::fs::ZipFileReader;
use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "manifest.json";
const MRPACK_INDEX: &str = "modrinth.index.json";
const BATCH_SIZE: usize = 500;

// CurseForge class ids -> the instance folder that content type lives in.
fn folder_for_class(class_id: Option<u32>) -> &'static str {
    match class_id {
        Some(12) => "resourcepacks",
        Some(6552) => "shaderpacks",
        _ => "mods",
    }
}

#[derive(Deserialize)]
struct Manifest {
    minecraft: ManifestMinecraft,
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    #[serde(default)]
    files: Vec<ManifestFile>,
    overrides: Option<String>,
}

#[derive(Deserialize)]
struct ManifestMinecraft {
    version: String,
    #[serde(rename = "modLoaders", default)]
    mod_loaders: Vec<ManifestModLoader>,
}

#[derive(Deserialize)]
struct ManifestModLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct ManifestFile {
    #[serde(rename = "projectID")]
    project_id: u32,
    #[serde(rename = "fileID")]
    file_id: u32,
    #[serde(default = "default_true")]
    required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct CfFile {
    id: u32,
    #[serde(rename = "modId")]
    mod_id: u32,
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileLength")]
    file_length: u64,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    #[serde(rename = "isAvailable", default = "default_true")]
    is_available: bool,
    #[serde(default)]
    hashes: Vec<CfHash>,
}

#[derive(Deserialize)]
struct CfHash {
    value: String,
    algo: u8,
}

#[derive(Deserialize)]
struct CfMod {
    id: u32,
    name: String,
    #[serde(rename = "classId")]
    class_id: Option<u32>,
    links: Option<CfLinks>,
    #[serde(rename = "allowModDistribution")]
    allow_mod_distribution: Option<bool>,
}

#[derive(Deserialize)]
struct CfLinks {
    #[serde(rename = "websiteUrl")]
    website_url: Option<String>,
}

#[derive(Deserialize)]
struct CfList<T> {
    data: Vec<T>,
}

/// A required manifest file that needs a verified browser download.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub name: String,
    #[serde(flatten)]
    pub download: downloads::ManualDownload,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedPack {
    /// Path of a pack the native installer can read (`.mrpack`).
    pub path: PathBuf,
    /// Required files needing manual download before installation.
    pub skipped: Vec<SkippedFile>,
}

/// Converts `path` if it's a CurseForge modpack; returns it untouched if it's
/// already an `.mrpack`.
pub async fn convert_curseforge_pack(
    path: &Path,
) -> crate::Result<ConvertedPack> {
    let reader = ZipFileReader::new(path).await.map_err(|_| {
        crate::ErrorKind::InputError("Failed to read modpack zip".to_string())
    })?;
    let entries = reader.file().entries();
    let find = |name: &str| {
        entries
            .iter()
            .position(|e| e.filename().as_str().is_ok_and(|n| n == name))
    };

    if find(MRPACK_INDEX).is_some() {
        return Ok(ConvertedPack {
            path: path.to_path_buf(),
            skipped: Vec::new(),
        });
    }
    let Some(manifest_index) = find(MANIFEST) else {
        return Err(crate::ErrorKind::InputError(
            "This file isn't a modpack (no manifest.json or modrinth.index.json)"
                .to_string(),
        )
        .into());
    };

    let mut manifest_bytes = Vec::new();
    reader
        .reader_with_entry(manifest_index)
        .await?
        .read_to_end_checked(&mut manifest_bytes)
        .await?;
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;

    let required: Vec<&ManifestFile> =
        manifest.files.iter().filter(|f| f.required).collect();
    let files = fetch_files(&required).await?;
    let mods = fetch_mods(&required).await?;

    let mut index_files = Vec::new();
    let mut skipped = Vec::new();
    for entry in &required {
        let project = mods.get(&entry.project_id);
        let name = project.map(|m| m.name.clone()).unwrap_or_else(|| {
            format!("CurseForge project {}", entry.project_id)
        });
        let page_url = project
            .and_then(|m| m.links.as_ref())
            .and_then(|l| l.website_url.clone());

        let file = files.get(&entry.file_id).ok_or_else(|| {
            crate::ErrorKind::InputError(format!(
                "Required CurseForge file {}/{} is missing",
                entry.project_id, entry.file_id
            ))
        })?;
        let (index, manual) = pack_file_entry(entry, file, project)?;
        index_files.push(index);
        if manual {
            let sha1 = file
                .hashes
                .iter()
                .find(|hash| hash.algo == 1)
                .unwrap()
                .value
                .to_ascii_lowercase();
            skipped.push(SkippedFile {
                name,
                download: downloads::ManualDownload {
                    project_id: entry.project_id,
                    file_id: entry.file_id,
                    file_name: file.file_name.clone(),
                    file_size: file.file_length,
                    sha1,
                    page_url: page_url.and_then(|url| {
                        downloads::official_page(
                            &format!(
                                "{}/files/{}",
                                url.trim_end_matches('/'),
                                entry.file_id
                            ),
                            entry.file_id,
                        )
                    }),
                },
            });
        }
    }

    let mut dependencies = serde_json::Map::new();
    dependencies.insert(
        "minecraft".into(),
        manifest.minecraft.version.clone().into(),
    );
    let loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|l| l.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first());
    if let Some(loader) = loader
        && let Some((kind, version)) = loader.id.split_once('-')
    {
        let key = match kind {
            "forge" => Some("forge"),
            "neoforge" => Some("neoforge"),
            "fabric" => Some("fabric-loader"),
            "quilt" => Some("quilt-loader"),
            _ => None,
        };
        if let Some(key) = key {
            dependencies.insert(key.into(), version.into());
        }
    }

    let pack_name = manifest
        .name
        .clone()
        .unwrap_or_else(|| "Modpack".to_string());
    let index = json!({
        "game": "minecraft",
        "formatVersion": 1,
        "versionId": manifest.version.clone().unwrap_or_else(|| "1.0.0".to_string()),
        "name": pack_name,
        "summary": manifest.description,
        "files": index_files,
        "dependencies": dependencies,
    });

    let output = write_mrpack(&reader, path, &manifest, &index).await?;
    Ok(ConvertedPack {
        path: output,
        skipped,
    })
}

fn pack_file_entry(
    entry: &ManifestFile,
    file: &CfFile,
    project: Option<&CfMod>,
) -> crate::Result<(serde_json::Value, bool)> {
    if file.mod_id != entry.project_id || file.id != entry.file_id {
        return Err(crate::ErrorKind::InputError(
            "CurseForge manifest file/project mismatch".into(),
        )
        .into());
    }
    let sha1 = file
        .hashes
        .iter()
        .find(|hash| hash.algo == 1)
        .ok_or_else(|| {
            crate::ErrorKind::InputError(
                "Required CurseForge file has no SHA-1".into(),
            )
        })?
        .value
        .to_ascii_lowercase();
    crate::state::content_store::validate_digest(&sha1, 40)?;
    if file.file_name.is_empty()
        || file.file_name.contains(['/', '\\'])
        || matches!(file.file_name.as_str(), "." | "..")
        || file.file_length > u32::MAX as u64
    {
        return Err(crate::ErrorKind::InputError(
            "Invalid CurseForge pack file".into(),
        )
        .into());
    }
    let folder = folder_for_class(project.and_then(|mod_| mod_.class_id));
    Ok((
        json!({ "path": format!("{folder}/{}", file.file_name), "hashes": { "sha1": sha1 },
        "downloads": [downloads::download_url(entry.project_id, entry.file_id)?], "fileSize": file.file_length }),
        !file.is_available
            || file.download_url.is_none()
            || project
                .is_some_and(|mod_| mod_.allow_mod_distribution == Some(false)),
    ))
}

async fn fetch_files(
    required: &[&ManifestFile],
) -> crate::Result<HashMap<u32, CfFile>> {
    let mut out = HashMap::new();
    let ids: Vec<u32> = required.iter().map(|f| f.file_id).collect();
    for chunk in ids.chunks(BATCH_SIZE) {
        let body =
            curseforge_api_post("mods/files", &json!({ "fileIds": chunk }))
                .await?;
        let list: CfList<CfFile> = serde_json::from_value(body)?;
        out.extend(list.data.into_iter().map(|f| (f.id, f)));
    }
    // Guard against a file id that resolves to a different project.
    out.retain(|_, f| {
        required
            .iter()
            .any(|r| r.file_id == f.id && r.project_id == f.mod_id)
    });
    Ok(out)
}

async fn fetch_mods(
    required: &[&ManifestFile],
) -> crate::Result<HashMap<u32, CfMod>> {
    let mut out = HashMap::new();
    let mut ids: Vec<u32> = required.iter().map(|f| f.project_id).collect();
    ids.sort_unstable();
    ids.dedup();
    for chunk in ids.chunks(BATCH_SIZE) {
        let body =
            curseforge_api_post("mods", &json!({ "modIds": chunk })).await?;
        let list: CfList<CfMod> = serde_json::from_value(body)?;
        out.extend(list.data.into_iter().map(|m| (m.id, m)));
    }
    Ok(out)
}

/// Writes `modrinth.index.json` plus the pack's overrides (renamed to the
/// `overrides/` folder the native installer expects) next to the other
/// converted packs in the cache.
async fn write_mrpack(
    reader: &ZipFileReader,
    source: &Path,
    manifest: &Manifest,
    index: &serde_json::Value,
) -> crate::Result<PathBuf> {
    let state = crate::State::get().await?;
    let dir = state.directories.caches_dir().join("orbiont-curseforge");
    write_mrpack_in(reader, source, manifest, index, &dir).await
}

async fn write_mrpack_in(
    reader: &ZipFileReader,
    source: &Path,
    manifest: &Manifest,
    index: &serde_json::Value,
    dir: &Path,
) -> crate::Result<PathBuf> {
    crate::util::io::create_dir_all(dir).await?;
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "modpack".to_string());
    let stem: String = sanitize(&stem).chars().take(80).collect();
    let output = dir.join(format!("{stem}-{}.mrpack", uuid::Uuid::new_v4()));
    let (file, temporary) = tempfile::NamedTempFile::new_in(dir)?.into_parts();
    let file = tokio::fs::File::from_std(file);
    let mut writer = ZipFileWriter::with_tokio(file);
    writer
        .write_entry_whole(
            ZipEntryBuilder::new(
                MRPACK_INDEX.to_string().into(),
                Compression::Deflate,
            ),
            &serde_json::to_vec_pretty(index)?,
        )
        .await?;

    let overrides = format!(
        "{}/",
        manifest
            .overrides
            .as_deref()
            .unwrap_or("overrides")
            .trim_matches('/')
    );
    for (i, entry) in reader.file().entries().iter().enumerate() {
        let Ok(name) = entry.filename().as_str() else {
            continue;
        };
        let Some(relative) = name.strip_prefix(&overrides) else {
            continue;
        };
        if relative.is_empty() || name.ends_with('/') {
            continue;
        }
        let mut bytes = Vec::new();
        reader
            .reader_with_entry(i)
            .await?
            .read_to_end_checked(&mut bytes)
            .await?;
        writer
            .write_entry_whole(
                ZipEntryBuilder::new(
                    format!("overrides/{relative}").into(),
                    Compression::Deflate,
                ),
                &bytes,
            )
            .await?;
    }

    writer.close().await?;
    tokio::fs::OpenOptions::new()
        .write(true)
        .open(&temporary)
        .await?
        .sync_all()
        .await?;
    temporary.persist_noclobber(&output).map_err(|error| {
        crate::util::io::IOError::with_path(error.error, &output)
    })?;
    Ok(output)
}

fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn required_files_use_ids_including_restricted_files_and_reject_mismatches()
    {
        let entry = ManifestFile {
            project_id: 10,
            file_id: 100,
            required: true,
        };
        let mut file: CfFile = serde_json::from_value(json!({ "id": 100, "modId": 10, "fileName": "mod.jar", "fileLength": 3,
            "downloadUrl": "https://attacker.test/ignored.jar", "hashes": [{ "algo": 1, "value": "a9993e364706816aba3e25717850c26c9cd0d89d" }] })).unwrap();
        let (index, manual) = pack_file_entry(&entry, &file, None).unwrap();
        assert!(!manual);
        assert_eq!(
            index["downloads"][0],
            downloads::download_url(10, 100).unwrap()
        );
        file.download_url = None;
        let (index, manual) = pack_file_entry(&entry, &file, None).unwrap();
        assert!(manual);
        assert_eq!(index["path"], "mods/mod.jar");
        assert_eq!(index["fileSize"], 3);
        let serialized = serde_json::to_value(SkippedFile {
            name: "Mod".into(),
            download: downloads::ManualDownload {
                project_id: 10,
                file_id: 100,
                file_name: "mod.jar".into(),
                file_size: 3,
                sha1: file.hashes[0].value.clone(),
                page_url: None,
            },
        })
        .unwrap();
        assert_eq!(serialized["projectId"], 10);
        assert_eq!(serialized["fileId"], 100);
        assert!(serialized.get("download").is_none());
        file.mod_id = 11;
        assert!(pack_file_entry(&entry, &file, None).is_err());
        file.mod_id = 10;
        file.hashes.clear();
        assert!(pack_file_entry(&entry, &file, None).is_err());
    }

    fn source_pack(path: &Path, payload: &[u8]) {
        let mut archive =
            zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        archive
            .start_file(
                "overrides/config/settings.txt",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        archive.write_all(payload).unwrap();
        archive.finish().unwrap();
    }

    fn override_bytes(path: &Path) -> Vec<u8> {
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        let mut bytes = Vec::new();
        archive
            .by_name("overrides/config/settings.txt")
            .unwrap()
            .read_to_end(&mut bytes)
            .unwrap();
        bytes
    }

    #[tokio::test]
    async fn same_named_conversions_do_not_replace_packs_already_in_use() {
        let directory = tempfile::tempdir().unwrap();
        let cache = directory.path().join("cache");
        let manifest: Manifest = serde_json::from_value(json!({
            "minecraft": { "version": "1.21.1" },
            "name": "Pack", "version": "1",
        }))
        .unwrap();
        let index = json!({ "name": "Pack" });
        let mut outputs = Vec::new();
        for (folder, payload) in [
            ("first", b"first".as_slice()),
            ("second", b"second".as_slice()),
        ] {
            let source_dir = directory.path().join(folder);
            std::fs::create_dir(&source_dir).unwrap();
            let source = source_dir.join("pack.zip");
            source_pack(&source, payload);
            let reader = ZipFileReader::new(&source).await.unwrap();
            outputs.push(
                write_mrpack_in(&reader, &source, &manifest, &index, &cache)
                    .await
                    .unwrap(),
            );
        }
        assert_ne!(outputs[0], outputs[1]);
        assert_eq!(override_bytes(&outputs[0]), b"first");
        assert_eq!(override_bytes(&outputs[1]), b"second");
        assert!(outputs.iter().all(|path| {
            path.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("pack-")
        }));
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 2);

        // A corrupt source must not leave an incomplete cache file behind.
        let source = directory.path().join("corrupt.zip");
        source_pack(&source, b"payload to corrupt");
        let mut bytes = std::fs::read(&source).unwrap();
        let offset = bytes
            .windows(b"payload to corrupt".len())
            .position(|value| value == b"payload to corrupt")
            .unwrap();
        bytes[offset] ^= 1;
        std::fs::write(&source, bytes).unwrap();
        let reader = ZipFileReader::new(&source).await.unwrap();
        assert!(
            write_mrpack_in(&reader, &source, &manifest, &index, &cache)
                .await
                .is_err()
        );
        assert_eq!(std::fs::read_dir(&cache).unwrap().count(), 2);
        assert_eq!(override_bytes(&outputs[0]), b"first");
    }
}
