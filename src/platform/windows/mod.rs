use super::{CursorData, ResultType};
use crate::{
    common::PORTABLE_APPNAME_RUNTIME_ENV_KEY,
    custom_server::*,
    ipc,
    privacy_mode::win_topmost_window::{self, WIN_TOPMOST_INJECTED_PROCESS_EXE},
};
use base::message_proto::{DisplayInfo, Resolution, WindowsSession};
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail,
    config::{self, Config},
    libc::{c_int, wchar_t},
    log, sleep,
    sysinfo::{Pid, System},
    timeout, tokio,
};
use std::{
    collections::HashMap,
    ffi::{CString, OsString},
    fs,
    io::{self, prelude::*},
    mem,
    os::{
        windows::{ffi::OsStringExt, process::CommandExt},
    },
    path::*,
    ptr::null_mut,
    sync::{atomic::Ordering, Arc, Mutex},
    time::{Duration, Instant},
};
use wallpaper;
#[cfg(not(debug_assertions))]
use winapi::um::libloaderapi::{LoadLibraryExW, LOAD_LIBRARY_SEARCH_USER_DIRS};
use winapi::{
    ctypes::c_void,
    shared::{minwindef::*, ntdef::NULL, windef::*, winerror::*},
    um::{
        errhandlingapi::GetLastError,
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        libloaderapi::{
            GetProcAddress, LoadLibraryA, LoadLibraryExA, LOAD_LIBRARY_SEARCH_SYSTEM32,
        },
        minwinbase::STILL_ACTIVE,
        processthreadsapi::{
            GetCurrentProcess, GetCurrentProcessId, GetExitCodeProcess, OpenProcess,
            OpenProcessToken, ProcessIdToSessionId, PROCESS_INFORMATION, STARTUPINFOW,
        },
        securitybaseapi::{
            AllocateAndInitializeSid, DuplicateToken, EqualSid, FreeSid, GetTokenInformation,
        },
        shellapi::ShellExecuteW,
        sysinfoapi::{GetNativeSystemInfo, SYSTEM_INFO},
        winbase::*,
        wingdi::*,
        winnt::{
            SecurityImpersonation, TokenElevation, TokenGroups, TokenImpersonation, TokenType,
            DOMAIN_ALIAS_RID_ADMINS, ES_AWAYMODE_REQUIRED, ES_CONTINUOUS, ES_DISPLAY_REQUIRED,
            ES_SYSTEM_REQUIRED, HANDLE, PROCESS_ALL_ACCESS, PROCESS_QUERY_LIMITED_INFORMATION,
            PSID, SECURITY_BUILTIN_DOMAIN_RID, SECURITY_NT_AUTHORITY, SID_IDENTIFIER_AUTHORITY,
            TOKEN_ELEVATION, TOKEN_GROUPS, TOKEN_QUERY, TOKEN_TYPE,
        },
        winreg::HKEY_CURRENT_USER,
        winuser::*,
    },
};
use windows::Win32::{
    Foundation::{CloseHandle as WinCloseHandle, HANDLE as WinHANDLE},
    Security::{
        GetTokenInformation as WinGetTokenInformation, IsWellKnownSid, TokenUser,
        WinLocalSystemSid, TOKEN_QUERY as WIN_TOKEN_QUERY, TOKEN_USER,
    },
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    },
    System::Threading::{
        OpenProcess as WinOpenProcess, OpenProcessToken as WinOpenProcessToken,
        QueryFullProcessImageNameW as WinQueryFullProcessImageNameW,
        PROCESS_QUERY_LIMITED_INFORMATION as WIN_PROCESS_QUERY_LIMITED_INFORMATION,
    },
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};
use winreg::{enums::*, RegKey};

mod acl;
mod installer_handoff;
mod installer_shell;
mod msi_registry;
pub mod sessions;
mod cursor;
mod cursor_dc;
mod service;
mod process_launch;
mod sas_desktop;
mod session;
mod install_info;
mod install_cmds;
mod install;
mod uninstall;
mod custom_client;
mod dll_bootstrap;
mod shell;
mod elevation;
mod rdp_window;
mod logon_token;
mod display;
mod certificate;
mod input_chars;
mod wake_lock;
mod update;
mod update_msi;
mod update_staging;
mod misc;
mod process_query;
#[cfg(not(target_pointer_width = "64"))]
mod process_query_wmic;
pub use cursor::*;
use cursor_dc::*;
pub use service::*;
pub use process_launch::*;
pub use sas_desktop::*;
pub use session::*;
pub use install_info::*;
pub use install_cmds::*;
pub use install::*;
pub use uninstall::*;
pub use custom_client::*;
pub use dll_bootstrap::*;
pub use shell::*;
pub use elevation::*;
pub use rdp_window::*;
pub use logon_token::*;
pub use display::*;
pub use certificate::*;
pub use input_chars::*;
pub use wake_lock::*;
pub use update::*;
pub use update_msi::*;
pub use update_staging::*;
pub use misc::*;
pub use process_query::*;
#[cfg(not(target_pointer_width = "64"))]
pub(super) use process_query_wmic::*;
pub(crate) use acl::current_process_user_sid_string;
pub use acl::{
    set_path_permission, set_path_permission_for_portable_service_shmem_dir,
    set_path_permission_for_portable_service_shmem_file,
    validate_path_for_portable_service_shmem_dir,
};
use installer_handoff::run_cmds;
use installer_shell::{
    embedded_shortcut_commands, embedded_tray_shortcut_commands, escape_nested_cmd_ampersands,
    shortcut_bytes, validate_install_value,
};

pub const FLUTTER_RUNNER_WIN32_WINDOW_CLASS: &'static str = "FLUTTER_RUNNER_WIN32_WINDOW"; // main window, install window
pub const EXPLORER_EXE: &'static str = "explorer.exe";
pub const SET_FOREGROUND_WINDOW: &'static str = "SET_FOREGROUND_WINDOW";

const REG_NAME_INSTALL_DESKTOPSHORTCUTS: &str = "DESKTOPSHORTCUTS";
const REG_NAME_INSTALL_STARTMENUSHORTCUTS: &str = "STARTMENUSHORTCUTS";
const REG_NAME_MSI_PRODUCT_CODE: &str = "MsiProductCode";
const REG_NAME_UNINSTALL_STRING: &str = "UninstallString";
const REG_NAME_WINDOWS_INSTALLER: &str = "WindowsInstaller";
const MSI_WINDOWS_INSTALLER_VALUE: u32 = 1;
const MSI_EXIT_SUCCESS_REBOOT_INITIATED: u32 = 1641;
const MSI_EXIT_SUCCESS_REBOOT_REQUIRED: u32 = 3010;
const HKLM_PREFIX: &str = "HKEY_LOCAL_MACHINE\\";

extern "C" {
    fn get_current_session(rdp: BOOL) -> DWORD;
    fn is_session_locked(session_id: DWORD) -> BOOL;
    fn LaunchProcessWin(
        cmd: *const u16,
        session_id: DWORD,
        as_user: BOOL,
        show: BOOL,
        token_pid: &mut DWORD,
    ) -> HANDLE;
    fn GetSessionUserTokenWin(
        lphUserToken: LPHANDLE,
        dwSessionId: DWORD,
        as_user: BOOL,
        token_pid: &mut DWORD,
    ) -> BOOL;
    fn selectInputDesktop() -> BOOL;
    fn inputDesktopSelected() -> BOOL;
    fn is_windows_server() -> BOOL;
    fn is_windows_10_or_greater() -> BOOL;
    fn handleMask(
        out: *mut u8,
        mask: *const u8,
        width: i32,
        height: i32,
        bmWidthBytes: i32,
        bmHeight: i32,
    ) -> i32;
    fn drawOutline(out: *mut u8, in_: *const u8, width: i32, height: i32, out_size: i32);
    fn get_di_bits(out: *mut u8, dc: HDC, hbmColor: HBITMAP, width: i32, height: i32) -> i32;
    fn blank_screen(v: BOOL);
    fn win32_enable_lowlevel_keyboard(hwnd: HWND) -> i32;
    fn win32_disable_lowlevel_keyboard(hwnd: HWND);
    fn win_stop_system_key_propagate(v: BOOL);
    fn is_win_down() -> BOOL;
    fn is_local_system() -> BOOL;
    fn alloc_console_and_redirect();
    fn is_service_running_w(svc_name: *const u16) -> bool;
}

extern "system" {
    fn BlockInput(v: BOOL) -> BOOL;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test-only reusable Win32 HANDLE RAII helper.
    // If a future non-test path needs the same pattern, move it out of this test module.
    //
    // This struct is similar to `base::platform::windows::RAIIHandle`,
    // but `RAIIHandle` depends on `WinApi` crate, while this `HandleGuard` only depends on `windows` crate.
    struct HandleGuard(WinHANDLE);

    impl HandleGuard {
        #[inline]
        fn new(handle: WinHANDLE) -> Self {
            Self(handle)
        }

        #[inline]
        fn get(&self) -> WinHANDLE {
            self.0
        }
    }

    impl Drop for HandleGuard {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_invalid() {
                    let _ = WinCloseHandle(self.0);
                }
            }
        }
    }

    #[test]
    fn test_is_process_running_as_system_invalid_pid_errors() {
        assert!(is_process_running_as_system(u32::MAX).is_err());
    }

    #[test]
    fn test_is_process_running_as_system_matches_current_process_token_user() {
        let pid = unsafe { windows::Win32::System::Threading::GetCurrentProcessId() };
        let actual = is_process_running_as_system(pid).unwrap();

        let expected = unsafe {
            // Keep this test consistent: use only the `windows` crate APIs/types.
            let process = HandleGuard::new(
                WinOpenProcess(WIN_PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
                    .expect("WinOpenProcess should succeed for current process"),
            );
            let mut token = WinHANDLE::default();
            WinOpenProcessToken(process.get(), WIN_TOKEN_QUERY, &mut token)
                .expect("WinOpenProcessToken should succeed for current process");
            let token = HandleGuard::new(token);

            let mut token_user_size = 0u32;
            let _ = WinGetTokenInformation(token.get(), TokenUser, None, 0, &mut token_user_size);
            assert_ne!(token_user_size, 0, "TokenUser size should be non-zero");

            let mut buffer = vec![0u8; token_user_size as usize];
            WinGetTokenInformation(
                token.get(),
                TokenUser,
                Some(buffer.as_mut_ptr() as *mut core::ffi::c_void),
                token_user_size,
                &mut token_user_size,
            )
            .expect("WinGetTokenInformation(TokenUser) should succeed for current process");

            let min_size = std::mem::size_of::<TOKEN_USER>();
            assert!(
                buffer.len() >= min_size,
                "TokenUser buffer too small (got {}, need >= {})",
                buffer.len(),
                min_size
            );
            let token_user: TOKEN_USER =
                std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_USER);
            let expected = IsWellKnownSid(token_user.User.Sid, WinLocalSystemSid).as_bool();
            expected
        };

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_uninstall_cert() {
        println!("uninstall driver certs: {:?}", cert::uninstall_cert());
    }

    #[test]
    fn test_get_unicode_char_by_vk() {
        let chr = get_char_from_vk(0x41); // VK_A
        assert_eq!(chr, Some('a'));
        let chr = get_char_from_vk(VK_ESCAPE as u32); // VK_ESC
        assert_eq!(chr, None)
    }

    #[test]
    fn install_app_names_enforce_ascii_command_safety() {
        assert!(validate_install_app_name("RustDesk-Admin1").is_ok());
        for app_name in ["", "RustDesk_Admin", "RustDesk&whoami", "RustDesk应用"] {
            assert!(
                validate_install_app_name(app_name).is_err(),
                "unsafe application name was accepted: {app_name}"
            );
        }
    }

    #[test]
    fn vbs_files_use_utf16le_with_bom_and_crlf() {
        const EXPECTED: &[u8] = &[0xFF, 0xFE, b'a', 0, b'\r', 0, b'\n', 0, b'b', 0];
        let tip = format!("vbs_encoding_{}", uuid::Uuid::new_v4().simple());
        let path = write_vbs("a\nb".to_owned(), &tip).expect("VBS file should be written");
        let bytes = std::fs::read(&path).expect("VBS file should be readable");
        std::fs::remove_file(path).expect("VBS file should be removed");

        assert_eq!(bytes, EXPECTED);
    }

    #[cfg(not(target_pointer_width = "64"))]
    #[test]
    fn test_get_pids_with_args_from_wmic_output() {
        let output = r#"
CommandLine=
ProcessId=33796

CommandLine=
ProcessId=34668

CommandLine="C:\Program Files\testapp\TestApp.exe" --tray
ProcessId=13728

CommandLine="C:\Program Files\testapp\TestApp.exe"
ProcessId=10136
"#;
        let name = "testapp.exe";
        let args = vec!["--tray"];
        let pids = super::get_pids_with_args_from_wmic_output(
            String::from_utf8_lossy(output.as_bytes()),
            name,
            &args,
        );
        assert_eq!(pids.len(), 1);
        assert_eq!(pids[0].as_u32(), 13728);

        let args: Vec<&str> = vec![];
        let pids = super::get_pids_with_args_from_wmic_output(
            String::from_utf8_lossy(output.as_bytes()),
            name,
            &args,
        );
        assert_eq!(pids.len(), 1);
        assert_eq!(pids[0].as_u32(), 10136);

        let args = vec!["--other"];
        let pids = super::get_pids_with_args_from_wmic_output(
            String::from_utf8_lossy(output.as_bytes()),
            name,
            &args,
        );
        assert_eq!(pids.len(), 0);
    }

    #[cfg(not(target_pointer_width = "64"))]
    #[test]
    fn test_get_pids_with_first_arg_from_wmic_output() {
        let output = r#"
CommandLine=
ProcessId=33796

CommandLine=
ProcessId=34668

CommandLine="C:\Program Files\testapp\TestApp.exe" --tray
ProcessId=13728

CommandLine="C:\Program Files\testapp\TestApp.exe"
ProcessId=10136
    "#;
        let name = "testapp.exe";
        let arg = "--tray";
        let pids = super::get_pids_with_first_arg_from_wmic_output(
            String::from_utf8_lossy(output.as_bytes()),
            name,
            arg,
        );
        assert_eq!(pids.len(), 1);
        assert_eq!(pids[0].as_u32(), 13728);

        let arg = "";
        let pids = super::get_pids_with_first_arg_from_wmic_output(
            String::from_utf8_lossy(output.as_bytes()),
            name,
            arg,
        );
        assert_eq!(pids.len(), 1);
        assert_eq!(pids[0].as_u32(), 10136);

        let arg = "--other";
        let pids = super::get_pids_with_first_arg_from_wmic_output(
            String::from_utf8_lossy(output.as_bytes()),
            name,
            arg,
        );
        assert_eq!(pids.len(), 0);
    }
}
