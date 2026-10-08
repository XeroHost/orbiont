//! Launch acceptance and observed game processes are different signals.
use serde::Serialize;
#[derive(Debug, Clone, Serialize)]
pub struct LaunchDiagnostic {
    pub state: String,
    pub elapsed_ms: u64,
    pub diagnostic: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct ProcessStatus {
    pub game_running: bool,
    pub launch: LaunchDiagnostic,
}
#[derive(Debug, Serialize)]
pub struct StopOutcome {
    pub state: &'static str,
    pub force_token: Option<String>,
}
#[cfg(any(windows, test))]
fn retail_identity(family: &str, image: &std::path::Path) -> bool {
    family == "Microsoft.MinecraftUWP_8wekyb3d8bbwe"
        && image.file_name().is_some_and(is_game_process)
}
#[cfg(any(windows, test))]
fn graceful_close(
    close: impl FnOnce() -> super::Result<()>,
    wait: impl FnOnce(u32) -> super::Result<bool>,
) -> super::Result<bool> {
    close()?;
    wait(5000)
}

#[cfg(windows)]
#[allow(clippy::cfg_not_test)]
pub(super) fn stop(force_token: Option<String>) -> super::Result<StopOutcome> {
    #[cfg(not(test))]
    {
        native_stop::stop(force_token)
    }
    // Native test suites must never close a developer's running game.
    #[cfg(test)]
    {
        let _ = force_token;
        Err(super::BedrockError::new(
            super::ErrorCode::Unsupported,
            "Live stop is disabled in tests",
        ))
    }
}

#[cfg(all(windows, not(test)))]
// The fixture build must not link an executable live termination path.
#[allow(clippy::cfg_not_test)]
mod native_stop {
    use super::StopOutcome;
    use crate::api::bedrock::{BedrockError, ErrorCode, Result};
    use std::{
        ffi::c_void,
        sync::Mutex,
        time::{Duration, Instant},
    };
    type Handle = *mut c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetPackageFamilyName(
            handle: Handle,
            length: *mut u32,
            name: *mut u16,
        ) -> i32;
        fn QueryFullProcessImageNameW(
            handle: Handle,
            flags: u32,
            path: *mut u16,
            length: *mut u32,
        ) -> i32;
        fn WaitForSingleObject(handle: Handle, milliseconds: u32) -> u32;
        fn TerminateProcess(handle: Handle, code: u32) -> i32;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(
            callback: unsafe extern "system" fn(Handle, isize) -> i32,
            parameter: isize,
        ) -> i32;
        fn GetWindowThreadProcessId(window: Handle, pid: *mut u32) -> u32;
        fn PostMessageW(
            window: Handle,
            message: u32,
            wparam: usize,
            lparam: isize,
        ) -> i32;
    }
    // The open handle binds all requests to the same process object even if
    // Windows later reuses its numeric PID. Integer storage is Send without
    // claiming thread safety for an arbitrary raw pointer.
    struct RetailProcess {
        handle: isize,
        pid: u32,
    }
    impl RetailProcess {
        fn handle(&self) -> Handle {
            self.handle as Handle
        }
        fn exited(&self, milliseconds: u32) -> Result<bool> {
            // SAFETY: this owned process handle remains open until Drop.
            match unsafe { WaitForSingleObject(self.handle(), milliseconds) } {
                0 => Ok(true),
                258 => Ok(false),
                _ => Err(failure(
                    "Could not wait for the selected Minecraft process",
                )),
            }
        }
    }
    impl Drop for RetailProcess {
        fn drop(&mut self) {
            // SAFETY: each successful OpenProcess handle is closed exactly once.
            unsafe {
                CloseHandle(self.handle());
            }
        }
    }
    struct Pending {
        token: String,
        process: RetailProcess,
        created: Instant,
    }
    static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
    fn failure(message: &str) -> BedrockError {
        BedrockError::new(ErrorCode::LaunchFailed, message)
    }
    fn verified(pid: u32) -> Result<RetailProcess> {
        // PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE | PROCESS_TERMINATE.
        // No termination happens here; that right is used only with the token
        // returned after a graceful-close timeout and confirmed in the UI.
        let handle = unsafe { OpenProcess(0x1000 | 0x100000 | 1, 0, pid) };
        if handle.is_null() {
            return Err(failure("Cannot verify Minecraft process identity"));
        }
        let process = RetailProcess {
            handle: handle as isize,
            pid,
        };
        let mut family = [0u16; 256];
        let mut length = family.len() as u32;
        // SAFETY: buffers have their declared capacities and handle is owned.
        let result = unsafe {
            GetPackageFamilyName(handle, &raw mut length, family.as_mut_ptr())
        };
        let family = String::from_utf16_lossy(
            &family
                [..length.saturating_sub(1).min(family.len() as u32) as usize],
        );
        let mut image = vec![0u16; 32768].into_boxed_slice();
        let mut image_length = image.len() as u32;
        let queried = unsafe {
            QueryFullProcessImageNameW(
                handle,
                0,
                image.as_mut_ptr(),
                &raw mut image_length,
            )
        };
        let image = String::from_utf16_lossy(
            &image[..image_length.min(image.len() as u32) as usize],
        );
        if result != 0
            || queried == 0
            || !super::retail_identity(&family, std::path::Path::new(&image))
        {
            return Err(failure(
                "Minecraft process is not the detected official retail package",
            ));
        }
        Ok(process)
    }
    unsafe extern "system" fn close_window(
        window: Handle,
        parameter: isize,
    ) -> i32 {
        let mut pid = 0;
        // SAFETY: synchronous EnumWindows receives a reference to the owned
        // process that remains alive throughout enumeration. Do not post to a
        // recycled PID if the retained process object has already exited.
        unsafe {
            let process = &*(parameter as *const RetailProcess);
            GetWindowThreadProcessId(window, &raw mut pid);
            if pid == process.pid
                && WaitForSingleObject(process.handle(), 0) == 258
            {
                PostMessageW(window, 0x0010, 0, 0);
            }
        }
        1
    }
    fn stopped() -> StopOutcome {
        StopOutcome {
            state: "stopped",
            force_token: None,
        }
    }
    pub(super) fn stop(force_token: Option<String>) -> Result<StopOutcome> {
        let mut pending = PENDING
            .try_lock()
            .map_err(|_| failure("Minecraft stop is already pending"))?;
        if let Some(token) = force_token {
            let selected = pending.as_ref().filter(|p| p.token == token && p.created.elapsed() < Duration::from_secs(600))
                .ok_or_else(|| failure("The forced-stop request expired; request a normal close again"))?;
            if !selected.process.exited(0)? {
                // SAFETY: token selects the retained verified handle, never a
                // name match or a fresh PID lookup.
                if unsafe { TerminateProcess(selected.process.handle(), 1) }
                    == 0
                {
                    return Err(failure(
                        "Windows could not force close the selected Minecraft process",
                    ));
                }
                if !selected.process.exited(2000)? {
                    return Err(failure("Minecraft has not exited yet"));
                }
            }
            *pending = None;
            return Ok(stopped());
        }
        let mut system = sysinfo::System::new();
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing(),
        );
        let candidates: Vec<_> = system
            .processes()
            .values()
            .filter(|p| super::is_game_process(p.name()))
            .collect();
        if candidates.is_empty() {
            *pending = None;
            return Ok(stopped());
        }
        if candidates.len() != 1 {
            return Err(failure(
                "More than one Minecraft process is present; close them in the game",
            ));
        }
        let process = verified(candidates[0].pid().as_u32())?;
        // The Bedrock mutation mutex is never held while waiting for exit.
        let exited = super::graceful_close(
            || {
                unsafe {
                    EnumWindows(close_window, (&raw const process) as isize);
                }
                Ok(())
            },
            |milliseconds| process.exited(milliseconds),
        )?;
        if exited {
            *pending = None;
            return Ok(stopped());
        }
        let token = uuid::Uuid::new_v4().to_string();
        *pending = Some(Pending {
            token: token.clone(),
            process,
            created: Instant::now(),
        });
        Ok(StopOutcome {
            state: "timeout",
            force_token: Some(token),
        })
    }
}
fn diagnostic(
    requested: Option<u64>,
    accepted: bool,
    running: bool,
    now: u64,
) -> LaunchDiagnostic {
    let elapsed_ms = requested
        .map(|start| now.saturating_sub(start))
        .unwrap_or(0);
    let state = if running {
        "running"
    } else if requested.is_none() {
        "idle"
    } else if elapsed_ms >= 30000 {
        "timeout"
    } else if accepted {
        "starting"
    } else {
        "requested"
    };
    LaunchDiagnostic {
        state: state.into(),
        elapsed_ms,
        diagnostic: if state == "timeout" {
            Some("Windows accepted a launch request, but the Minecraft engine has not been observed. Open the official launcher or check Microsoft Store repair status.".into())
        } else {
            None
        },
    }
}
#[derive(Default)]
struct Request {
    started: Option<std::time::Instant>,
    accepted: bool,
    failure: Option<String>,
}
static REQUEST: std::sync::Mutex<Request> = std::sync::Mutex::new(Request {
    started: None,
    accepted: false,
    failure: None,
});
pub(super) fn requested() {
    if let Ok(mut request) = REQUEST.lock() {
        *request = Request {
            started: Some(std::time::Instant::now()),
            accepted: false,
            failure: None,
        };
    }
}
pub(super) fn accepted() {
    if let Ok(mut request) = REQUEST.lock() {
        request.accepted = true;
    }
}
pub(super) fn failed(message: String) {
    if let Ok(mut request) = REQUEST.lock() {
        request.failure = Some(message);
    }
}
#[cfg(any(windows, test))]
pub(super) fn is_game_process(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy();
    name.eq_ignore_ascii_case("Minecraft.Windows.exe")
        || name.eq_ignore_ascii_case("Minecraft.Windows")
}
#[allow(clippy::cfg_not_test)]
pub(super) fn running() -> bool {
    #[cfg(all(windows, not(test)))]
    {
        static SYSTEM: std::sync::LazyLock<std::sync::Mutex<sysinfo::System>> =
            std::sync::LazyLock::new(|| {
                std::sync::Mutex::new(sysinfo::System::new())
            });
        let Ok(mut system) = SYSTEM.lock() else {
            return true;
        };
        system.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::All,
            true,
            sysinfo::ProcessRefreshKind::nothing(),
        );
        system
            .processes()
            .values()
            .any(|p| is_game_process(p.name()))
    }
    #[cfg(any(not(windows), test))]
    {
        false
    }
}
#[allow(
    clippy::cfg_not_test,
    reason = "Only live operations use the process-wide launch timer; fixtures simulate launch states"
)]
#[cfg(not(test))]
pub(super) fn mutation_pending() -> bool {
    REQUEST
        .lock()
        .map(|request| {
            request.failure.is_none()
                && request.started.is_some_and(|start| {
                    start.elapsed() < std::time::Duration::from_secs(30)
                })
        })
        .unwrap_or(true)
}
pub(super) fn status() -> ProcessStatus {
    let game_running = running();
    let launch = if let Ok(mut request) = REQUEST.lock() {
        let elapsed = request.started.map(|start| {
            start.elapsed().as_millis().min(u64::MAX as u128) as u64
        });
        let mut launch = diagnostic(
            elapsed.map(|_| 0),
            request.accepted,
            game_running,
            elapsed.unwrap_or(0),
        );
        if let Some(message) = &request.failure
            && !game_running
        {
            launch.state = "failed".into();
            launch.diagnostic = Some(message.clone());
        }
        if game_running {
            *request = Request::default();
        }
        launch
    } else {
        LaunchDiagnostic {
            state: "failed".into(),
            elapsed_ms: 0,
            diagnostic: Some("Launch monitor is unavailable".into()),
        }
    };
    ProcessStatus {
        game_running,
        launch,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn graceful_stop_requests_close_before_waiting_with_a_fixed_deadline() {
        let events = std::cell::RefCell::new(Vec::new());
        let stopped = graceful_close(
            || {
                events.borrow_mut().push("close");
                Ok(())
            },
            |deadline| {
                assert_eq!(deadline, 5000);
                events.borrow_mut().push("wait");
                Ok(false)
            },
        )
        .unwrap();
        assert!(!stopped);
        assert_eq!(*events.borrow(), ["close", "wait"]);
        assert!(
            graceful_close(
                || Ok(()),
                |deadline| {
                    assert_eq!(deadline, 5000);
                    Ok(true)
                }
            )
            .unwrap()
        );
    }
    #[test]
    fn process_identity_requires_the_retail_package_and_engine_image() {
        let image = std::path::Path::new("Minecraft.Windows.exe");
        assert!(retail_identity(
            "Microsoft.MinecraftUWP_8wekyb3d8bbwe",
            image
        ));
        assert!(!retail_identity("Unrelated.Package_8wekyb3d8bbwe", image));
        assert!(!retail_identity(
            "Microsoft.MinecraftUWP_8wekyb3d8bbwe",
            std::path::Path::new("MinecraftLauncher.exe")
        ));
    }
    #[cfg(windows)]
    #[test]
    fn fixture_build_never_executes_native_process_termination() {
        assert_eq!(
            stop(None).unwrap_err().code,
            super::super::ErrorCode::Unsupported
        );
        assert_eq!(
            stop(Some("any-token".into())).unwrap_err().code,
            super::super::ErrorCode::Unsupported
        );
    }
    #[test]
    fn process_monitor_recognizes_only_the_bedrock_engine() {
        use std::ffi::OsStr;
        for name in [
            "Minecraft.Windows.exe",
            "minecraft.windows.EXE",
            "Minecraft.Windows",
        ] {
            assert!(is_game_process(OsStr::new(name)));
        }
        for name in [
            "java.exe",
            "javaw.exe",
            "MinecraftLauncher.exe",
            "Minecraft.Windows.exe.bak",
            "Minecraft",
        ] {
            assert!(!is_game_process(OsStr::new(name)));
        }
    }
    #[test]
    fn accepted_launch_is_starting_until_engine_is_observed_and_times_out() {
        assert_eq!(diagnostic(Some(100), false, false, 101).state, "requested");
        assert_eq!(diagnostic(Some(100), true, false, 105).state, "starting");
        assert_eq!(diagnostic(Some(100), true, true, 106).state, "running");
        let timed = diagnostic(Some(100), true, false, 30100);
        assert_eq!(timed.state, "timeout");
        assert!(timed.diagnostic.is_some());
        assert_eq!(diagnostic(None, false, false, 500).state, "idle");
    }
}
