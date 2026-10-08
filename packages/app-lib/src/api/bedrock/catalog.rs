use super::{BedrockError, ErrorCode, Result};
use std::path::{Path, PathBuf};

fn invalid(message: impl ToString) -> BedrockError {
    BedrockError::new(ErrorCode::InvalidFile, message)
}

pub(super) fn prepare_archive(
    path: &Path,
    destination: &Path,
) -> Result<Vec<PathBuf>> {
    prepare_archive_bounded(path, destination, 0, &mut 0)
}
fn prepare_archive_bounded(
    path: &Path,
    destination: &Path,
    depth: usize,
    expanded: &mut u64,
) -> Result<Vec<PathBuf>> {
    use std::{fs::File, io::Read};
    if depth > 4 {
        return Err(invalid("Too many nested Bedrock archives"));
    }
    crate::util::archive::preflight_file(path).map_err(invalid)?;
    let mut archive = zip::ZipArchive::new(File::open(path).map_err(invalid)?)
        .map_err(invalid)?;
    if archive.len() > crate::util::archive::MAX_ARCHIVE_ENTRIES {
        return Err(invalid("Too many archive entries"));
    }
    let mut names = Vec::new();
    let mut total = 0u64;
    let mut seen = std::collections::HashSet::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(invalid)?;
        let name = entry.name().to_owned();
        crate::util::archive::validate_entry(&name, entry.size(), &mut total)
            .map_err(invalid)?;
        if entry.enclosed_name().is_none()
            || name.contains(['\\', ':'])
            || name.chars().any(char::is_control)
            || name.split('/').any(|part| matches!(part, "." | ".."))
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            || !seen.insert(name.to_lowercase())
        {
            return Err(invalid("Unsafe Bedrock archive path"));
        }
        if total > 8 * 1024 * 1024 * 1024 {
            return Err(BedrockError::new(
                ErrorCode::FileTooLarge,
                "Archive too large",
            ));
        }
        if !entry.is_dir() {
            names.push((index, name));
        }
    }
    if names.is_empty() {
        return Err(invalid("Empty archive"));
    }
    require_space(destination, total.saturating_mul(2))?;
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ["mcpack", "mcaddon", "mcworld"].contains(&extension.as_str()) {
        if expanded.saturating_add(total)
            > crate::util::archive::MAX_EXPANDED_BYTES
        {
            return Err(invalid(
                "Archive exceeds streaming decompression limit",
            ));
        }
        super::validation::validate_import(path)
            .map_err(|_| invalid("Invalid Bedrock archive"))?;
        *expanded += total;
        return Ok(vec![path.to_owned()]);
    }
    if extension != "zip" {
        return Err(invalid("Unsupported Bedrock format"));
    }
    let embedded: Vec<_> = names
        .iter()
        .filter(|(_, name)| {
            [".mcpack", ".mcaddon", ".mcworld"]
                .iter()
                .any(|suffix| name.to_ascii_lowercase().ends_with(suffix))
        })
        .collect();
    if !embedded.is_empty() {
        if embedded.len() > 16 {
            return Err(invalid("Too many embedded Bedrock files"));
        }
        let mut paths = Vec::new();
        for (number, (index, name)) in embedded.into_iter().enumerate() {
            let extension =
                Path::new(name).extension().unwrap().to_string_lossy();
            let output =
                destination.join(format!("content-{number}.{extension}"));
            let mut entry = archive.by_index(*index).map_err(invalid)?;
            let size = entry.size();
            let written = std::io::copy(
                &mut (&mut entry).take(
                    size.min(
                        crate::util::archive::MAX_EXPANDED_BYTES
                            .saturating_sub(*expanded),
                    ) + 1,
                ),
                &mut File::create(&output).map_err(invalid)?,
            )
            .map_err(invalid)?;
            charge_expanded(expanded, written)?;
            if written != size {
                return Err(invalid("Invalid embedded file size"));
            }
            // Validate all embedded archives before handing any of them to the game.
            prepare_archive_bounded(&output, destination, depth + 1, expanded)?;
            paths.push(output);
        }
        return Ok(paths);
    }
    let mut prefix = String::new();
    loop {
        let relative: Vec<_> = names
            .iter()
            .map(|(_, name)| &name[prefix.len()..])
            .collect();
        let Some((folder, _)) = relative[0].split_once('/') else {
            break;
        };
        let next = format!("{folder}/");
        if !relative.iter().all(|name| name.starts_with(&next)) {
            break;
        }
        prefix.push_str(&next);
    }
    let has = |name: &str| {
        names
            .iter()
            .any(|(_, value)| value == &format!("{prefix}{name}"))
    };
    if !has("manifest.json") && !has("level.dat") {
        let pack_roots: Vec<_> = names
            .iter()
            .filter_map(|(_, name)| {
                let relative = name.strip_prefix(&prefix)?;
                let folder = relative.strip_suffix("/manifest.json")?;
                (!folder.contains('/')).then(|| format!("{prefix}{folder}/"))
            })
            .collect();
        if !pack_roots.is_empty() {
            if pack_roots.len() > 16 {
                return Err(invalid("Too many Bedrock packs"));
            }
            let mut paths = Vec::new();
            for (number, root) in pack_roots.iter().enumerate() {
                let directory = destination.join(format!("pack-{number}"));
                std::fs::create_dir(&directory).map_err(invalid)?;
                let package = directory.join("source.zip");
                let entries: Vec<_> = names
                    .iter()
                    .filter(|(_, name)| name.starts_with(root))
                    .cloned()
                    .collect();
                write_archive(
                    &mut archive,
                    &entries,
                    root,
                    &package,
                    expanded,
                )?;
                paths.extend(prepare_archive_bounded(
                    &package,
                    &directory,
                    depth + 1,
                    expanded,
                )?);
                if paths.len() > 16 {
                    return Err(invalid("Too many Bedrock packs"));
                }
            }
            return Ok(paths);
        }
    }
    let kind = if has("level.dat")
        && names
            .iter()
            .any(|(_, value)| value.starts_with(&format!("{prefix}db/")))
    {
        "mcworld"
    } else if has("manifest.json") {
        "mcpack"
    } else {
        return Err(invalid("No Bedrock world or pack in archive"));
    };
    let output = destination.join(format!("content.{kind}"));
    write_archive(&mut archive, &names, &prefix, &output, expanded)?;
    Ok(vec![output])
}

fn write_archive(
    archive: &mut zip::ZipArchive<std::fs::File>,
    names: &[(usize, String)],
    prefix: &str,
    output: &Path,
    expanded: &mut u64,
) -> Result<()> {
    use std::{
        fs::File,
        io::{Read, Write},
    };
    let mut writer =
        zip::ZipWriter::new(File::create(output).map_err(invalid)?);
    for (index, name) in names {
        let mut entry = archive.by_index(*index).map_err(invalid)?;
        writer
            .start_file(
                &name[prefix.len()..],
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated),
            )
            .map_err(invalid)?;
        let size = entry.size();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut read = 0u64;
        loop {
            let count = entry.read(&mut buffer).map_err(invalid)?;
            if count == 0 {
                break;
            }
            read += count as u64;
            charge_expanded(expanded, count as u64)?;
            if read > size {
                return Err(invalid("Invalid archive entry size"));
            }
            writer.write_all(&buffer[..count]).map_err(invalid)?;
        }
        if read != size {
            return Err(invalid("Truncated archive entry"));
        }
    }
    writer.finish().map_err(invalid)?;
    Ok(())
}
fn charge_expanded(expanded: &mut u64, count: u64) -> Result<()> {
    *expanded = expanded
        .checked_add(count)
        .ok_or_else(|| invalid("Archive size overflow"))?;
    if *expanded > crate::util::archive::MAX_EXPANDED_BYTES {
        return Err(invalid("Archive exceeds streaming decompression limit"));
    }
    Ok(())
}

pub async fn import_catalog_file(
    project_id: u32,
    file_id: u32,
    path: PathBuf,
    target: Option<super::WorldTarget>,
) -> Result<usize> {
    let status = super::get_status().await?;
    if !status.supported {
        return Err(BedrockError::new(
            ErrorCode::Unsupported,
            "Bedrock requires Windows",
        ));
    }
    if !status.game.is_some_and(|game| game.can_launch) {
        return Err(BedrockError::new(
            ErrorCode::NotInstalled,
            "Minecraft for Windows is not installed",
        ));
    }
    // Re-resolve the IDs natively; never trust the frontend's edition or downloaded path.
    let project =
        crate::orbiont::curseforge_api(&format!("mods/{project_id}"), &[])
            .await
            .map_err(invalid)?;
    if project["data"]["gameId"].as_u64() != Some(78022)
        || !matches!(
            project["data"]["classId"].as_u64(),
            Some(4984 | 6929 | 6913 | 6940)
        )
    {
        return Err(invalid("Not a supported Bedrock project"));
    }
    let is_world = project["data"]["classId"].as_u64() == Some(6913);
    if is_world == target.is_some() {
        return Err(invalid(
            "Select a destination world for packs; import maps as new worlds",
        ));
    }
    let file = crate::orbiont::curseforge_api(
        &format!("mods/{project_id}/files/{file_id}"),
        &[],
    )
    .await
    .map_err(invalid)?;
    let file = &file["data"];
    if file["modId"].as_u64() != Some(project_id as u64)
        || file["id"].as_u64() != Some(file_id as u64)
    {
        return Err(invalid("Bedrock file/project mismatch"));
    }
    let sha1 = file["hashes"]
        .as_array()
        .and_then(|hashes| {
            hashes.iter().find(|hash| hash["algo"].as_u64() == Some(1))
        })
        .and_then(|hash| hash["value"].as_str())
        .ok_or_else(|| invalid("Missing file hash"))?;
    let size = file["fileLength"]
        .as_u64()
        .ok_or_else(|| invalid("Missing file size"))?;
    if size > crate::util::archive::MAX_ARCHIVE_BYTES {
        return Err(BedrockError::new(
            ErrorCode::FileTooLarge,
            "Archive too large",
        ));
    }
    let state = crate::State::get().await.map_err(invalid)?;
    let root = state.directories.caches_dir().join("bedrock-imports");
    tokio::fs::create_dir_all(&root).await.map_err(invalid)?;
    require_space(&root, size.saturating_mul(2))?;
    let stage = tempfile::tempdir_in(root).map_err(invalid)?;
    let verified = crate::util::fetch::stage_curseforge_file(
        &path,
        sha1,
        size,
        stage.path(),
    )
    .await
    .map_err(invalid)?;
    let name = file["fileName"]
        .as_str()
        .ok_or_else(|| invalid("Missing file name"))?;
    let extension = Path::new(name)
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("Missing file extension"))?;
    if !["zip", "mcpack", "mcaddon", "mcworld"]
        .iter()
        .any(|s| s.eq_ignore_ascii_case(extension))
    {
        return Err(invalid("Unsupported Bedrock format"));
    }
    let archive = stage.path().join(format!("source.{extension}"));
    verified
        .copy_to(&archive, &state.io_semaphore)
        .await
        .map_err(invalid)?;
    let destination = stage.path().to_owned();
    let paths = tokio::task::spawn_blocking(move || {
        prepare_archive(&archive, &destination)
    })
    .await
    .map_err(invalid)??;
    let count = paths.len();
    if let Some(target) = target {
        #[cfg(windows)]
        {
            return super::on_windows(move || {
                let world = super::world_packs::target_path(
                    &super::data_root(&target.root_id)?,
                    &target,
                )?;
                super::world_packs::install_into_world(&world, &paths)
            })
            .await;
        }
        #[cfg(not(windows))]
        {
            let _ = target;
            return Err(BedrockError::new(
                ErrorCode::Unsupported,
                "Bedrock requires Windows",
            ));
        }
    }
    // Windows accepts the launch before Minecraft finishes reading the file.
    let _retained = stage.keep();
    for path in paths {
        super::import_file(path).await?;
    }
    Ok(count)
}

/// A conservative preflight for the next known writes, not a reservation or
/// a guarantee against concurrent disk usage, compression overhead or quotas.
pub(super) fn require_space(path: &Path, bytes: u64) -> Result<()> {
    let available = fs4::available_space(path).map_err(invalid)?;
    if available < bytes.saturating_add(64 * 1024 * 1024) {
        return Err(BedrockError::new(
            ErrorCode::InsufficientSpace,
            "Insufficient free space for Bedrock staging and recovery copies",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn rejects_large_central_directory_before_any_staging_output() {
        let fixture = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let path =
            archive(fixture.path(), "bad.zip", &[("manifest.json", b"{}")]);
        let mut bytes = std::fs::read(&path).unwrap();
        let end = bytes.len() - 22;
        bytes[end + 8..end + 10].copy_from_slice(&20_001u16.to_le_bytes());
        bytes[end + 10..end + 12].copy_from_slice(&20_001u16.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(prepare_archive(&path, out.path()).is_err());
        assert_eq!(std::fs::read_dir(out.path()).unwrap().count(), 0);
    }
    #[test]
    fn expanded_stream_budget_and_disk_preflight_fail_before_oversized_writes()
    {
        let mut expanded = crate::util::archive::MAX_EXPANDED_BYTES - 1;
        assert!(charge_expanded(&mut expanded, 2).is_err());
        let fixture = tempfile::tempdir().unwrap();
        assert_eq!(
            require_space(fixture.path(), u64::MAX).unwrap_err().code,
            ErrorCode::InsufficientSpace
        );
        assert!(
            prepare_archive_bounded(
                &fixture.path().join("unused.zip"),
                fixture.path(),
                5,
                &mut 0
            )
            .is_err()
        );
    }
    fn archive(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let path = dir.join(name);
        let mut zip =
            zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, bytes) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }
    #[test]
    fn wraps_a_root_bedrock_pack_and_world_for_official_import() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let pack = archive(
            dir.path(),
            "pack.zip",
            &[("manifest.json", b"{}"), ("textures/a.png", b"image")],
        );
        let result = prepare_archive(&pack, out.path()).unwrap();
        assert_eq!(result[0].extension().unwrap(), "mcpack");
        let world = archive(
            dir.path(),
            "world.zip",
            &[("Map/level.dat", b"level"), ("Map/db/CURRENT", b"db")],
        );
        let result = prepare_archive(&world, out.path()).unwrap();
        assert_eq!(result[0].extension().unwrap(), "mcworld");
        let zip =
            zip::ZipArchive::new(std::fs::File::open(&result[0]).unwrap())
                .unwrap();
        assert!(zip.index_for_name("level.dat").is_some());
        assert!(zip.index_for_name("db/CURRENT").is_some());
    }
    #[test]
    fn extracts_both_embedded_behavior_and_resource_packs() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let inner =
            archive(dir.path(), "inner.mcpack", &[("manifest.json", b"{}")]);
        let bytes = std::fs::read(inner).unwrap();
        let path = archive(
            dir.path(),
            "bundle.zip",
            &[
                ("BP.mcpack", &bytes),
                ("RP.mcpack", &bytes),
                ("readme.txt", b"info"),
            ],
        );
        let paths = prepare_archive(&path, out.path()).unwrap();
        assert_eq!(paths.len(), 2);
        for path in paths {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
    }
    #[test]
    fn imports_both_pack_folders_in_an_addon_zip() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let path = archive(
            dir.path(),
            "addon.zip",
            &[
                ("Addon/Behavior/manifest.json", b"{}"),
                ("Addon/Behavior/entities/a.json", b"entity"),
                ("Addon/Resource/manifest.json", b"{}"),
                ("Addon/Resource/textures/a.png", b"image"),
            ],
        );
        let paths = prepare_archive(&path, out.path()).unwrap();
        assert_eq!(paths.len(), 2);
        assert_ne!(paths[0], paths[1]);
        for path in paths {
            assert_eq!(path.extension().unwrap(), "mcpack");
            let zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap())
                .unwrap();
            assert!(zip.index_for_name("manifest.json").is_some());
        }
    }
    #[test]
    fn rejects_java_worlds_unknown_content_and_unsafe_names() {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        for entries in [
            vec![
                ("level.dat", b"java".as_slice()),
                ("region/r.0.0.mca", b"java".as_slice()),
            ],
            vec![("../manifest.json", b"{}".as_slice())],
            vec![("script.exe", b"MZ".as_slice())],
        ] {
            let path = archive(dir.path(), "bad.zip", &entries);
            assert!(prepare_archive(&path, out.path()).is_err());
        }
    }
}
