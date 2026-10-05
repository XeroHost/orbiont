use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum ImportError {
    InvalidFile,
    FileTooLarge,
}

pub fn validate_import(path: &Path) -> Result<PathBuf, ImportError> {
    use std::{fs::File, io::Read};

    if !path.is_absolute() {
        return Err(ImportError::InvalidFile);
    }
    let path =
        std::fs::canonicalize(path).map_err(|_| ImportError::InvalidFile)?;
    // Network shares are outside this local integration. Extended local paths
    // returned by canonicalize on Windows remain supported.
    let text = path.to_string_lossy();
    if text.starts_with("\\\\?\\UNC\\")
        || (text.starts_with("\\\\") && !text.starts_with("\\\\?\\"))
    {
        return Err(ImportError::InvalidFile);
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !["mcworld", "mcpack", "mcaddon"]
        .iter()
        .any(|value| extension.eq_ignore_ascii_case(value))
    {
        return Err(ImportError::InvalidFile);
    }
    let mut file = File::open(&path).map_err(|_| ImportError::InvalidFile)?;
    let metadata = file.metadata().map_err(|_| ImportError::InvalidFile)?;
    if !metadata.is_file() || metadata.len() < 4 {
        return Err(ImportError::InvalidFile);
    }
    if metadata.len() > 8 * 1024 * 1024 * 1024 {
        return Err(ImportError::FileTooLarge);
    }
    let mut header = [0; 4];
    file.read_exact(&mut header)
        .map_err(|_| ImportError::InvalidFile)?;
    if header != *b"PK\x03\x04" {
        return Err(ImportError::InvalidFile);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "orbiont-bedrock-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn accepts_all_three_formats_and_uppercase_extensions() {
        let fixture = Fixture::new();
        for name in ["Mundo español.MCWORLD", "pack.mcpack", "addon.mcaddon"] {
            let path = fixture.file(name, b"PK\x03\x04content");
            assert_eq!(
                validate_import(&path),
                Ok(fs::canonicalize(path).unwrap())
            );
        }
    }

    #[test]
    fn rejects_other_formats_and_disguised_executables() {
        let fixture = Fixture::new();
        for (name, content) in [
            ("java.mrpack", b"PK\x03\x04".as_slice()),
            ("pack.mcpack.exe", b"PK\x03\x04".as_slice()),
            ("script.mcpack", b"powershell.exe".as_slice()),
            ("empty.mcworld", b"".as_slice()),
        ] {
            assert_eq!(
                validate_import(&fixture.file(name, content)),
                Err(ImportError::InvalidFile)
            );
        }
    }

    #[test]
    fn rejects_directories_missing_files_and_relative_paths() {
        let fixture = Fixture::new();
        let directory = fixture.0.join("folder.mcworld");
        fs::create_dir(&directory).unwrap();
        for path in [
            directory,
            fixture.0.join("absent.mcpack"),
            PathBuf::from("world.mcworld"),
        ] {
            assert_eq!(validate_import(&path), Err(ImportError::InvalidFile));
        }
    }

    #[test]
    fn rejects_oversized_content_before_reading_it() {
        let fixture = Fixture::new();
        let path = fixture.file("large.mcworld", b"PK\x03\x04");
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(8 * 1024 * 1024 * 1024 + 1)
            .unwrap();
        assert_eq!(validate_import(&path), Err(ImportError::FileTooLarge));
    }
}
