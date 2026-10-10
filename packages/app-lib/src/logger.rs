/*
    tracing is set basd on the environment variable RUST_LOG=xxx, depending on the amount of logs to show
        ERROR > WARN > INFO > DEBUG > TRACE
    eg. RUST_LOG=info will show info, warn, and error logs
        RUST_LOG="theseus=trace" will show *all* messages but from theseus only (and not dependencies using similar crates)
        RUST_LOG="theseus=trace" will show *all* messages but from theseus only (and not dependencies using similar crates)

    Error messages returned to Tauri will display as traced error logs if they return an error.
    This will also include an attached span trace if the error is from a tracing error, and the level is set to info, debug, or trace

    on unix:
        RUST_LOG="theseus=trace" {run command}

    The default is theseus=show, meaning only logs from theseus will be displayed, and at the info or higher level.

*/

// Handling for the live development logging
// This will log to the console, and will not log to a file
#[cfg(debug_assertions)]
pub fn start_logger(_app_identifier: &str) -> Option<()> {
    use tracing_subscriber::prelude::*;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("theseus=info,theseus_gui=info")
        });
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(|| RedactingWriter::new(std::io::stdout())),
        )
        .with(filter)
        .with(tracing_error::ErrorLayer::default())
        .init();
    Some(())
}

// Handling for the live production logging
// This will log to a file in the logs directory, and will not show any logs in the console
#[cfg(not(debug_assertions))]
pub fn start_logger(app_identifier: &str) -> Option<()> {
    use crate::prelude::DirectoryInfo;
    use chrono::Local;
    use std::fs::OpenOptions;
    use tracing_subscriber::fmt::time::ChronoLocal;
    use tracing_subscriber::prelude::*;

    // Initialize and get logs directory path
    let logs_dir = if let Some(d) =
        DirectoryInfo::launcher_logs_dir_path(app_identifier)
    {
        d
    } else {
        eprintln!("Could not start logger");
        return None;
    };

    let log_file_name =
        format!("session_{}.log", Local::now().format("%Y%m%d_%H%M%S"));
    let log_file_path = logs_dir.join(log_file_name);

    if let Err(err) = std::fs::create_dir_all(&logs_dir) {
        eprintln!("Could not create logs directory: {err}");
    }

    let file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_file_path)
    {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Could not start open log file: {e}");
            return None;
        }
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new("theseus=info,theseus_gui=info")
        });

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(move || {
                    RedactingWriter::new(
                        file.try_clone().expect("clone log file"),
                    )
                })
                .with_ansi(false) // disable ANSI escape codes
                .with_timer(ChronoLocal::rfc_3339()),
        )
        .with(filter)
        .with(tracing_error::ErrorLayer::default())
        .init();

    Some(())
}

/// Redact labelled secrets even if a token was never persisted in the account database.
pub fn redact_diagnostics(text: &str) -> String {
    use std::sync::LazyLock;
    static QUOTED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
        r#"(?i)((?:access_token|refresh_token|id_token|identityToken|RpsTicket|code_verifier|verifier|authorization|code_challenge|api_key|password|client_secret|[?&]token|[?&]key|[?&]sig|[?&]code|[?&]state|\bcode|\bstate|\btoken)\s*["']?\s*[:=]\s*)("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')"#
    ).expect("quoted secret redaction regex")
    });
    static LABELLED: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
        r#"(?i)((?:access_token|refresh_token|id_token|identityToken|RpsTicket|code_verifier|verifier|authorization|code_challenge|api_key|password|client_secret|[?&]token|[?&]key|[?&]sig|[?&]code|[?&]state)\s*[\"']?\s*[:=]\s*[\"']?)([^\s\"'&,}]+)"#
    ).expect("secret redaction regex")
    });
    static BEARER: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"(?i)Bearer\s+[^\s\"'&,}]+"#).expect("bearer regex")
    });
    static JWT: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
        )
        .expect("JWT regex")
    });
    static IDENTITY: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r#"(?i)XBL3\.0\s+x=[^\s"'&,}]+"#)
            .expect("Xbox identity regex")
    });
    let text = BEARER.replace_all(text, "Bearer [REDACTED]");
    let text = IDENTITY.replace_all(&text, "[REDACTED]");
    let text = QUOTED.replace_all(&text, "${1}\"[REDACTED]\"");
    let text = LABELLED.replace_all(&text, "${1}[REDACTED]");
    static USER_PATH: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(
            r#"(?i)([A-Z]:[\\/]+Users[\\/]+|/(?:Users|home)/)[^\\/\r\n"']+"#,
        )
        .expect("user path regex")
    });
    let text = JWT.replace_all(&text, "[REDACTED]");
    USER_PATH.replace_all(&text, "${1}[USER]").into_owned()
}

struct RedactingWriter<W: std::io::Write> {
    writer: W,
    buffer: Vec<u8>,
}
impl<W: std::io::Write> RedactingWriter<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            buffer: Vec::new(),
        }
    }
}
impl<W: std::io::Write> std::io::Write for RedactingWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_empty() {
            let redacted =
                redact_diagnostics(&String::from_utf8_lossy(&self.buffer));
            self.writer.write_all(redacted.as_bytes())?;
            self.buffer.clear();
        }
        self.writer.flush()
    }
}
impl<W: std::io::Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let _ = std::io::Write::flush(self);
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    #[test]
    fn diagnostics_redact_unpersisted_secrets_and_split_writes() {
        use std::io::Write;
        let input = r#"access_token=sample-secret refresh_token:other-secret https://login/?code=oauth-secret&state=state-secret Bearer bearer-secret eyJhbGciOiJIUzI1NiJ9.cGF5bG9hZA.signature"#;
        let input = format!(
            r#"{input} Authorization: "Bearer quoted-bearer-secret" authorization: Bearer unquoted-bearer-secret identityToken="XBL3.0 x=hash;xbox-secret" password="long password secret" code="unpersisted-oauth-code""#
        );
        let output = redact_diagnostics(&input);
        for secret in [
            "sample-secret",
            "other-secret",
            "oauth-secret",
            "state-secret",
            "bearer-secret",
            "eyJhbGciOiJIUzI1NiJ9",
            "quoted-bearer-secret",
            "unquoted-bearer-secret",
            "xbox-secret",
            "long password secret",
            "unpersisted-oauth-code",
        ] {
            assert!(!output.contains(secret));
        }
        let mut output = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut output);
            writer.write_all(b"access_token=").unwrap();
            writer.write_all(b"split-secret").unwrap();
        }
        assert!(!String::from_utf8(output).unwrap().contains("split-secret"));
    }
}
