use super::*;

#[inline]
pub fn set_server_running(b: bool) {
    *SERVER_RUNNING.write().unwrap() = b;
}

// is server process, with "--server" args
#[inline]
pub fn is_server() -> bool {
    *IS_SERVER
}

#[inline]
pub fn need_fs_cm_send_files() -> bool {
    #[cfg(windows)]
    {
        is_server()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Android is scoped-storage only: the peer may never touch anything outside the app
/// workspace (`Config::get_home()`, i.e. the app-specific external files directory).
///
/// Every peer supplied path must be validated with this before it reaches the
/// filesystem, for reads, writes, renames, creations and deletions alike. The path is
/// resolved to its canonical form (of the deepest existing ancestor, so paths that are
/// about to be created are handled too) so symlinks cannot escape the workspace.
///
/// Only the `ReadDir` protocol action treats an empty path as the home directory.
/// Callers must opt in to that protocol-specific behavior with `allow_empty`.
#[cfg(target_os = "android")]
pub fn is_peer_path_allowed(path: &str, allow_empty: bool) -> bool {
    use std::path::{Component, Path, PathBuf};

    // Canonicalize the deepest existing ancestor and re-append the missing tail.
    fn resolve(path: &Path) -> Option<PathBuf> {
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        let mut base = path.to_path_buf();
        loop {
            if let Ok(mut resolved) = base.canonicalize() {
                while let Some(component) = tail.pop() {
                    resolved.push(component);
                }
                return Some(resolved);
            }
            tail.push(base.file_name()?.to_os_string());
            if !base.pop() {
                return None;
            }
        }
    }

    if path.is_empty() {
        return allow_empty;
    }
    let path = Path::new(path);
    // `..` is never needed by the protocol and would defeat the prefix check below.
    if !path.is_absolute() || path.components().any(|c| c == Component::ParentDir) {
        return false;
    }
    let home = Config::get_home();
    let home = home.canonicalize().unwrap_or(home);
    if home.as_os_str().is_empty() {
        return false;
    }
    // `Path::starts_with` compares whole components, and is true for equal paths.
    resolve(path).map_or(false, |target| target.starts_with(&home))
}

#[inline]
#[cfg(not(target_os = "android"))]
pub fn is_peer_path_allowed(_path: &str, _allow_empty: bool) -> bool {
    true
}

#[inline]
pub fn is_main() -> bool {
    *IS_MAIN
}

#[inline]
pub fn is_cm() -> bool {
    *IS_CM
}

// Is server logic running.
#[inline]
pub fn is_server_running() -> bool {
    *SERVER_RUNNING.read().unwrap()
}

pub fn run_me<T: AsRef<std::ffi::OsStr>>(args: Vec<T>) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "linux")]
    if let Ok(appdir) = std::env::var("APPDIR") {
        let appimage_cmd = std::path::Path::new(&appdir).join("AppRun");
        if appimage_cmd.exists() {
            log::info!("path: {:?}", appimage_cmd);
            return std::process::Command::new(appimage_cmd).args(&args).spawn();
        }
    }
    let cmd = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(cmd);
    #[cfg(windows)]
    let mut force_foreground = false;
    #[cfg(windows)]
    {
        let arg_strs = args
            .iter()
            .map(|x| x.as_ref().to_string_lossy())
            .collect::<Vec<_>>();
        if arg_strs == vec!["--install"] || arg_strs == &["--noinstall"] {
            cmd.env(crate::platform::SET_FOREGROUND_WINDOW, "1");
            force_foreground = true;
        }
    }
    let result = cmd.args(&args).spawn();
    match result.as_ref() {
        Ok(_child) =>
        {
            #[cfg(windows)]
            if force_foreground {
                unsafe { winapi::um::winuser::AllowSetForegroundWindow(_child.id() as u32) };
            }
        }
        Err(err) => log::error!("run_me: {err:?}"),
    }
    result
}

#[inline]
pub fn username() -> String {
    // fix bug of whoami
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return whoami::username().trim_end_matches('\0').to_owned();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return DEVICE_NAME.lock().unwrap().clone();
}

// Exactly the implementation of "whoami::hostname()".
// This wrapper is to suppress warnings.
#[inline(always)]
#[cfg(not(target_os = "ios"))]
pub fn whoami_hostname() -> String {
    let mut hostname = whoami::fallible::hostname().unwrap_or_else(|_| "localhost".to_string());
    hostname.make_ascii_lowercase();
    hostname
}

#[inline]
pub fn hostname() -> String {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        #[allow(unused_mut)]
        let mut name = whoami_hostname();
        // some time, there is .local, some time not, so remove it for osx
        #[cfg(target_os = "macos")]
        if name.ends_with(".local") {
            name = name.trim_end_matches(".local").to_owned();
        }
        name
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return DEVICE_NAME.lock().unwrap().clone();
}

#[inline]
pub fn get_sysinfo() -> serde_json::Value {
    use hbb_common::sysinfo::System;
    let mut system = System::new();
    system.refresh_memory();
    system.refresh_cpu();
    let memory = system.total_memory();
    let memory = (memory as f64 / 1024. / 1024. / 1024. * 100.).round() / 100.;
    let cpus = system.cpus();
    let cpu_name = cpus.first().map(|x| x.brand()).unwrap_or_default();
    let cpu_name = cpu_name.trim_end();
    let cpu_freq = cpus.first().map(|x| x.frequency()).unwrap_or_default();
    let cpu_freq = (cpu_freq as f64 / 1024. * 100.).round() / 100.;
    let cpu = if cpu_freq > 0. {
        format!("{}, {}GHz, ", cpu_name, cpu_freq)
    } else {
        "".to_owned() // android
    };
    let num_cpus = num_cpus::get();
    let num_pcpus = num_cpus::get_physical();
    let mut os = system.distribution_id();
    os = format!("{} / {}", os, system.long_os_version().unwrap_or_default());
    #[cfg(windows)]
    {
        os = format!("{os} - {}", system.os_version().unwrap_or_default());
    }
    let hostname = hostname(); // sys.hostname() return localhost on android in my test
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let out;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let mut out;
    out = json!({
        "cpu": format!("{cpu}{num_cpus}/{num_pcpus} cores"),
        "memory": format!("{memory}GB"),
        "os": os,
        "hostname": hostname,
    });
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let username = crate::platform::get_active_username();
        if !username.is_empty() && (!cfg!(windows) || username != "SYSTEM") {
            out["username"] = json!(username);
        }
    }
    out
}

#[cfg(all(target_os = "windows", not(target_pointer_width = "64")))]
pub fn check_process(arg: &str, same_session_id: bool) -> bool {
    let mut path = std::env::current_exe().unwrap_or_default();
    if let Ok(linked) = path.read_link() {
        path = linked;
    }
    let Some(filename) = path.file_name() else {
        return false;
    };
    let filename = filename.to_string_lossy().to_string();
    match crate::platform::windows::get_pids_with_first_arg_check_session(
        &filename,
        arg,
        same_session_id,
    ) {
        Ok(pids) => {
            let self_pid = hbb_common::sysinfo::Pid::from_u32(std::process::id());
            pids.into_iter().filter(|pid| *pid != self_pid).count() > 0
        }
        Err(e) => {
            log::error!("Failed to check process with arg: \"{}\", {}", arg, e);
            false
        }
    }
}

#[allow(unused_mut)]
#[cfg(not(all(target_os = "windows", not(target_pointer_width = "64"))))]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn check_process(arg: &str, mut same_uid: bool) -> bool {
    #[cfg(target_os = "macos")]
    if !crate::platform::is_root() && !same_uid {
        log::warn!("Can not get other process's command line arguments on macos without root");
        same_uid = true;
    }
    use hbb_common::sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes();
    let mut path = std::env::current_exe().unwrap_or_default();
    if let Ok(linked) = path.read_link() {
        path = linked;
    }
    let path = path.to_string_lossy().to_lowercase();
    let my_uid = sys
        .process((std::process::id() as usize).into())
        .map(|x| x.user_id())
        .unwrap_or_default();
    for (_, p) in sys.processes().iter() {
        let mut cur_path = p.exe().to_path_buf();
        if let Ok(linked) = cur_path.read_link() {
            cur_path = linked;
        }
        if cur_path.to_string_lossy().to_lowercase() != path {
            continue;
        }
        if p.pid().to_string() == std::process::id().to_string() {
            continue;
        }
        if same_uid && p.user_id() != my_uid {
            continue;
        }
        // on mac, p.cmd() get "/Applications/RustDesk.app/Contents/MacOS/RustDesk", "XPC_SERVICE_NAME=com.carriez.RustDesk_server"
        let parg = if p.cmd().len() <= 1 { "" } else { &p.cmd()[1] };
        if arg.is_empty() {
            if !parg.starts_with("--") {
                return true;
            }
        } else if arg == parg {
            return true;
        }
    }
    false
}
