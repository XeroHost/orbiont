use crate::api::Result;
use theseus::logs::LogType;
use theseus::logs::{self, CensoredString, LatestLogCursor, Logs};

/*
A log is a struct containing the filename string, stdout, and stderr, as follows:

pub struct Logs {
    pub filename:  String,
    pub stdout: String,
    pub stderr: String,
}
*/

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("logs")
        .js_init_script(include_str!("webview-logs.js"))
        .invoke_handler(tauri::generate_handler![
            webview_log,
            logs_get_logs,
            logs_get_logs_by_filename,
            logs_get_output_by_filename,
            logs_delete_logs,
            logs_delete_logs_by_filename,
            logs_get_latest_log_cursor,
            logs_get_live_log_buffer,
            logs_clear_live_log_buffer,
        ])
        .build()
}

/// Get all logs for an instance, sorted by filename.
#[tauri::command]
pub async fn logs_get_logs(
    instance_id: &str,
    clear_contents: Option<bool>,
) -> Result<Vec<Logs>> {
    let val = logs::get_logs(instance_id, clear_contents).await?;

    Ok(val)
}

/// Get a log struct for an instance by filename.
#[tauri::command]
pub async fn logs_get_logs_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<Logs> {
    Ok(logs::get_logs_by_filename(instance_id, log_type, filename).await?)
}

/// Get the output for an instance by filename.
#[tauri::command]
pub async fn logs_get_output_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<CensoredString> {
    Ok(logs::get_output_by_filename(instance_id, log_type, &filename).await?)
}

/// Delete all logs for an instance.
#[tauri::command]
pub async fn logs_delete_logs(instance_id: &str) -> Result<()> {
    Ok(logs::delete_logs(instance_id).await?)
}

/// Delete a log for an instance by filename.
#[tauri::command]
pub async fn logs_delete_logs_by_filename(
    instance_id: &str,
    log_type: LogType,
    filename: String,
) -> Result<()> {
    Ok(logs::delete_logs_by_filename(instance_id, log_type, &filename).await?)
}

/// Get live log from a cursor
#[tauri::command]
pub async fn logs_get_latest_log_cursor(
    instance_id: &str,
    cursor: u64, // 0 to start at beginning of file
) -> Result<LatestLogCursor> {
    Ok(logs::get_latest_log_cursor(instance_id, cursor).await?)
}

/// Get all buffered live log lines for an instance.
#[tauri::command]
pub async fn logs_get_live_log_buffer(
    instance_id: &str,
) -> Result<CensoredString> {
    Ok(logs::get_live_log_buffer(instance_id).await?)
}

/// Clear the live log buffer for an instance.
#[tauri::command]
pub async fn logs_clear_live_log_buffer(instance_id: &str) -> Result<()> {
    logs::clear_live_log_buffer(instance_id);
    Ok(())
}

// IPC is also bounded: the JavaScript limiter is not a trust boundary.
#[tauri::command]
pub async fn webview_log<R: tauri::Runtime>(
    webview: tauri::Webview<R>,
    level: String,
    message: String,
) {
    use std::sync::{LazyLock, Mutex};
    use std::time::{Duration, Instant};
    static RATE: LazyLock<Mutex<(Instant, usize)>> =
        LazyLock::new(|| Mutex::new((Instant::now(), 0)));
    if webview.label() != "main"
        || !matches!(level.as_str(), "warn" | "error")
        || message.len() > 4096
    {
        return;
    }
    {
        let Ok(mut rate) = RATE.lock() else {
            return;
        };
        if rate.0.elapsed() >= Duration::from_secs(10) {
            *rate = (Instant::now(), 0);
        }
        if rate.1 >= 20 {
            return;
        }
        rate.1 += 1;
    }
    let Ok(secrets) = theseus::debug_info::secret_values().await else {
        tracing::error!(
            "WebView error: payload omitted because saved secret values could not be read"
        );
        return;
    };
    let message = theseus::debug_info::sanitize(&message, &secrets);
    if level == "warn" {
        tracing::warn!("WebView: {message}");
    } else {
        tracing::error!("WebView: {message}");
    }
}
