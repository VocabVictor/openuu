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
