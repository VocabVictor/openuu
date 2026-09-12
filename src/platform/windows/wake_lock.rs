use super::*;

pub struct WakeLock(u32);

// Failed to compile keepawake-rs on i686
impl WakeLock {
    pub fn new(display: bool, idle: bool, sleep: bool) -> Self {
        let mut flag = ES_CONTINUOUS;
        if display {
            flag |= ES_DISPLAY_REQUIRED;
        }
        if idle {
            flag |= ES_SYSTEM_REQUIRED;
        }
        if sleep {
            flag |= ES_AWAYMODE_REQUIRED;
        }
        unsafe { SetThreadExecutionState(flag) };
        WakeLock(flag)
    }

    pub fn set_display(&mut self, display: bool) -> ResultType<()> {
        let flag = if display {
            self.0 | ES_DISPLAY_REQUIRED
        } else {
            self.0 & !ES_DISPLAY_REQUIRED
        };
        if flag != self.0 {
            unsafe { SetThreadExecutionState(flag) };
            self.0 = flag;
        }
        Ok(())
    }
}

impl Drop for WakeLock {
    fn drop(&mut self) {
        unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
    }
}

// `check_process("--tray", ..)` can miss a tray process that is already running,
// and every miss spawns one more tray icon.
//
// The case confirmed in #15689: `run_after_run_cmds()` spawns the tray in the
// caller's own context, so installing or toggling the service from a RustDesk
// that was itself started elevated leaves a high integrity tray behind. A main
// window started normally afterwards runs at medium integrity and cannot open
// that process with `PROCESS_QUERY_INFORMATION | PROCESS_VM_READ`. sysinfo then
// falls back to `PROCESS_QUERY_LIMITED_INFORMATION`, which is not enough for
// `GetModuleFileNameExW`, so the executable path comes back empty and the tray
// is skipped before its command line is ever looked at.
//
// A second blind spot: 32-bit builds read the command line through `wmic`
// (#11638), which is no longer installed by default since Windows 11 24H2.
//
// Both are cases of one process failing to inspect another, and patching the
// inspection has regressed twice already (#6692), so use a named mutex instead:
// the kernel answers without us needing any access to the other process.
//
// Returns `false` if another tray process is already running in this session.
pub fn try_lock_tray_single_instance() -> bool {
    use winapi::um::{
        errhandlingapi::{GetLastError, SetLastError},
        synchapi::CreateMutexW,
    };
    // `Local\` is the per session namespace, so the name is scoped to this
    // session already and cannot be squatted by another user.
    let name = wide_string(&format!("Local\\{}_tray", crate::get_app_name()));
    unsafe {
        // A successful `CreateMutexW` doesn't clear the last error, clear it to
        // reliably detect `ERROR_ALREADY_EXISTS`.
        SetLastError(0);
        // The handle is deliberately kept open for the lifetime of the process.
        let handle = CreateMutexW(null_mut(), FALSE, name.as_ptr());
        let last_error = GetLastError();
        if !handle.is_null() {
            if last_error == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                return false;
            }
            return true;
        }
        if last_error == ERROR_ACCESS_DENIED {
            // The mutex exists but was created by a tray running at a higher
            // integrity level, which is exactly the elevated tray described
            // above. Defer to it instead of adding a second icon.
            return false;
        }
        // Unexpected: show the tray icon anyway, a duplicated icon is better
        // than never showing the tray icon at all.
        log::warn!(
            "Failed to create the tray single instance mutex: {}",
            io::Error::from_raw_os_error(last_error as _)
        );
        true
    }
}
