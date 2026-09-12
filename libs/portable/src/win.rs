use std::{fs, os::windows::process::CommandExt, path::Path, process::Command};

// Used for privacy mode(magnifier impl).
pub const RUNTIME_BROKER_EXE: &'static str = "C:\\Windows\\System32\\RuntimeBroker.exe";
pub const WIN_TOPMOST_INJECTED_PROCESS_EXE: &'static str = "RuntimeBroker_rustdesk.exe";

pub(super) fn copy_runtime_broker(dir: &Path) {
    let src = RUNTIME_BROKER_EXE;
    let tgt = WIN_TOPMOST_INJECTED_PROCESS_EXE;
    let target_file = dir.join(tgt);
    if target_file.exists() {
        if let (Ok(src_file), Ok(tgt_file)) = (fs::read(src), fs::read(&target_file)) {
            let src_md5 = format!("{:x}", md5::compute(&src_file));
            let tgt_md5 = format!("{:x}", md5::compute(&tgt_file));
            if src_md5 == tgt_md5 {
                return;
            }
        }
    }
    let _allow_err = Command::new("taskkill")
        .args(&["/F", "/IM", "RuntimeBroker_rustdesk.exe"])
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
        .output();
    let _allow_err = std::fs::copy(src, &format!("{}\\{}", dir.to_string_lossy(), tgt));
}

/// Check if the executable is a Quick Support version.
/// Note: This function must be kept in sync with `src/core_main.rs`.
#[inline]
pub(super) fn is_quick_support_exe(exe: &str) -> bool {
    let exe = exe.to_lowercase();
    exe.contains("-qs-") || exe.contains("-qs.exe") || exe.contains("_qs.exe")
}
