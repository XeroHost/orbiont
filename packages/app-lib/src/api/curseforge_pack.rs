//! Converts a CurseForge modpack zip (`manifest.json` + overrides) into an
//! `.mrpack`, so CurseForge modpacks install through the one native modpack
//! installer instead of a second, provider-specific one.
//!
//! The manifest only lists `projectID`/`fileID` pairs; they're resolved with
//! CurseForge's batch lookups through the catalog proxy (two requests for the
//! whole pack). Files whose author disabled third-party distribution have no
//! download URL: they're left out and reported back so the UI can point the
//! user at CurseForge for them.

use crate::api::orbiont::curseforge_api_post;
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

/// A manifest file that couldn't be included in the converted pack.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFile {
    pub name: String,
    pub page_url: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConvertedPack {
    /// Path of a pack the native installer can read (`.mrpack`).
    pub path: PathBuf,
    /// Required files that were left out (distribution disabled by the author).
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

        let Some(file) = files.get(&entry.file_id) else {
            skipped.push(SkippedFile { name, page_url });
            continue;
        };
        let (Some(url), Some(sha1)) = (
            file.download_url.as_deref(),
            file.hashes.iter().find(|h| h.algo == 1),
        ) else {
            skipped.push(SkippedFile { name, page_url });
            continue;
        };

        let folder = folder_for_class(project.and_then(|m| m.class_id));
        index_files.push(json!({
            "path": format!("{folder}/{}", file.file_name),
            "hashes": { "sha1": sha1.value },
            "downloads": [url],
            "fileSize": file.file_length,
        }));
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
        "files": index_files,
        "dependencies": dependencies,
    });

    let output = write_mrpack(&reader, path, &manifest, &index).await?;
    Ok(ConvertedPack {
        path: output,
        skipped,
    })
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
    crate::util::io::create_dir_all(&dir).await?;
    let stem = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "modpack".to_string());
    let output = dir.join(format!("{}.mrpack", sanitize(&stem)));

    let file = tokio::fs::File::create(&output).await?;
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
