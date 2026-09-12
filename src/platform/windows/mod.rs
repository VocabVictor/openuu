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

#[inline]
pub fn uninstall_cert() -> ResultType<()> {
    cert::uninstall_cert()
}

mod cert {
    use hbb_common::ResultType;

    extern "C" {
        fn DeleteRustDeskTestCertsW();
    }
    pub fn uninstall_cert() -> ResultType<()> {
        unsafe {
            DeleteRustDeskTestCertsW();
        }
        Ok(())
    }
}

#[inline]
pub fn get_char_from_vk(vk: u32) -> Option<char> {
    get_char_from_unicode(get_unicode_from_vk(vk)?)
}

pub fn get_char_from_unicode(unicode: u16) -> Option<char> {
    let buff = [unicode];
    if let Some(chr) = String::from_utf16(&buff[..1]).ok()?.chars().next() {
        if chr.is_control() {
            return None;
        } else {
            Some(chr)
        }
    } else {
        None
    }
}

pub fn get_unicode_from_vk(vk: u32) -> Option<u16> {
    const BUF_LEN: i32 = 32;
    let mut buff = [0_u16; BUF_LEN as usize];
    let buff_ptr = buff.as_mut_ptr();
    let len = unsafe {
        let current_window_thread_id = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        let layout = GetKeyboardLayout(current_window_thread_id);

        // refs: https://github.com/rustdesk-org/rdev/blob/25a99ce71ab42843ad253dd51e6a35e83e87a8a4/src/windows/keyboard.rs#L115
        let press_state = 129;
        let mut state: [BYTE; 256] = [0; 256];
        let shift_left = rdev::get_modifier(rdev::Key::ShiftLeft);
        let shift_right = rdev::get_modifier(rdev::Key::ShiftRight);
        if shift_left {
            state[VK_LSHIFT as usize] = press_state;
        }
        if shift_right {
            state[VK_RSHIFT as usize] = press_state;
        }
        if shift_left || shift_right {
            state[VK_SHIFT as usize] = press_state;
        }
        ToUnicodeEx(vk, 0x00, &state as _, buff_ptr, BUF_LEN, 0, layout)
    };
    if len == 1 {
        Some(buff[0])
    } else {
        None
    }
}

pub fn is_process_consent_running() -> ResultType<bool> {
    let output = std::process::Command::new("cmd")
        .args(&["/C", "tasklist | findstr consent.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok(output.status.success() && !output.stdout.is_empty())
}

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

/// Calculate the total size of a directory in KB
/// Does not follow symlinks to prevent directory traversal attacks.
fn get_directory_size_kb(path: &str) -> u64 {
    let mut total_size = 0u64;
    let mut stack = vec![PathBuf::from(path)];

    while let Some(current_path) = stack.pop() {
        let entries = match std::fs::read_dir(&current_path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let metadata = match std::fs::symlink_metadata(entry.path()) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };

            if metadata.is_symlink() {
                continue;
            }

            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total_size = total_size.saturating_add(metadata.len());
            }
        }
    }

    total_size / 1024
}

pub fn update_me(debug: bool) -> ResultType<()> {
    let app_name = crate::get_app_name();
    let src_exe = std::env::current_exe()?.to_string_lossy().to_string();
    let (subkey, path, _, exe) = get_install_info();
    let is_installed = std::fs::metadata(&exe).is_ok();
    if !is_installed {
        bail!("{} is not installed.", &app_name);
    }
    let is_msi = is_msi_installed().ok();
    let reg_msi_key = get_reg_msi_key(&subkey, is_msi)?;

    let app_exe_name = &format!("{}.exe", &app_name);
    // NOTE: The pids below are matched by command line, which can silently come
    // back empty even while the processes are running:
    // - a 32-bit build cannot read the command line of a 64-bit process, so it
    //   shells out to `wmic` instead (#11638), and `wmic` is no longer installed
    //   by default since Windows 11 24H2;
    // - a non-elevated process cannot read the command line of an elevated one.
    // The `taskkill` in the commands below matches by image name and is not
    // affected, but `*_sessions` are then empty, so `_restore_session_guard`
    // silently restores nothing and the update leaves the user without a tray
    // icon and main window until the app is launched again. Reading the command
    // line through `NtQueryInformationProcess` instead would fix the queries for
    // every caller.
    let main_window_pids =
        crate::platform::get_pids_of_process_with_args::<_, &str>(&app_exe_name, &[]);
    let main_window_sessions = main_window_pids
        .iter()
        .map(|pid| get_session_id_of_process(pid.as_u32()))
        .flatten()
        .collect::<Vec<_>>();
    kill_process_by_pids(&app_exe_name, main_window_pids)?;
    let tray_pids = crate::platform::get_pids_of_process_with_args(&app_exe_name, &["--tray"]);
    let tray_sessions = tray_pids
        .iter()
        .map(|pid| get_session_id_of_process(pid.as_u32()))
        .flatten()
        .collect::<Vec<_>>();
    kill_process_by_pids(&app_exe_name, tray_pids)?;
    let is_service_running = is_self_service_running();

    let mut version_major = "0";
    let mut version_minor = "0";
    let mut version_build = "0";
    let versions: Vec<&str> = crate::VERSION.split(".").collect();
    if versions.len() > 0 {
        version_major = versions[0];
    }
    if versions.len() > 1 {
        version_minor = versions[1];
    }
    if versions.len() > 2 {
        version_build = versions[2];
    }
    let version = crate::VERSION.replace("-", ".");
    let size = get_directory_size_kb(&path);
    let build_date = crate::BUILD_DATE;
    // Use the icon in the previous installation directory if possible.
    let display_icon = get_custom_icon("", &exe).unwrap_or(exe.to_string());

    fn get_reg_cmd(
        subkey: &str,
        is_msi: Option<bool>,
        display_icon: &str,
        version: &str,
        build_date: &str,
        version_major: &str,
        version_minor: &str,
        version_build: &str,
        size: u64,
    ) -> String {
        let reg_display_icon = if is_msi.unwrap_or(false) {
            "".to_string()
        } else {
            format!(
                "reg add {} /f /v DisplayIcon /t REG_SZ /d \"{}\"",
                subkey, display_icon
            )
        };
        format!(
            "
{reg_display_icon}
reg add {subkey} /f /v DisplayVersion /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v Version /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v BuildDate /t REG_SZ /d \"{build_date}\"
reg add {subkey} /f /v VersionMajor /t REG_DWORD /d {version_major}
reg add {subkey} /f /v VersionMinor /t REG_DWORD /d {version_minor}
reg add {subkey} /f /v VersionBuild /t REG_DWORD /d {version_build}
reg add {subkey} /f /v EstimatedSize /t REG_DWORD /d {size}
        "
        )
    }

    let reg_cmd = {
        let reg_cmd_main = get_reg_cmd(
            &subkey,
            is_msi,
            &display_icon,
            &version,
            &build_date,
            &version_major,
            &version_minor,
            &version_build,
            size,
        );
        let reg_cmd_msi = if let Some(reg_msi_key) = &reg_msi_key {
            // This is best-effort: failure may leave a stale version in the Windows app list,
            // but should not interrupt the update.
            format!("reg add {reg_msi_key} /f /v DisplayVersion /t REG_SZ /d \"{version}\"")
        } else {
            "".to_owned()
        };
        format!("{}{}", reg_cmd_main, reg_cmd_msi)
    };

    let filter = format!(" /FI \"PID ne {}\"", get_current_pid());
    let restore_service_cmd = if is_service_running {
        format!("sc start {}", &app_name)
    } else {
        "".to_owned()
    };

    // We do not try to remove all files in the old version.
    // Because I don't know whether additional files will be installed here after installation, such as drivers.
    // Just copy files to the installation directory works fine.
    //if exist \"{path}\" rd /s /q \"{path}\"
    // md \"{path}\"
    //
    // We need `taskkill` because:
    // 1. There may be some other processes like `rustdesk --connect` are running.
    // 2. Sometimes, the main window and the tray icon are showing
    // while I cannot find them by `tasklist` or the methods above.
    // There's should be 4 processes running: service, server, tray and main window.
    // But only 2 processes are shown in the tasklist.
    let cmds = format!(
        "
chcp 65001
sc stop {app_name}
taskkill /F /IM {app_name}.exe{filter}
{reg_cmd}
{copy_exe}
{rename_exe}
{remove_meta_toml}
{restore_service_cmd}
{sleep}
    ",
        app_name = app_name,
        copy_exe = copy_exe_cmd(&src_exe, &exe, &path)?,
        rename_exe = rename_exe_cmd(&src_exe, &path)?,
        remove_meta_toml = remove_meta_toml_cmd(is_msi.unwrap_or(true), &path),
        sleep = if debug { "timeout 300" } else { "" },
    );

    let _restore_session_guard = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let is_root = is_root();
            if tray_sessions.is_empty() {
                log::info!("No tray process found.");
            } else {
                log::info!(
                    "Try to restore the tray process..., sessions: {:?}",
                    &tray_sessions
                );
                // When not running as root, only spawn once since run_exe_direct
                // doesn't target specific sessions.
                let mut spawned_non_root_tray = false;
                for s in tray_sessions.clone().into_iter() {
                    if s != 0 {
                        // We need to check if is_root here because if `update_me()` is called from
                        // the main window running with administrator permission,
                        // `run_exe_in_session()` will fail with error 1314 ("A required privilege is
                        // not held by the client").
                        //
                        // This issue primarily affects the MSI-installed version running in Administrator
                        // session during testing, but we check permissions here to be safe.
                        if is_root {
                            allow_err!(run_exe_in_session(&exe, vec!["--tray"], s, true));
                        } else if !spawned_non_root_tray {
                            // Only spawn once for non-root since run_exe_direct doesn't take session parameter
                            allow_err!(run_exe_direct(&exe, vec!["--tray"], false));
                            spawned_non_root_tray = true;
                        }
                    }
                }
            }
            if main_window_sessions.is_empty() {
                log::info!("No main window process found.");
            } else {
                log::info!("Try to restore the main window process...");
                std::thread::sleep(std::time::Duration::from_millis(2000));
                // When not running as root, only spawn once since run_exe_direct
                // doesn't target specific sessions.
                let mut spawned_non_root_main = false;
                for s in main_window_sessions.clone().into_iter() {
                    if s != 0 {
                        if is_root {
                            allow_err!(run_exe_in_session(&exe, vec![], s, true));
                        } else if !spawned_non_root_main {
                            // Only spawn once for non-root since run_exe_direct doesn't take session parameter
                            allow_err!(run_exe_direct(&exe, vec![], false));
                            spawned_non_root_main = true;
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
        }),
    };

    run_cmds(cmds, debug, "update")?;

    std::thread::sleep(std::time::Duration::from_millis(2000));
    log::info!("Update completed.");

    Ok(())
}

fn normalize_msi_product_code(value: &str) -> Option<String> {
    let value = value.trim().trim_matches('"');
    let value = value.strip_prefix('{')?.strip_suffix('}')?;
    let product_code = uuid::Uuid::parse_str(value).ok()?;
    Some(format!("{{{}}}", product_code.hyphenated()).to_uppercase())
}

fn build_msi_uninstall_command(product_code: &str) -> String {
    format!(
        "set \"RUSTDESK_MSI_EXIT_CODE=\"\n\
MsiExec.exe /X {product_code} /norestart REBOOT=ReallySuppress\n\
set \"RUSTDESK_MSI_EXIT_CODE=%ERRORLEVEL%\"\n\
if \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_REQUIRED}\" echo MSI uninstall succeeded with a reboot recommendation; continuing without reboot.\n\
if \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_INITIATED}\" echo MSI uninstall succeeded with a reboot request; continuing without forcing reboot.\n\
if not \"%RUSTDESK_MSI_EXIT_CODE%\"==\"0\" if not \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_REQUIRED}\" if not \"%RUSTDESK_MSI_EXIT_CODE%\"==\"{MSI_EXIT_SUCCESS_REBOOT_INITIATED}\" exit /b %RUSTDESK_MSI_EXIT_CODE%\n\
ver > nul"
    )
}

fn get_reg_string_of(subkey: &str, name: &str) -> ResultType<Option<String>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = subkey.strip_prefix(HKLM_PREFIX).unwrap_or(subkey);
    let key = match hklm.open_subkey(path) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => bail!("Failed to open registry key {subkey}: {err}"),
    };
    match key.get_value::<String, _>(name) {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => bail!("Failed to read {name} from registry key {subkey}: {err}"),
    }
}

fn get_windows_installer_state(subkey: &str) -> ResultType<Option<bool>> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = subkey.strip_prefix(HKLM_PREFIX).unwrap_or(subkey);
    let key = match hklm.open_subkey(path) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => bail!("Failed to open registry key {subkey}: {err}"),
    };
    match key.get_value::<u32, _>(REG_NAME_WINDOWS_INSTALLER) {
        Ok(value) => Ok(Some(value == MSI_WINDOWS_INSTALLER_VALUE)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => bail!("Failed to read {REG_NAME_WINDOWS_INSTALLER} from {subkey}: {err}"),
    }
}

fn parse_msi_product_code_from_uninstall_string(
    uninstall_string: &str,
    subkey: &str,
) -> ResultType<Option<String>> {
    if !uninstall_string
        .to_ascii_lowercase()
        .contains("msiexec.exe")
    {
        return Ok(None);
    }
    let start = uninstall_string
        .rfind('{')
        .ok_or_else(|| anyhow!("MSI uninstall string has no product code in {subkey}"))?;
    let end = uninstall_string
        .rfind('}')
        .ok_or_else(|| anyhow!("MSI uninstall string has no product code in {subkey}"))?;
    if start >= end {
        bail!("Invalid MSI uninstall string in {subkey}");
    }
    let product_code = uninstall_string
        .get(start..=end)
        .and_then(normalize_msi_product_code)
        .ok_or_else(|| anyhow!("Invalid MSI uninstall string in {subkey}"))?;
    Ok(Some(product_code))
}

fn get_msi_product_code(subkey: &str, installer_state: Option<bool>) -> ResultType<Option<String>> {
    if installer_state == Some(false) {
        return Ok(None);
    }
    let product_code = get_reg_string_of(subkey, REG_NAME_MSI_PRODUCT_CODE)?;
    if let Some(product_code) = product_code.filter(|value| !value.is_empty()) {
        return normalize_msi_product_code(&product_code)
            .map(Some)
            .ok_or_else(|| anyhow!("Invalid MSI product code in {subkey}"));
    }

    let uninstall_string =
        get_reg_string_of(subkey, REG_NAME_UNINSTALL_STRING)?.unwrap_or_default();
    match parse_msi_product_code_from_uninstall_string(&uninstall_string, subkey)? {
        Some(product_code) => Ok(Some(product_code)),
        None if installer_state == Some(true) => {
            msi_registry::find_product_code(&crate::get_app_name())
        }
        None => Ok(None),
    }
}

fn is_msi_uninstall_entry_in_view(subkey: &str, wow: bool, app_name: &str) -> ResultType<bool> {
    let flags = KEY_READ
        | if wow {
            KEY_WOW64_32KEY
        } else {
            KEY_WOW64_64KEY
        };
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = subkey.strip_prefix(HKLM_PREFIX).unwrap_or(subkey);
    let key = match hklm.open_subkey_with_flags(path, flags) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(anyhow!("Failed to open registry key {subkey}: {err}")),
    };
    msi_registry::is_matching_entry(&key, app_name, subkey)
}

fn get_msi_uninstall_subkey(product_code: &str) -> ResultType<String> {
    let app_name = crate::get_app_name();
    let subkey = get_subkey(product_code, false);
    if is_msi_uninstall_entry_in_view(&subkey, false, &app_name)? {
        return Ok(subkey);
    }
    if is_msi_uninstall_entry_in_view(&subkey, true, &app_name)? {
        return Ok(get_subkey(product_code, true));
    }
    bail!("Matching native MSI uninstall entry {product_code} was not found")
}

fn get_reg_msi_key(subkey: &str, is_msi: Option<bool>) -> ResultType<Option<String>> {
    // Only proceed if it's a custom client and MSI is installed.
    // `is_msi.unwrap_or(true)` is intentional: subsequent code validates the registry,
    // hence no early return is required upon MSI detection failure.
    if !(crate::common::is_custom_client() && is_msi.unwrap_or(true)) {
        return Ok(None);
    }

    let Some(product_code) = get_msi_product_code(subkey, is_msi)? else {
        if is_msi == Some(true) {
            bail!("MSI product code was not found in {subkey}");
        }
        return Ok(None);
    };
    Ok(Some(get_msi_uninstall_subkey(&product_code)?))
}

// Double confirm the process name
fn kill_process_by_pids(name: &str, pids: Vec<Pid>) -> ResultType<()> {
    let name = name.to_lowercase();
    let s = System::new_all();
    // No need to check all names of `pids` first, and kill them then.
    // It's rare case that they're not matched.
    for pid in pids {
        if let Some(process) = s.process(pid) {
            if process.name().to_lowercase() != name {
                bail!("Failed to kill the process, the names are mismatched.");
            }
            if !process.kill() {
                bail!("Failed to kill the process");
            }
        } else {
            bail!("Failed to kill the process, the pid is not found");
        }
    }
    Ok(())
}

pub fn handle_custom_client_staging_dir_before_update(
    custom_client_staging_dir: &PathBuf,
) -> ResultType<()> {
    let Some(current_exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    else {
        bail!("Failed to get current exe directory");
    };

    // Clean up existing staging directory
    if custom_client_staging_dir.exists() {
        log::debug!(
            "Removing existing custom client staging directory: {:?}",
            custom_client_staging_dir
        );
        if let Err(e) = remove_custom_client_staging_dir(custom_client_staging_dir) {
            bail!(
                "Failed to remove existing custom client staging directory {:?}: {}",
                custom_client_staging_dir,
                e
            );
        }
    }

    let src_path = current_exe_dir.join("custom.txt");
    if src_path.exists() {
        // Verify that custom.txt is not a symlink before copying
        let metadata = match std::fs::symlink_metadata(&src_path) {
            Ok(m) => m,
            Err(e) => {
                bail!(
                    "Failed to read metadata for custom.txt at {:?}: {}",
                    src_path,
                    e
                );
            }
        };

        if metadata.is_symlink() {
            allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
            bail!(
                "custom.txt at {:?} is a symlink, refusing to stage for security reasons.",
                src_path
            );
        }

        if metadata.is_file() {
            if !custom_client_staging_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(custom_client_staging_dir) {
                    bail!("Failed to create parent directory {:?} when staging custom client files: {}", custom_client_staging_dir, e);
                }
            }
            let dst_path = custom_client_staging_dir.join("custom.txt");
            if let Err(e) = std::fs::copy(&src_path, &dst_path) {
                allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
                bail!(
                    "Failed to copy custom txt from {:?} to {:?}: {}",
                    src_path,
                    dst_path,
                    e
                );
            }
        } else {
            log::warn!(
                "custom.txt at {:?} is not a regular file, skipping.",
                src_path
            );
        }
    } else {
        log::info!("No custom txt found to stage for update.");
    }

    Ok(())
}

// Used for auto update and manual update in the main window.
pub fn update_to(file: &str) -> ResultType<()> {
    if file.ends_with(".exe") {
        let custom_client_staging_dir = get_custom_client_staging_dir();
        if crate::is_custom_client() {
            handle_custom_client_staging_dir_before_update(&custom_client_staging_dir)?;
        } else {
            // Clean up any residual staging directory from previous custom client
            allow_err!(remove_custom_client_staging_dir(&custom_client_staging_dir));
        }
        if !run_uac(file, "--update")? {
            bail!(
                "Failed to run the update exe with UAC, error: {:?}",
                std::io::Error::last_os_error()
            );
        }
    } else if file.ends_with(".msi") {
        if let Err(e) = update_me_msi(file, false) {
            bail!("Failed to run the update msi: {}", e);
        }
    } else {
        // unreachable!()
        bail!("Unsupported update file format: {}", file);
    }
    Ok(())
}

// Don't launch tray app when running with `\qn`.
// 1. Because `/qn` requires administrator permission and the tray app should be launched with user permission.
//   Or launching the main window from the tray app will cause the main window to be launched with administrator permission.
// 2. We are not able to launch the tray app if the UI is in the login screen.
// `fn update_me()` can handle the above cases, but for msi update, we need to do more work to handle the above cases.
//    1. Record the tray app session ids.
//    2. Do the update.
//    3. Restore the tray app sessions.
//    `1` and `3` must be done in custom actions.
//    We need also to handle the command line parsing to find the tray processes.
pub fn update_me_msi(msi: &str, quiet: bool) -> ResultType<()> {
    let quiet_args = if quiet { " /qn LAUNCH_TRAY_APP=N" } else { "" };
    let cmds =
        format!("chcp 65001 && msiexec /i \"{msi}\"{quiet_args} REBOOT=ReallySuppress /norestart");
    run_cmds(cmds, false, "update-msi")?;
    Ok(())
}

fn get_import_config(exe: &str) -> String {
    if config::is_outgoing_only() {
        return "".to_string();
    }
    let exe = escape_nested_cmd_ampersands(exe);
    let config_path = Config::file();
    let config_path = escape_nested_cmd_ampersands(config_path.to_str().unwrap_or(""));
    format!("
sc stop {app_name}
sc delete {app_name}
sc create {app_name} binpath= \"\\\"{exe}\\\" --import-config \\\"{config_path}\\\"\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
sc stop {app_name}
sc delete {app_name}
",
    app_name = crate::get_app_name(),
)
}

fn get_create_service(exe: &str) -> String {
    if config::is_outgoing_only() {
        return "".to_string();
    }
    let stop = Config::get_option("stop-service") == "Y";
    if stop {
        format!("
if exist \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\" del /f /q \"%PROGRAMDATA%\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
", app_name = crate::get_app_name())
    } else {
        let exe = escape_nested_cmd_ampersands(exe);
        format!("
sc create {app_name} binpath= \"\\\"{exe}\\\" --service\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
",
    app_name = crate::get_app_name())
    }
}

fn run_after_run_cmds(silent: bool) {
    let (_, _, _, exe) = get_install_info();
    if !silent {
        log::debug!("Spawn new window");
        allow_err!(std::process::Command::new("cmd")
            .args(&["/c", "timeout", "/t", "2", "&", &format!("{exe}")])
            .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
            .spawn());
    }
    if Config::get_option("stop-service") != "Y" {
        allow_err!(std::process::Command::new(&exe).arg("--tray").spawn());
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
}

#[inline]
pub fn try_remove_temp_update_files() {
    let temp_dir = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp_dir) else {
        log::debug!("Failed to read temp directory: {:?}", temp_dir);
        return;
    };

    let one_hour = std::time::Duration::from_secs(60 * 60);
    for entry in entries {
        if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                // Match files like rustdesk-*.msi or rustdesk-*.exe
                if file_name.starts_with("rustdesk-")
                    && (file_name.ends_with(".msi") || file_name.ends_with(".exe"))
                {
                    // Skip files modified within the last hour to avoid deleting files being downloaded
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            if let Ok(elapsed) = modified.elapsed() {
                                if elapsed < one_hour {
                                    continue;
                                }
                            }
                        }
                    }
                    if let Err(e) = std::fs::remove_file(&path) {
                        log::debug!("Failed to remove temp update file {:?}: {}", path, e);
                    } else {
                        log::info!("Removed temp update file: {:?}", path);
                    }
                }
            }
        }
    }
}

#[inline]
pub fn try_kill_broker() {
    allow_err!(std::process::Command::new("cmd")
        .arg("/c")
        .arg(&format!(
            "taskkill /F /IM {}",
            WIN_TOPMOST_INJECTED_PROCESS_EXE
        ))
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
        .spawn());
}

pub fn message_box(text: &str) {
    let mut text = text.to_owned();
    let nodialog = std::env::var("NO_DIALOG").unwrap_or_default() == "Y";
    if !text.ends_with("!") || nodialog {
        use arboard::Clipboard as ClipboardContext;
        match ClipboardContext::new() {
            Ok(mut ctx) => {
                ctx.set_text(&text).ok();
                if !nodialog {
                    text = format!("{}\n\nAbove text has been copied to clipboard", &text);
                }
            }
            _ => {}
        }
    }
    if nodialog {
        if std::env::var("PRINT_OUT").unwrap_or_default() == "Y" {
            println!("{text}");
        }
        if let Ok(x) = std::env::var("WRITE_TO_FILE") {
            if !x.is_empty() {
                allow_err!(std::fs::write(x, text));
            }
        }
        return;
    }
    let text = text
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    let caption = "OpenUU Output"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<u16>>();
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), MB_OK) };
}

pub fn alloc_console() {
    unsafe {
        alloc_console_and_redirect();
    }
}

fn get_license() -> Option<CustomServer> {
    let mut lic: CustomServer = Default::default();
    if let Ok(tmp) = get_license_from_exe_name() {
        lic = tmp;
    } else {
        // for back compatibility from migrating from <= 1.2.1 to 1.2.2
        lic.key = get_reg("Key");
        lic.host = get_reg("Host");
        lic.api = get_reg("Api");
    }
    if lic.key.is_empty() || lic.host.is_empty() {
        return None;
    }
    Some(lic)
}

pub struct WallPaperRemover {
    old_path: String,
}

impl WallPaperRemover {
    pub fn new() -> ResultType<Self> {
        let start = std::time::Instant::now();
        if !Self::need_remove() {
            bail!("already solid color");
        }
        let old_path = match Self::get_recent_wallpaper() {
            Ok(old_path) => old_path,
            Err(e) => {
                log::info!("Failed to get recent wallpaper: {:?}, use fallback", e);
                wallpaper::get().map_err(|e| anyhow!(e.to_string()))?
            }
        };
        Self::set_wallpaper(None)?;
        log::info!(
            "created wallpaper remover,  old_path: {:?},  elapsed: {:?}",
            old_path,
            start.elapsed(),
        );
        Ok(Self { old_path })
    }

    pub fn support() -> bool {
        wallpaper::get().is_ok() || !Self::get_recent_wallpaper().unwrap_or_default().is_empty()
    }

    fn get_recent_wallpaper() -> ResultType<String> {
        // SystemParametersInfoW may return %appdata%\Microsoft\Windows\Themes\TranscodedWallpaper, not real path and may not real cache
        // https://www.makeuseof.com/find-desktop-wallpapers-file-location-windows-11/
        // https://superuser.com/questions/1218413/write-to-current-users-registry-through-a-different-admin-account
        let (hkcu, sid) = if is_root() {
            let sid = get_current_process_session_id().ok_or(anyhow!("failed to get sid"))?;
            (RegKey::predef(HKEY_USERS), format!("{}\\", sid))
        } else {
            (RegKey::predef(HKEY_CURRENT_USER), "".to_string())
        };
        let explorer_key = hkcu.open_subkey_with_flags(
            &format!(
                "{}Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Wallpapers",
                sid
            ),
            KEY_READ,
        )?;
        Ok(explorer_key.get_value("BackgroundHistoryPath0")?)
    }

    fn need_remove() -> bool {
        if let Ok(wallpaper) = wallpaper::get() {
            return !wallpaper.is_empty();
        }
        false
    }

    fn set_wallpaper(path: Option<String>) -> ResultType<()> {
        wallpaper::set_from_path(&path.unwrap_or_default()).map_err(|e| anyhow!(e.to_string()))
    }
}

impl Drop for WallPaperRemover {
    fn drop(&mut self) {
        // If the old background is a slideshow, it will be converted into an image. AnyDesk does the same.
        allow_err!(Self::set_wallpaper(Some(self.old_path.clone())));
    }
}

fn get_uninstall_amyuni_idd() -> String {
    match std::env::current_exe() {
        Ok(path) => format!("\"{}\" --uninstall-amyuni-idd", path.to_str().unwrap_or("")),
        Err(e) => {
            log::warn!("Failed to get current exe path, cannot get command of uninstalling idd, Zzerror: {:?}", e);
            "".to_string()
        }
    }
}

#[inline]
pub fn is_self_service_running() -> bool {
    is_service_running(&crate::get_app_name())
}

pub fn is_service_running(service_name: &str) -> bool {
    unsafe {
        let service_name = wide_string(service_name);
        is_service_running_w(service_name.as_ptr() as _)
    }
}

pub fn is_x64() -> bool {
    const PROCESSOR_ARCHITECTURE_AMD64: u16 = 9;

    let mut sys_info = SYSTEM_INFO::default();
    unsafe {
        GetNativeSystemInfo(&mut sys_info as _);
    }
    unsafe { sys_info.u.s().wProcessorArchitecture == PROCESSOR_ARCHITECTURE_AMD64 }
}

pub fn release_arch_suffix() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("x86_64"),
        "aarch64" => Some("aarch64"),
        _ => None,
    }
}

pub fn try_kill_rustdesk_main_window_process() -> ResultType<()> {
    // Kill rustdesk.exe without extra arg, should only be called by --server
    // We can find the exact process which occupies the ipc, see more from https://github.com/winsiderss/systeminformer
    let app_name = crate::get_app_name().to_lowercase();
    log::info!("try kill main window process");
    use hbb_common::sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes();
    let my_uid = sys
        .process((std::process::id() as usize).into())
        .map(|x| x.user_id())
        .unwrap_or_default();
    let my_pid = std::process::id();
    if app_name.is_empty() {
        bail!("app name is empty");
    }
    for (_, p) in sys.processes().iter() {
        let p_name = p.name().to_lowercase();
        // name equal
        if !(p_name == app_name || p_name == app_name.clone() + ".exe") {
            continue;
        }
        // arg more than 1
        if p.cmd().len() < 1 {
            continue;
        }
        // first arg contain app name
        if !p.cmd()[0].to_lowercase().contains(&p_name) {
            continue;
        }
        // only one arg or the second arg is empty uni link
        let is_empty_uni = p.cmd().len() == 2 && crate::common::is_empty_uni_link(&p.cmd()[1]);
        if !(p.cmd().len() == 1 || is_empty_uni) {
            continue;
        }
        // skip self
        if p.pid().as_u32() == my_pid {
            continue;
        }
        // because we call it with --server, so we can check user_id, remove this if call it with user process
        if p.user_id() == my_uid {
            log::info!("user id equal, continue");
            continue;
        }
        log::info!("try kill process: {:?}, pid = {:?}", p.cmd(), p.pid());
        nt_terminate_process(p.pid().as_u32())?;
        log::info!("kill process success: {:?}, pid = {:?}", p.cmd(), p.pid());
        return Ok(());
    }
    bail!("failed to find rustdesk main window process");
}

fn nt_terminate_process(process_id: DWORD) -> ResultType<()> {
    type NtTerminateProcess = unsafe extern "system" fn(HANDLE, DWORD) -> DWORD;
    unsafe {
        let h_module = if is_win_10_or_greater() {
            LoadLibraryExA(
                CString::new("ntdll.dll")?.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        } else {
            LoadLibraryA(CString::new("ntdll.dll")?.as_ptr())
        };
        if !h_module.is_null() {
            let f_nt_terminate_process: NtTerminateProcess = std::mem::transmute(GetProcAddress(
                h_module,
                CString::new("NtTerminateProcess")?.as_ptr(),
            ));
            let h_token = OpenProcess(PROCESS_ALL_ACCESS, 0, process_id);
            if !h_token.is_null() {
                if f_nt_terminate_process(h_token, 1) == 0 {
                    log::info!("terminate process {} success", process_id);
                    CloseHandle(h_token);
                    return Ok(());
                } else {
                    CloseHandle(h_token);
                    bail!("NtTerminateProcess {} failed", process_id);
                }
            } else {
                bail!("OpenProcess {} failed", process_id);
            }
        } else {
            bail!("Failed to load ntdll.dll");
        }
    }
}

pub fn try_set_window_foreground(window: HWND) {
    let env_key = SET_FOREGROUND_WINDOW;
    if let Ok(value) = std::env::var(env_key) {
        if value == "1" {
            unsafe {
                SetForegroundWindow(window);
            }
            std::env::remove_var(env_key);
        }
    }
}

fn get_pids<S: AsRef<str>>(name: S) -> ResultType<Vec<u32>> {
    let name = name.as_ref().to_lowercase();
    let mut pids = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)?;
        if snapshot == WinHANDLE::default() {
            return Ok(pids);
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let proc_name = OsString::from_wide(&entry.szExeFile)
                    .to_string_lossy()
                    .to_lowercase();

                if proc_name.contains(&name) {
                    pids.push(entry.th32ProcessID);
                }

                if !Process32NextW(snapshot, &mut entry).is_ok() {
                    break;
                }
            }
        }

        let _ = WinCloseHandle(snapshot);
    }

    Ok(pids)
}

pub fn is_msi_installed() -> std::io::Result<bool> {
    let (subkey, _, _, _) = get_install_info();
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let install_key = hklm.open_subkey(subkey.strip_prefix(HKLM_PREFIX).unwrap_or(&subkey))?;
    Ok(MSI_WINDOWS_INSTALLER_VALUE
        == install_key.get_value::<u32, _>(REG_NAME_WINDOWS_INSTALLER)?)
}

pub fn is_cur_exe_the_installed() -> bool {
    let (_, _, _, exe) = get_install_info();
    // Check if is installed, because `exe` is the default path if is not installed.
    if !std::fs::metadata(&exe).is_ok() {
        return false;
    }
    let mut path = std::env::current_exe().unwrap_or_default();
    if let Ok(linked) = path.read_link() {
        path = linked;
    }
    let path = path.to_string_lossy().to_lowercase();
    path == exe.to_lowercase()
}

#[cfg(not(target_pointer_width = "64"))]
pub fn get_pids_with_first_arg_check_session<S1: AsRef<str>, S2: AsRef<str>>(
    name: S1,
    arg: S2,
    same_session_id: bool,
) -> ResultType<Vec<hbb_common::sysinfo::Pid>> {
    // Though `wmic` can return the sessionId, for simplicity we only return processid.
    let pids = get_pids_with_first_arg_by_wmic(name, arg);
    if !same_session_id {
        return Ok(pids);
    }
    let Some(cur_sid) = get_current_process_session_id() else {
        bail!("Can't get current process session id");
    };
    let mut same_session_pids = vec![];
    for pid in pids.into_iter() {
        let mut sid = 0;
        if unsafe { ProcessIdToSessionId(pid.as_u32(), &mut sid) == TRUE } {
            if sid == cur_sid {
                same_session_pids.push(pid);
            }
        } else {
            // Only log here, because this call almost never fails.
            log::warn!(
                "Failed to get session id of the process id, error: {:?}",
                std::io::Error::last_os_error()
            );
        }
    }
    Ok(same_session_pids)
}

#[cfg(not(target_pointer_width = "64"))]
fn get_pids_with_args_from_wmic_output<S2: AsRef<str>>(
    output: std::borrow::Cow<'_, str>,
    name: &str,
    args: &[S2],
) -> Vec<hbb_common::sysinfo::Pid> {
    // CommandLine=
    // ProcessId=33796
    //
    // CommandLine=
    // ProcessId=34668
    //
    // CommandLine="C:\Program Files\RustDesk\RustDesk.exe" --tray
    // ProcessId=13728
    //
    // CommandLine="C:\Program Files\RustDesk\RustDesk.exe"
    // ProcessId=10136
    let mut pids = Vec::new();
    let mut proc_found = false;
    for line in output.lines() {
        if line.starts_with("ProcessId=") {
            if proc_found {
                if let Ok(pid) = line["ProcessId=".len()..].trim().parse::<u32>() {
                    pids.push(hbb_common::sysinfo::Pid::from_u32(pid));
                }
                proc_found = false;
            }
        } else if line.starts_with("CommandLine=") {
            proc_found = false;
            let cmd = line["CommandLine=".len()..].trim().to_lowercase();
            if args.is_empty() {
                if cmd.ends_with(&name) || cmd.ends_with(&format!("{}\"", &name)) {
                    proc_found = true;
                }
            } else {
                proc_found = args.iter().all(|arg| cmd.contains(arg.as_ref()));
            }
        }
    }
    pids
}

// Note the args are not compared strictly, only check if the args are contained in the command line.
// If we want to check the args strictly, we need to parse the command line and compare each arg.
// Maybe we have to introduce some external crate like `shell_words` to do this.
#[cfg(not(target_pointer_width = "64"))]
pub(super) fn get_pids_with_args_by_wmic<S1: AsRef<str>, S2: AsRef<str>>(
    name: S1,
    args: &[S2],
) -> Vec<hbb_common::sysinfo::Pid> {
    let name = name.as_ref().to_lowercase();
    std::process::Command::new("wmic.exe")
        .args([
            "process",
            "where",
            &format!("name='{}'", name),
            "get",
            "commandline,processid",
            "/value",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| {
            get_pids_with_args_from_wmic_output::<S2>(
                String::from_utf8_lossy(&output.stdout),
                &name,
                args,
            )
        })
        .unwrap_or_default()
}

#[cfg(not(target_pointer_width = "64"))]
fn get_pids_with_first_arg_from_wmic_output(
    output: std::borrow::Cow<'_, str>,
    name: &str,
    arg: &str,
) -> Vec<hbb_common::sysinfo::Pid> {
    let mut pids = Vec::new();
    let mut proc_found = false;
    for line in output.lines() {
        if line.starts_with("ProcessId=") {
            if proc_found {
                if let Ok(pid) = line["ProcessId=".len()..].trim().parse::<u32>() {
                    pids.push(hbb_common::sysinfo::Pid::from_u32(pid));
                }
                proc_found = false;
            }
        } else if line.starts_with("CommandLine=") {
            proc_found = false;
            let cmd = line["CommandLine=".len()..].trim().to_lowercase();
            if cmd.is_empty() {
                continue;
            }
            if !arg.is_empty() && cmd.starts_with(arg) {
                proc_found = true;
            } else {
                for x in [&format!("{}\"", name), &format!("{}", name)] {
                    if cmd.contains(x) {
                        let cmd = cmd.split(x).collect::<Vec<_>>()[1..].join("");
                        if arg.is_empty() {
                            if cmd.trim().is_empty() {
                                proc_found = true;
                            }
                        } else if cmd.trim().starts_with(arg) {
                            proc_found = true;
                        }
                        break;
                    }
                }
            }
        }
    }
    pids
}

// Note the args are not compared strictly, only check if the args are contained in the command line.
// If we want to check the args strictly, we need to parse the command line and compare each arg.
// Maybe we have to introduce some external crate like `shell_words` to do this.
#[cfg(not(target_pointer_width = "64"))]
pub(super) fn get_pids_with_first_arg_by_wmic<S1: AsRef<str>, S2: AsRef<str>>(
    name: S1,
    arg: S2,
) -> Vec<hbb_common::sysinfo::Pid> {
    let name = name.as_ref().to_lowercase();
    let arg = arg.as_ref().to_lowercase();
    std::process::Command::new("wmic.exe")
        .args([
            "process",
            "where",
            &format!("name='{}'", name),
            "get",
            "commandline,processid",
            "/value",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| {
            get_pids_with_first_arg_from_wmic_output(
                String::from_utf8_lossy(&output.stdout),
                &name,
                &arg,
            )
        })
        .unwrap_or_default()
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
