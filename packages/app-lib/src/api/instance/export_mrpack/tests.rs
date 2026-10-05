use super::*;
use std::io::Cursor;

#[test]
fn failed_exports_preserve_the_previous_archive_and_remove_temporary_files() {
    let directory = tempfile::tempdir().unwrap();
    let destination = directory.path().join("pack.orbpack");
    let previous = b"previous valid export";
    std::fs::write(&destination, previous).unwrap();
    let files = vec![(
        ReadableContent::Local(directory.path().join("missing-source")),
        SafeRelativeUtf8UnixPathBuf::try_from(
            "config/settings.txt".to_string(),
        )
        .unwrap(),
        1,
    )];
    assert!(
        write_pack_archive_to_path(
            &destination,
            files,
            "orbiont.index.json",
            b"{}",
            |_| Ok(()),
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&destination).unwrap(), previous);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

    // Failure reporting progress also leaves the previous export intact.
    assert!(
        write_pack_archive_to_path(
            &destination,
            Vec::new(),
            "orbiont.index.json",
            b"{}",
            |_| Err(input("progress interrupted")),
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&destination).unwrap(), previous);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

    write_pack_archive_to_path(
        &destination,
        Vec::new(),
        "orbiont.index.json",
        b"{}",
        |_| Ok(()),
    )
    .unwrap();
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(&destination).unwrap())
            .unwrap();
    let mut manifest = String::new();
    archive
        .by_name("orbiont.index.json")
        .unwrap()
        .read_to_string(&mut manifest)
        .unwrap();
    assert_eq!(manifest, "{}");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn orbpack_is_available_and_is_the_default_export_format() {
    let format: PackExportFormat = serde_json::from_str("\"orbpack\"").unwrap();
    assert_eq!(format.extension(), "orbpack");
    assert_eq!(format.index_name(), "orbiont.index.json");
    assert_eq!(
        serde_json::to_value(PackExportFormat::default()).unwrap(),
        "orbpack"
    );
}

#[test]
fn curseforge_manifest_preserves_pack_and_loader_metadata() {
    for (loader, prefix) in [
        (ModLoader::Forge, "forge"),
        (ModLoader::NeoForge, "neoforge"),
        (ModLoader::Fabric, "fabric"),
        (ModLoader::Quilt, "quilt"),
    ] {
        let manifest = curseforge_manifest(
            "1.21.1",
            loader,
            Some("1.2.3"),
            "Custom name",
            "2.0.0",
            Some("Description"),
        )
        .unwrap();
        assert_eq!(manifest["manifestType"], "minecraftModpack");
        assert_eq!(manifest["manifestVersion"], 1);
        assert_eq!(manifest["minecraft"]["version"], "1.21.1");
        assert_eq!(
            manifest["minecraft"]["modLoaders"][0]["id"],
            format!("{prefix}-1.2.3")
        );
        assert_eq!(manifest["minecraft"]["modLoaders"][0]["primary"], true);
        assert_eq!(manifest["name"], "Custom name");
        assert_eq!(manifest["version"], "2.0.0");
        assert_eq!(manifest["description"], "Description");
        assert_eq!(manifest["overrides"], "overrides");
        assert_eq!(manifest["files"], serde_json::json!([]));
    }
    let vanilla = curseforge_manifest(
        "1.21.1",
        ModLoader::Vanilla,
        None,
        "Pack",
        "1",
        None,
    )
    .unwrap();
    assert_eq!(vanilla["minecraft"]["modLoaders"], serde_json::json!([]));
    assert!(
        curseforge_manifest(
            "1.21.1",
            ModLoader::Forge,
            None,
            "Pack",
            "1",
            None
        )
        .is_err()
    );
}

#[test]
fn all_archive_formats_preserve_selected_content_and_write_the_right_index() {
    let dir = std::env::temp_dir()
        .join(format!("orbiont-export-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("sample");
    std::fs::write(&source, b"selected content").unwrap();
    for format in [
        PackExportFormat::Orbpack,
        PackExportFormat::Mrpack,
        PackExportFormat::Curseforge,
    ] {
        let manifest = if matches!(format, PackExportFormat::Orbpack) {
            serde_json::to_vec(
                &crate::pack::orbpack::OrbpackManifest::new(
                    "1.21.1".into(),
                    ModLoader::Fabric,
                    Some("0.16.0".into()),
                    "Pack".into(),
                    "1.0".into(),
                    None,
                )
                .unwrap(),
            )
            .unwrap()
        } else if matches!(format, PackExportFormat::Curseforge) {
            serde_json::to_vec(
                &curseforge_manifest(
                    "1.21.1",
                    ModLoader::Fabric,
                    Some("0.16.0"),
                    "Pack",
                    "1.0",
                    None,
                )
                .unwrap(),
            )
            .unwrap()
        } else {
            br#"{"game":"minecraft","formatVersion":1,"versionId":"1.0","name":"Pack","files":[],"dependencies":{"minecraft":"1.21.1"}}"#.to_vec()
        };
        let mut cursor = Cursor::new(Vec::new());
        let files = ["mods/local.jar.disabled", "config/settings.txt"]
            .into_iter()
            .map(|path| {
                (
                    ReadableContent::Local(source.clone()),
                    path.to_string().try_into().unwrap(),
                    16,
                )
            })
            .collect();
        write_pack_archive(
            &mut cursor,
            files,
            format.index_name(),
            &manifest,
            |_| Ok(()),
        )
        .unwrap();
        let mut archive =
            zip::ZipArchive::new(Cursor::new(cursor.into_inner())).unwrap();
        assert_eq!(archive.len(), 3);
        for path in [
            "overrides/mods/local.jar.disabled",
            "overrides/config/settings.txt",
        ] {
            let mut data = Vec::new();
            archive
                .by_name(path)
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            assert_eq!(data, b"selected content");
        }
        let mut data = Vec::new();
        archive
            .by_name(format.index_name())
            .unwrap()
            .read_to_end(&mut data)
            .unwrap();
        assert_eq!(data, manifest);
        let other_index = match format {
            PackExportFormat::Orbpack => "modrinth.index.json",
            PackExportFormat::Mrpack => "manifest.json",
            PackExportFormat::Curseforge => "modrinth.index.json",
        };
        assert!(archive.by_name(other_index).is_err());
        if !matches!(format, PackExportFormat::Curseforge) {
            assert!(archive.by_name("manifest.json").is_err());
        }
    }
    // Remove only the files created by this test.
    std::fs::remove_file(source).unwrap();
    std::fs::remove_dir(dir).unwrap();
}

#[test]
fn selection_keeps_exclusions_and_never_exports_internal_paths() {
    let selection = ExportSelection::new(
        vec!["mods".into(), "config".into(), "profile.json".into()],
        vec!["mods/excluded.jar".into()],
    );
    let path = |value: &str| value.to_string().try_into().unwrap();
    assert!(selection.is_included(&path("mods/included.jar")));
    assert!(!selection.is_included(&path("mods/excluded.jar")));
    assert!(!selection.is_included(&path("profile.json")));
    assert!(!is_path_exportable(&path("mods/mcef-cache/runtime")));
    for value in [
        ".orbiont/optifine.json",
        ".ORBIONT/optifine.json",
        "mods/OptiFine_1.21.1_HD_U_J1.jar",
        "mods/preview_OptiFine_1.21.1_HD_U_J1.jar.disabled",
    ] {
        assert!(!is_path_exportable(&path(value)), "{value}");
    }
    assert!(selection.should_visit_directory(&path("config")));
    assert!(!selection.should_visit_directory(&path("saves")));
}
