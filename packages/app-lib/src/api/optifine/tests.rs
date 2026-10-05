use super::*;
use std::io::{Cursor, Write};

fn config_class() -> Vec<u8> {
    // Real class-file field metadata. Values deliberately aren't adjacent to
    // field names: a substring search would accept the wrong version.
    let strings = [
        "MC_VERSION",
        "OF_EDITION",
        "OF_RELEASE",
        "ConstantValue",
        "Ljava/lang/String;",
        "decoy",
        "1.21.1",
        "HD_U",
        "J1",
    ];
    let mut bytes = Vec::from([0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52]);
    bytes.extend_from_slice(&13u16.to_be_bytes());
    for text in strings {
        bytes.push(1);
        bytes.extend_from_slice(&(text.len() as u16).to_be_bytes());
        bytes.extend_from_slice(text.as_bytes());
    }
    for index in [7u16, 8, 9] {
        bytes.push(8);
        bytes.extend_from_slice(&index.to_be_bytes());
    }
    bytes.extend_from_slice(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 3]);
    for (name, value) in [(1u16, 10u16), (2, 11), (3, 12)] {
        bytes.extend_from_slice(&0x19u16.to_be_bytes());
        bytes.extend_from_slice(&name.to_be_bytes());
        bytes.extend_from_slice(&5u16.to_be_bytes());
        bytes.extend_from_slice(&1u16.to_be_bytes());
        bytes.extend_from_slice(&4u16.to_be_bytes());
        bytes.extend_from_slice(&2u32.to_be_bytes());
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes
}

fn installer(config: &[u8]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("notch/net/optifine/Config.class", options)
        .unwrap();
    zip.write_all(config).unwrap();
    zip.start_file("optifine/Patcher.class", options).unwrap();
    zip.write_all(b"patcher fixture").unwrap();
    zip.finish().unwrap().into_inner()
}

#[test]
fn reads_version_from_class_fields_and_hashes_the_whole_installer() {
    let bytes = installer(&config_class());
    let info = inspect_bytes(&bytes).unwrap();
    assert_eq!(info.minecraft_version, "1.21.1");
    assert_eq!(info.version, "HD_U_J1");
    assert_eq!(
        info.installer_sha256,
        format!("{:x}", Sha256::digest(&bytes))
    );
}

#[test]
fn rejects_other_mods_truncated_classes_and_unsafe_metadata() {
    assert!(inspect_bytes(&installer(b"MC_VERSION 1.21.1")).is_err());
    assert!(inspect_bytes(b"not a jar").is_err());
    let mut reference = inspect_bytes(&installer(&config_class())).unwrap();
    reference.installer_sha256 = "../outside".into();
    assert!(reference.validate().is_err());
}

#[test]
fn prevents_mismatched_game_and_loader_combinations() {
    let reference = inspect_bytes(&installer(&config_class())).unwrap();
    assert!(
        reference
            .check_compatibility("1.21.1", ModLoader::Vanilla)
            .is_ok()
    );
    assert!(
        reference
            .check_compatibility("26.3", ModLoader::Vanilla)
            .is_err()
    );
    for loader in [
        ModLoader::Forge,
        ModLoader::Fabric,
        ModLoader::Quilt,
        ModLoader::NeoForge,
    ] {
        assert!(reference.check_compatibility("1.21.1", loader).is_err());
    }
}

fn vanilla_version() -> VersionInfo {
    serde_json::from_value(serde_json::json!({
        "assetIndex": { "id":"1.21", "sha1":"", "size":0, "totalSize":0, "url":"" },
        "assets":"1.21", "downloads":{}, "id":"1.21.1", "libraries":[],
        "mainClass":"net.minecraft.client.main.Main", "minimumLauncherVersion":21,
        "releaseTime":"2024-08-08T00:00:00Z", "time":"2024-08-08T00:00:00Z", "type":"release",
        "arguments":{"game":["--accessToken", "${auth_access_token}"],"jvm":["-cp", "${classpath}"]}
    })).unwrap()
}

#[test]
fn launch_patch_keeps_authentication_and_jvm_arguments_for_modern_and_legacy_versions()
 {
    let reference = inspect_bytes(&installer(&config_class())).unwrap();
    let mut version = vanilla_version();
    let jvm = serde_json::to_value(
        &version.arguments.as_ref().unwrap()[&ArgumentType::Jvm],
    )
    .unwrap();
    version.minecraft_arguments = Some("ignored legacy args".into());
    apply_launch_patch(&mut version, &reference);
    assert_eq!(version.main_class, "net.minecraft.launchwrapper.Launch");
    assert_eq!(
        serde_json::to_value(
            &version.arguments.as_ref().unwrap()[&ArgumentType::Game]
        )
        .unwrap(),
        serde_json::json!([
            "--accessToken",
            "${auth_access_token}",
            "--tweakClass",
            "optifine.OptiFineTweaker"
        ])
    );
    assert_eq!(
        serde_json::to_value(
            &version.arguments.as_ref().unwrap()[&ArgumentType::Jvm]
        )
        .unwrap(),
        jvm
    );
    assert!(
        version.libraries.iter().all(
            |library| library.include_in_classpath && !library.downloadable
        )
    );
    assert_eq!(version.libraries.len(), 2);
    version = vanilla_version();
    version
        .arguments
        .as_mut()
        .unwrap()
        .remove(&ArgumentType::Game);
    version.minecraft_arguments =
        Some("--accessToken ${auth_access_token}".into());
    apply_launch_patch(&mut version, &reference);
    assert_eq!(
        version.minecraft_arguments.as_deref(),
        Some(
            "--accessToken ${auth_access_token} --tweakClass optifine.OptiFineTweaker"
        )
    );
    assert!(
        !version
            .arguments
            .as_ref()
            .unwrap()
            .contains_key(&ArgumentType::Game)
    );
}

#[tokio::test]
async fn export_detects_renamed_and_content_addressed_optifine_jars() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("content-hash-without-extension");
    std::fs::write(&path, installer(&config_class())).unwrap();
    assert!(is_optifine_binary(&path).await.unwrap());
    std::fs::write(&path, b"ordinary configuration").unwrap();
    assert!(!is_optifine_binary(&path).await.unwrap());
}

#[test]
fn existing_install_jobs_preserve_or_remove_optifine_without_ambiguity() {
    use crate::install::model::InstallRequest;
    for reference in [
        None,
        Some(None),
        Some(Some(inspect_bytes(&installer(&config_class())).unwrap())),
    ] {
        let request = InstallRequest::InstallExistingInstance {
            instance_id: "instance".into(),
            force: false,
            optifine: reference.clone(),
        };
        let encoded = serde_json::to_vec(&request).unwrap();
        let decoded: InstallRequest = serde_json::from_slice(&encoded).unwrap();
        let InstallRequest::InstallExistingInstance { optifine, .. } = decoded
        else {
            panic!("Unexpected request");
        };
        assert_eq!(optifine, reference);
    }
}

#[tokio::test]
#[ignore = "Requires official locally downloaded OPTIFINE_TEST_INSTALLER, OPTIFINE_TEST_CLIENT and OPTIFINE_TEST_JAVA"]
async fn official_installer_patches_client_and_repairs_corrupt_artifacts() {
    let installer_path =
        PathBuf::from(std::env::var("OPTIFINE_TEST_INSTALLER").unwrap());
    let client = PathBuf::from(std::env::var("OPTIFINE_TEST_CLIENT").unwrap());
    let java = PathBuf::from(std::env::var("OPTIFINE_TEST_JAVA").unwrap());
    let bytes = read_installer(&installer_path).await.unwrap();
    let reference = inspect_bytes(&bytes).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let libraries = directory.path();
    let cached = library_path(libraries, "orbiont-installer", &reference);
    io::create_dir_all(cached.parent().unwrap()).await.unwrap();
    io::write(&cached, &bytes).await.unwrap();
    ensure_artifacts(libraries, &reference, &client, &java, false)
        .await
        .unwrap();
    let patched = library_path(libraries, "orbiont", &reference);
    let wrapper = library_path(libraries, "orbiont-wrapper", &reference);
    fn entries(path: &Path) -> HashMap<String, Vec<u8>> {
        let mut archive =
            zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|index| {
                let mut file = archive.by_index(index).unwrap();
                let name = file.name().to_owned();
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap();
                (name, bytes)
            })
            .collect()
    }
    let patch_entries = entries(&patched);
    let wrapper_hash = digest_file(&wrapper).await.unwrap();
    assert!(is_optifine_binary(&patched).await.unwrap());
    // A cache hit doesn't need the client; corrupt artifacts do.
    ensure_artifacts(
        libraries,
        &reference,
        Path::new("missing-client"),
        &java,
        false,
    )
    .await
    .unwrap();
    io::write(&patched, b"corrupt").await.unwrap();
    io::write(&wrapper, b"corrupt").await.unwrap();
    ensure_artifacts(libraries, &reference, &client, &java, false)
        .await
        .unwrap();
    // ZIP timestamps change across patcher runs; compare the actual classes.
    assert_eq!(entries(&patched), patch_entries);
    assert_eq!(digest_file(&wrapper).await.unwrap(), wrapper_hash);
    io::write(&cached, b"corrupt").await.unwrap();
    assert!(
        ensure_artifacts(libraries, &reference, &client, &java, true)
            .await
            .is_err()
    );
}
