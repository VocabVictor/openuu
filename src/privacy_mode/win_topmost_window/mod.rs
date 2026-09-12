use super::{PrivacyMode, INVALID_PRIVACY_MODE_CONN_ID};
use crate::{platform::windows::get_user_token, privacy_mode::PrivacyModeState};
use hbb_common::{allow_err, bail, log, ResultType};
use std::{
    ffi::CString,
    io::Error,
    mem::size_of,
    time::{Duration, Instant},
};
use winapi::{
    shared::{
        minwindef::{BOOL, FALSE, LPARAM, TRUE},
        ntdef::{HANDLE, NULL},
        windef::{HDC, HMONITOR, HWND, RECT},
    },
    um::{
        handleapi::CloseHandle,
        libloaderapi::{GetModuleHandleA, GetProcAddress},
        memoryapi::{VirtualAllocEx, WriteProcessMemory},
        processthreadsapi::{
            CreateProcessAsUserW, QueueUserAPC, ResumeThread, TerminateProcess,
            PROCESS_INFORMATION, STARTUPINFOW,
        },
        winbase::{WTSGetActiveConsoleSessionId, CREATE_SUSPENDED, DETACHED_PROCESS},
        winnt::{MEM_COMMIT, PAGE_READWRITE},
        winuser::*,
    },
};

mod privacy_impl;
mod inject;
use inject::{find_privacy_hwnds, inject_dll, set_privacy_windows_visible, wait_find_privacy_hwnds, wait_find_visible_privacy_hwnds};

pub(super) const PRIVACY_MODE_IMPL: &str = "privacy_mode_impl_mag";

pub const ORIGIN_PROCESS_EXE: &'static str = "C:\\Windows\\System32\\RuntimeBroker.exe";
pub const WIN_TOPMOST_INJECTED_PROCESS_EXE: &'static str = "RuntimeBroker_rustdesk.exe";
pub const INJECTED_PROCESS_EXE: &'static str = WIN_TOPMOST_INJECTED_PROCESS_EXE;
pub(super) const PRIVACY_WINDOW_CLASS: &'static str = "RustDeskPrivacyWindowClass";
pub(super) const PRIVACY_WINDOW_NAME: &'static str = "RustDeskPrivacyWindow";
const PRIVACY_WINDOW_WAIT_MILLIS: u128 = 1_000;
const PRIVACY_WINDOW_WAIT_EXTRA_MONITOR_MILLIS: u128 = 500;
const PRIVACY_WINDOW_POLL_INTERVAL_MILLIS: u64 = 100;
const WM_RUSTDESK_SHOW_WINDOWS: u32 = WM_APP + 3;
const WM_RUSTDESK_HIDE_WINDOWS: u32 = WM_APP + 4;

struct WindowHandlers {
    hthread: u64,
    hprocess: u64,
}

impl Drop for WindowHandlers {
    fn drop(&mut self) {
        self.reset();
    }
}

impl WindowHandlers {
    fn reset(&mut self) {
        unsafe {
            if self.hprocess != 0 {
                let _res = TerminateProcess(self.hprocess as _, 0);
                CloseHandle(self.hprocess as _);
            }
            self.hprocess = 0;
            if self.hthread != 0 {
                CloseHandle(self.hthread as _);
            }
            self.hthread = 0;
        }
    }

    fn is_default(&self) -> bool {
        self.hthread == 0 && self.hprocess == 0
    }
}

pub struct PrivacyModeImpl {
    impl_key: String,
    conn_id: i32,
    handlers: WindowHandlers,
    hwnd: u64,
}

impl PrivacyMode for PrivacyModeImpl {
    fn is_async_privacy_mode(&self) -> bool {
        false
    }

    fn init(&self) -> ResultType<()> {
        Ok(())
    }

    fn clear(&mut self) {
        allow_err!(self.turn_off_privacy(self.conn_id, None));
    }

    fn turn_on_privacy(&mut self, conn_id: i32) -> ResultType<bool> {
        if self.check_on_conn_id(conn_id)? {
            log::debug!("Privacy mode of conn {} is already on", conn_id);
            return Ok(true);
        }

        let exe_file = std::env::current_exe()?;
        if let Some(cur_dir) = exe_file.parent() {
            if !cur_dir.join("WindowInjection.dll").exists() {
                return Ok(false);
            }
        } else {
            bail!(
                "Invalid exe parent for {}",
                exe_file.to_string_lossy().as_ref()
            );
        }

        let should_start_broker = self.handlers.is_default();
        if should_start_broker {
            log::info!("turn_on_privacy, broker not running, try start");
            self.start()?;
            std::thread::sleep(std::time::Duration::from_millis(1_000));
        }

        if let Err(e) = self.show_privacy_windows(conn_id, true) {
            self.stop();
            return Err(e);
        }
        Ok(true)
    }

    fn turn_off_privacy(
        &mut self,
        conn_id: i32,
        state: Option<PrivacyModeState>,
    ) -> ResultType<()> {
        self.check_off_conn_id(conn_id)?;
        super::win_input::unhook()?;
        let hwnds = find_privacy_hwnds()?;
        let hide_result = set_privacy_windows_visible(&hwnds, false);
        if hide_result.is_err() {
            self.stop();
        }

        // Continue local state cleanup even after stop(); the broker has
        // been torn down, so keeping conn_id/hwnd would leave stale state.
        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            // Only publish the off state after the hide message was posted.
            // Otherwise the peer may receive a success-like state and then a
            // failed turn-off response for the same request.
            if hide_result.is_ok() {
                if let Some(state) = state {
                    allow_err!(super::set_privacy_mode_state(
                        conn_id,
                        state,
                        PRIVACY_MODE_IMPL.to_string(),
                        1_000
                    ));
                }
            }
            self.conn_id = INVALID_PRIVACY_MODE_CONN_ID.to_owned();
            self.hwnd = 0;
        }

        hide_result.map(|_| ())
    }

    #[inline]
    fn pre_conn_id(&self) -> i32 {
        self.conn_id
    }

    #[inline]
    fn get_impl_key(&self) -> &str {
        &self.impl_key
    }
}


