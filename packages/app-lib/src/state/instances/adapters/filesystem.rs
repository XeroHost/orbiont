use crate::state::{ProjectType, file_hash_cache_key, file_modified_at_ns};
use crate::util::io::{self, IOError};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub(crate) struct ScannedContentFile {
    pub relative_path: String,
    pub file_name: String,
    pub enabled: bool,
    pub size: u64,
    pub hash_cache_key: String,
    pub is_symlink: bool,
    pub has_linked_parent: bool,
}

pub(crate) fn scan_content_files(
    instances_dir: &Path,
    instance_path: &str,
) -> crate::Result<Vec<ScannedContentFile>> {
    crate::state::content_store::validate_instance_path(instance_path)?;
    let instance_full_path = instances_dir.join(instance_path);
    let instance_dir = io::canonicalize(instance_full_path)?;
    let linked_instance =
        instance_dir != io::canonicalize(instances_dir)?.join(instance_path);
    let mut files = Vec::new();

    for project_type in ProjectType::iterator() {
        let folder = project_type.get_folder();
        let folder_path = instance_dir.join(folder);
        let has_linked_parent = linked_instance
            || std::fs::symlink_metadata(&folder_path)
                .is_ok_and(|metadata| metadata.file_type().is_symlink());

        if !folder_path.exists() {
            continue;
        }

        for entry in std::fs::read_dir(&folder_path)
            .map_err(|err| IOError::with_path(err, &folder_path))?
        {
            let path = entry.map_err(IOError::from)?.path();
            if !path.is_file() {
                continue;
            }

            let Some(file_name) =
                path.file_name().and_then(|value| value.to_str())
            else {
                continue;
            };

            if !is_scannable_project_file(project_type, file_name) {
                continue;
            }

            let metadata = path.metadata().map_err(IOError::from)?;
            let size = metadata.len();
            let modified_at_ns =
                file_modified_at_ns(&metadata).map_err(IOError::from)?;
            let relative_path = format!("{folder}/{file_name}");
            let hash_cache_key = file_hash_cache_key(
                size,
                modified_at_ns,
                &format!("{instance_path}/{relative_path}"),
            );

            files.push(ScannedContentFile {
                has_linked_parent,
                relative_path,
                file_name: file_name.to_string(),
                enabled: !file_name.ends_with(".disabled"),
                size,
                hash_cache_key,
                is_symlink: path
                    .symlink_metadata()
                    .map_err(IOError::from)?
                    .file_type()
                    .is_symlink(),
            });
        }
    }

    Ok(files)
}

pub(crate) fn project_type_from_relative_path(
    relative_path: &str,
) -> Option<ProjectType> {
    ProjectType::get_from_parent_folder(PathBuf::from(relative_path))
}

fn is_scannable_project_file(
    project_type: ProjectType,
    file_name: &str,
) -> bool {
    let Some(extension) = Path::new(file_name.trim_end_matches(".disabled"))
        .extension()
        .and_then(|ext| ext.to_str())
    else {
        return false;
    };

    match project_type {
        ProjectType::Mod => extension.eq_ignore_ascii_case("jar"),
        ProjectType::DataPack
        | ProjectType::ResourcePack
        | ProjectType::ShaderPack => {
            extension.eq_ignore_ascii_case("zip")
                || extension.eq_ignore_ascii_case("jar")
        }
    }
}

#[cfg(test)]
mod upstream_scan_tests {
    use super::*;
    #[test]
    fn disabled_file_uses_actual_disk_timestamp_and_path() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("instance/mods");
        std::fs::create_dir_all(&mods).unwrap();
        let path = mods.join("example.jar.disabled");
        std::fs::write(&path, b"disabled-content").unwrap();
        let files = scan_content_files(dir.path(), "instance").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].relative_path, "mods/example.jar.disabled");
        assert!(!files[0].enabled);
        let metadata = std::fs::metadata(path).unwrap();
        assert_eq!(
            files[0].hash_cache_key,
            file_hash_cache_key(
                metadata.len(),
                file_modified_at_ns(&metadata).unwrap(),
                "instance/mods/example.jar.disabled"
            )
        );
        assert!(scan_content_files(dir.path(), "../outside").is_err());
    }
}
