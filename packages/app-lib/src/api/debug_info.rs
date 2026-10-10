//! Local, deliberately small diagnostic archive. Never includes databases or instance files.
use serde::Serialize;
use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub const MAX_FILES: usize = 8;
pub const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_SCAN: usize = 256;
const MAX_SECRET_VALUES: usize = 1024;
const MAX_SECRET_BYTES: usize = 256 * 1024;
const MAX_OVERRIDE_ROWS: i64 = 128;
const MAX_OVERRIDE_BYTES: usize = 32 * 1024;
pub const MAX_ENTRY_BYTES: usize = 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: usize = MAX_FILES * MAX_ENTRY_BYTES + 4096;

#[derive(Clone, Serialize)]
pub struct Progress {
    pub completed: usize,
    pub total: usize,
}

pub fn sanitize(text: &str, secrets: &[String]) -> String {
    // Never partially process an oversized secret set: unmatched values could leak.
    if secrets.len() > MAX_SECRET_VALUES
        || secrets
            .iter()
            .try_fold(0usize, |size, value| size.checked_add(value.len()))
            .is_none_or(|size| size > MAX_SECRET_BYTES)
    {
        return "[payload omitted: secret limits exceeded]".into();
    }
    // Labelled credentials are redacted before an exact secret can erase its label.
    let labelled = crate::logger::redact_diagnostics(text);
    let text = labelled.as_str();
    let mut literals: Vec<&str> = secrets
        .iter()
        .filter(|value| !value.is_empty())
        .flat_map(|value| {
            std::iter::once(value.as_str())
                .chain(value.lines().filter(|line| !line.is_empty()))
        })
        .collect();
    literals.sort_by_key(|value| std::cmp::Reverse(value.len()));
    literals.dedup();
    let patterns: Vec<String> =
        literals.into_iter().map(regex::escape).collect();
    let mut output = String::new();
    if patterns.is_empty() {
        if text.len() > MAX_ENTRY_BYTES {
            return "[oversized payload omitted]".into();
        }
        output.push_str(text);
    } else {
        let Ok(pattern) = regex::RegexBuilder::new(&format!(
            r"(\[REDACTED\]|\[USER\])|(?:{})",
            patterns.join("|")
        ))
        .size_limit(2 * 1024 * 1024)
        .build() else {
            return "[payload omitted: secret pattern limits exceeded]".into();
        };
        let mut cursor = 0;
        for captures in pattern.captures_iter(text) {
            let matched = captures.get(0).expect("regex full match");
            let replacement = if captures.get(1).is_some() {
                matched.as_str()
            } else {
                "[REDACTED]"
            };
            let segment = &text[cursor..matched.start()];
            if output.len() + segment.len() + replacement.len()
                > MAX_ENTRY_BYTES
            {
                return "[oversized sanitized payload omitted]".into();
            }
            output.push_str(segment);
            output.push_str(replacement);
            cursor = matched.end();
        }
        if output.len() + text.len() - cursor > MAX_ENTRY_BYTES {
            return "[oversized sanitized payload omitted]".into();
        }
        output.push_str(&text[cursor..]);
    }
    // Single pass: replacements never become input to other secret substitutions.
    let text = output;
    // Launch environment/arguments and hooks can contain arbitrary unlabelled credentials.
    let mut sanitized = text
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if [
                "custom_env",
                "environment",
                "launch args",
                "launch arguments",
                "command line",
                "hook",
                "password",
                "token",
                "client_secret",
                "authorization",
                "api_key",
            ]
            .iter()
            .any(|key| lower.contains(key))
            {
                "[REDACTED launch configuration]"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if text.ends_with('\n') {
        sanitized.push('\n');
    }
    sanitized
}

fn cancelled(cancel: &AtomicBool) -> std::io::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "Diagnostic export cancelled",
        ))
    } else {
        Ok(())
    }
}

pub fn export(
    logs: &Path,
    destination: &Path,
    version: &str,
    runtime_version: &str,
    secrets: &[String],
    cancel: Arc<AtomicBool>,
    progress: impl Fn(Progress),
) -> std::io::Result<()> {
    cancelled(&cancel)?;
    let parent = destination
        .parent()
        .ok_or_else(|| std::io::Error::other("Invalid destination"))?;
    if destination
        .symlink_metadata()
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Err(std::io::Error::other("Linked destination rejected"));
    }
    if destination.extension().and_then(|value| value.to_str()) != Some("zip") {
        return Err(std::io::Error::other(
            "Diagnostic destination must be a ZIP file",
        ));
    }
    let root = logs.canonicalize()?;
    if logs.symlink_metadata()?.file_type().is_symlink() {
        return Err(std::io::Error::other("Linked log directory rejected"));
    }
    let mut files = Vec::new();
    // Only direct session logs; no recursive traversal, names never exported.
    for entry in std::fs::read_dir(&root)?.take(MAX_SCAN) {
        cancelled(&cancel)?;
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let meta = entry.path().symlink_metadata()?;
        if meta.is_file()
            && !meta.file_type().is_symlink()
            && name.starts_with("session_")
            && name.ends_with(".log")
        {
            files.push(entry.path());
        }
    }
    files.sort();
    files.reverse();
    files.truncate(MAX_FILES);
    let total = files.len();
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut zip = zip::ZipWriter::new(temporary.as_file_mut());
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("system.txt", options)?;
        write!(
            zip,
            "App: {}\nOS: {}\nArchitecture: {}\nOS version: {}\nRuntime: {}\nContents: bounded sanitized launcher logs only. No database, account records, instance files or uploads.\n",
            version,
            std::env::consts::OS,
            std::env::consts::ARCH,
            sysinfo::System::os_version().unwrap_or_default(),
            if runtime_version.len() <= 4096 {
                sanitize(runtime_version, secrets)
            } else {
                "[oversized runtime omitted]".into()
            }
        )?;
        progress(Progress {
            completed: 0,
            total,
        });
        for (index, path) in files.iter().enumerate() {
            cancelled(&cancel)?;
            let meta = path.symlink_metadata()?;
            if !meta.is_file()
                || meta.file_type().is_symlink()
                || path.canonicalize()?.parent() != Some(root.as_path())
            {
                continue;
            }
            let mut open = std::fs::OpenOptions::new();
            open.read(true);
            // Open the link itself on Windows, reject it before reading. Unix O_NOFOLLOW
            // prevents a replaced directory entry from redirecting the file open.
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                open.custom_flags(0x00200000);
            }
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::OpenOptionsExt;
                open.custom_flags(0x20000);
            }
            #[cfg(target_os = "macos")]
            {
                use std::os::unix::fs::OpenOptionsExt;
                open.custom_flags(0x100);
            }
            let file = open.open(path)?;
            let opened = file.metadata()?;
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if opened.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            if !opened.is_file() {
                continue;
            }
            let mut input = file.take(MAX_FILE_BYTES);
            let mut bytes = Vec::new();
            input.read_to_end(&mut bytes)?;
            // Never expose a partial secret cut by the raw input limit: retain only
            // complete lines when the source was truncated.
            if opened.len() > MAX_FILE_BYTES {
                bytes.truncate(
                    bytes
                        .iter()
                        .rposition(|byte| *byte == b'\n')
                        .map_or(0, |index| index + 1),
                );
            }
            cancelled(&cancel)?;
            let text = sanitize(&String::from_utf8_lossy(&bytes), secrets);
            zip.start_file(format!("logs/session-{}.log", index + 1), options)?;
            let mut size = 0;
            for line in text.split_inclusive('\n') {
                cancelled(&cancel)?;
                if size + line.len() > MAX_ENTRY_BYTES {
                    break;
                }
                zip.write_all(line.as_bytes())?;
                size += line.len();
            }
            progress(Progress {
                completed: index + 1,
                total,
            });
        }
        zip.finish()?;
    }
    if temporary.as_file().metadata()?.len() > MAX_ARCHIVE_BYTES as u64 {
        return Err(std::io::Error::other(
            "Diagnostic archive size limit exceeded",
        ));
    }
    temporary.as_file().sync_all()?;
    cancelled(&cancel)?;
    // Persist renames the completed archive atomically; dropping on any failure cleans it.
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

fn push_secret(
    secrets: &mut Vec<String>,
    bytes: &mut usize,
    value: String,
) -> crate::Result<()> {
    if value.is_empty() {
        return Ok(());
    }
    *bytes = bytes.checked_add(value.len()).ok_or_else(|| {
        std::io::Error::other("Diagnostic secret limits exceeded")
    })?;
    if secrets.len() >= MAX_SECRET_VALUES || *bytes > MAX_SECRET_BYTES {
        return Err(
            std::io::Error::other("Diagnostic secret limits exceeded").into()
        );
    }
    secrets.push(value);
    Ok(())
}

async fn read_override_secrets(
    pool: &sqlx::SqlitePool,
    secrets: &mut Vec<String>,
    bytes: &mut usize,
) -> crate::Result<()> {
    // Read only bounded launch override records, rather than every instance's
    // metadata, content, groups and display state on every WebView error.
    let rows = sqlx::query_scalar::<_, Option<String>>(
            "SELECT CASE WHEN length(CAST(overrides AS BLOB)) <= ? THEN overrides ELSE NULL END FROM instance_launch_overrides LIMIT ?"
        ).bind(MAX_OVERRIDE_BYTES as i64).bind(MAX_OVERRIDE_ROWS + 1).fetch_all(pool).await?;
    if rows.len() > MAX_OVERRIDE_ROWS as usize {
        return Err(std::io::Error::other(
            "Diagnostic override limits exceeded",
        )
        .into());
    }
    let mut scanned = 0usize;
    for row in rows {
        let row = row.ok_or_else(|| {
            std::io::Error::other("Diagnostic override limits exceeded")
        })?;
        scanned += row.len();
        if scanned > MAX_SECRET_BYTES {
            return Err(std::io::Error::other(
                "Diagnostic override limits exceeded",
            )
            .into());
        }
        let overrides: crate::state::InstanceLaunchOverridesData =
            serde_json::from_str(&row)?;
        for value in overrides
            .custom_env_vars
            .unwrap_or_default()
            .into_iter()
            .map(|(_, value)| value)
            .chain(overrides.extra_launch_args.unwrap_or_default())
        {
            push_secret(secrets, bytes, value)?;
        }
    }
    Ok(())
}

pub async fn secret_values() -> crate::Result<Vec<String>> {
    let mut secrets = Vec::new();
    let mut bytes = 0;
    for (_, value) in std::env::vars().filter(|(_, value)| value.len() >= 3) {
        push_secret(&mut secrets, &mut bytes, value)?;
    }
    if let Some(state) = crate::State::get_if_initialized() {
        let settings = crate::settings::get().await?;
        for value in settings
            .custom_env_vars
            .into_iter()
            .map(|(_, value)| value)
            .chain(settings.extra_launch_args)
        {
            push_secret(&mut secrets, &mut bytes, value)?;
        }
        read_override_secrets(&state.pool, &mut secrets, &mut bytes).await?;
    }
    let arguments: Vec<String> = secrets
        .iter()
        .filter_map(|value| {
            value.split_once('=').map(|(_, value)| value.to_owned())
        })
        .collect();
    for value in arguments {
        push_secret(&mut secrets, &mut bytes, value)?;
    }
    secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
    secrets.dedup();
    Ok(secrets)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_pass_secret_redaction_is_bounded_and_preserves_utf8() {
        assert_eq!(
            sanitize(
                "custom-value-1234567890 \u{e9} \u{1f600}",
                &["custom-value-1234567890".into()]
            ),
            "[REDACTED] \u{e9} \u{1f600}"
        );
        assert_eq!(
            sanitize(
                "REDACTED RED ACTED",
                &["RED".into(), "ACTED".into(), "REDACTED".into()]
            ),
            "[REDACTED] [REDACTED] [REDACTED]"
        );
        assert_eq!(
            sanitize(&"x".repeat(MAX_FILE_BYTES as usize), &["x".into()]),
            "[oversized sanitized payload omitted]"
        );
        assert_eq!(
            sanitize("private-prefix", &["x".repeat(MAX_SECRET_BYTES + 1)]),
            "[payload omitted: secret limits exceeded]"
        );
        assert!(
            !sanitize(
                "password=unregistered-sensitive-value",
                &["password".into()]
            )
            .contains("unregistered-sensitive-value")
        );
        let mut secrets = Vec::new();
        let mut bytes = 0;
        assert!(
            push_secret(
                &mut secrets,
                &mut bytes,
                "x".repeat(MAX_SECRET_BYTES + 1)
            )
            .is_err()
        );
        assert!(secrets.is_empty());
    }
    #[test]
    fn bounded_sanitized_local_archive_and_cancellation() {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        for i in 0..12 {
            std::fs::write(logs.join(format!("session_{i:02}.log")), "password=hidden private-value C:\\Users\\Alice\\launcher\ncustom_env_vars: arbitrary-secret").unwrap();
        }
        std::fs::write(logs.join("app.db"), "raw-credential").unwrap();
        let dest = root.path().join("debug.zip");
        export(
            &logs,
            &dest,
            "test",
            "WebView test",
            &["private-value".into()],
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        assert_eq!(zip.len(), MAX_FILES + 1);
        for i in 0..zip.len() {
            let mut text = String::new();
            zip.by_index(i).unwrap().read_to_string(&mut text).unwrap();
            for secret in [
                "hidden",
                "private-value",
                "Alice",
                "arbitrary-secret",
                "raw-credential",
            ] {
                assert!(!text.contains(secret));
            }
        }
        let original = std::fs::read(&dest).unwrap();
        assert!(
            export(
                &logs,
                &dest,
                "test",
                "WebView test",
                &[],
                Arc::new(AtomicBool::new(true)),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&dest).unwrap(), original);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }
    #[tokio::test]
    async fn override_scan_is_fresh_schema_validated_and_bounded() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE instance_launch_overrides (overrides TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        let record = r#"{"custom_env_vars":[["KEY","private-value"]],"extra_launch_args":["--key=another-secret"]}"#;
        sqlx::query("INSERT INTO instance_launch_overrides VALUES (?)")
            .bind(record)
            .execute(&pool)
            .await
            .unwrap();
        let mut secrets = Vec::new();
        read_override_secrets(&pool, &mut secrets, &mut 0)
            .await
            .unwrap();
        assert_eq!(secrets, vec!["private-value", "--key=another-secret"]);
        sqlx::query("UPDATE instance_launch_overrides SET overrides = '{}' ")
            .execute(&pool)
            .await
            .unwrap();
        secrets.clear();
        read_override_secrets(&pool, &mut secrets, &mut 0)
            .await
            .unwrap();
        assert!(secrets.is_empty());
        sqlx::query("UPDATE instance_launch_overrides SET overrides = ?")
            .bind("x".repeat(MAX_OVERRIDE_BYTES + 1))
            .execute(&pool)
            .await
            .unwrap();
        assert!(
            read_override_secrets(&pool, &mut secrets, &mut 0)
                .await
                .is_err()
        );
        sqlx::query(r#"UPDATE instance_launch_overrides SET overrides = '{"custom_env_vars":42}'"#).execute(&pool).await.unwrap();
        assert!(
            read_override_secrets(&pool, &mut secrets, &mut 0)
                .await
                .is_err()
        );
        sqlx::query("DELETE FROM instance_launch_overrides")
            .execute(&pool)
            .await
            .unwrap();
        for _ in 0..=MAX_OVERRIDE_ROWS {
            sqlx::query("INSERT INTO instance_launch_overrides VALUES ('{}')")
                .execute(&pool)
                .await
                .unwrap();
        }
        assert!(
            read_override_secrets(&pool, &mut secrets, &mut 0)
                .await
                .is_err()
        );
    }
    #[test]
    fn raw_utf8_cutoff_and_runtime_metadata_never_emit_secret_prefixes() {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        let secret = "custom-value-1234567890";
        let dest = root.path().join("boundaries.zip");
        for (remaining, suffix) in
            [(18, secret.to_owned()), (11, format!("\u{e9}{secret}"))]
        {
            let input = format!(
                "safe line\n{}{suffix}",
                "x".repeat(MAX_FILE_BYTES as usize - remaining)
            );
            std::fs::write(logs.join("session_boundary.log"), input).unwrap();
            export(
                &logs,
                &dest,
                "test",
                &format!("{}{secret}", "x".repeat(4090)),
                &[secret.into()],
                Arc::new(AtomicBool::new(false)),
                |_| {},
            )
            .unwrap();
            let mut zip =
                zip::ZipArchive::new(std::fs::File::open(&dest).unwrap())
                    .unwrap();
            let mut log = String::new();
            zip.by_name("logs/session-1.log")
                .unwrap()
                .read_to_string(&mut log)
                .unwrap();
            assert_eq!(log, "safe line\n");
            let mut system = String::new();
            zip.by_name("system.txt")
                .unwrap()
                .read_to_string(&mut system)
                .unwrap();
            assert!(system.contains("[oversized runtime omitted]"));
            assert!(!system.contains("custom-v"));
        }
    }
    #[test]
    fn large_lines_expansion_and_late_cancel_are_safe() {
        let root = tempfile::tempdir().unwrap();
        let logs = root.path().join("logs");
        std::fs::create_dir(&logs).unwrap();
        // The bound falls in the middle of a custom value. Drop that incomplete line.
        std::fs::write(
            logs.join("session_large.log"),
            format!("safe line\n{}", "x".repeat(MAX_FILE_BYTES as usize + 100)),
        )
        .unwrap();
        let dest = root.path().join("bounded.zip");
        let seen = std::sync::Mutex::new(Vec::new());
        export(
            &logs,
            &dest,
            "test",
            "WebView test",
            &["xx".into()],
            Arc::new(AtomicBool::new(false)),
            |value| seen.lock().unwrap().push(value.completed),
        )
        .unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let mut text = String::new();
        zip.by_name("logs/session-1.log")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "safe line\n");
        assert_eq!(*seen.lock().unwrap(), vec![0, 1]);
        // Windows refuses to replace a file that is still open.
        drop(zip);
        std::fs::write(
            logs.join("session_large.log"),
            "x".repeat(MAX_FILE_BYTES as usize),
        )
        .unwrap();
        export(
            &logs,
            &dest,
            "test",
            "WebView test",
            &["x".into()],
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .unwrap();
        let mut zip =
            zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        assert!(
            zip.by_name("logs/session-1.log").unwrap().size()
                <= MAX_ENTRY_BYTES as u64
        );
        drop(zip);
        let original = std::fs::read(&dest).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        assert!(
            export(
                &logs,
                &dest,
                "test",
                "WebView test",
                &[],
                cancel.clone(),
                |value| {
                    if value.completed == 1 {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&dest).unwrap(), original);
        assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 2);
    }
}
