use super::*;

// Move this function into `src/platform/windows.rs` if there're more calls to load plugins.
// Load dll with full path.
#[cfg(target_os = "windows")]
pub(super) fn load_plugin_in_app_path(dll_name: &str) -> Result<Library, LibError> {
    match std::env::current_exe() {
        Ok(exe_file) => {
            if let Some(cur_dir) = exe_file.parent() {
                let full_path = cur_dir.join(dll_name);
                if !full_path.exists() {
                    Err(LibError::OpeningLibraryError(IoError::new(
                        IoErrorKind::NotFound,
                        format!("{} not found", dll_name),
                    )))
                } else {
                    Library::open(full_path)
                }
            } else {
                Err(LibError::OpeningLibraryError(IoError::new(
                    IoErrorKind::Other,
                    format!(
                        "Invalid exe parent for {}",
                        exe_file.to_string_lossy().as_ref()
                    ),
                )))
            }
        }
        Err(e) => Err(LibError::OpeningLibraryError(e)),
    }
}

/// FFI for rustdesk core's main entry.
/// Return true if the app should continue running with UI(possibly Flutter), false if the app should exit.
#[cfg(not(windows))]
#[no_mangle]
pub extern "C" fn rustdesk_core_main() -> bool {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if crate::core_main::core_main().is_some() {
        return true;
    } else {
        #[cfg(target_os = "macos")]
        std::process::exit(0);
    }
    #[cfg(not(target_os = "macos"))]
    false
}

#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn handle_applicationShouldOpenUntitledFile() {
    crate::platform::macos::handle_application_should_open_untitled_file();
}

#[cfg(windows)]
#[no_mangle]
pub extern "C" fn rustdesk_core_main_args(args_len: *mut c_int) -> *mut *mut c_char {
    unsafe { std::ptr::write(args_len, 0) };
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if let Some(args) = crate::core_main::core_main() {
            return rust_args_to_c_args(args, args_len);
        }
        return std::ptr::null_mut() as _;
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return std::ptr::null_mut() as _;
}

#[cfg(windows)]
#[no_mangle]
pub extern "C" fn rustdesk_is_disable_installation() -> c_int {
    hbb_common::config::is_disable_installation() as c_int
}

// https://gist.github.com/iskakaushik/1c5b8aa75c77479c33c4320913eebef6
#[cfg(windows)]
fn rust_args_to_c_args(args: Vec<String>, outlen: *mut c_int) -> *mut *mut c_char {
    let mut v = vec![];

    // Let's fill a vector with null-terminated strings
    for s in args {
        match CString::new(s) {
            Ok(s) => v.push(s),
            Err(_) => return std::ptr::null_mut() as _,
        }
    }

    // Turning each null-terminated string into a pointer.
    // `into_raw` takes ownershop, gives us the pointer and does NOT drop the data.
    let mut out = v.into_iter().map(|s| s.into_raw()).collect::<Vec<_>>();

    // Make sure we're not wasting space.
    out.shrink_to_fit();
    debug_assert!(out.len() == out.capacity());

    // Get the pointer to our vector.
    let len = out.len();
    let ptr = out.as_mut_ptr();
    std::mem::forget(out);

    // Let's write back the length the caller can expect
    unsafe { std::ptr::write(outlen, len as c_int) };

    // Finally return the data
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn free_c_args(ptr: *mut *mut c_char, len: c_int) {
    let len = len as usize;

    // Get back our vector.
    // Previously we shrank to fit, so capacity == length.
    let v = Vec::from_raw_parts(ptr, len, len);

    // Now drop one string at a time.
    for elem in v {
        let s = CString::from_raw(elem);
        std::mem::drop(s);
    }

    // Afterwards the vector will be dropped and thus freed.
}

#[cfg(windows)]
#[no_mangle]
pub unsafe extern "C" fn get_rustdesk_app_name(buffer: *mut u16, length: i32) -> i32 {
    let name = crate::platform::wide_string(&crate::get_app_name());
    if length > name.len() as i32 {
        std::ptr::copy_nonoverlapping(name.as_ptr(), buffer, name.len());
        return 0;
    }
    -1
}
