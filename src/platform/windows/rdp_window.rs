use super::*;

pub fn wide_string(s: &str) -> Vec<u16> {
    use std::os::windows::prelude::OsStrExt;
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect()
}

// This only changes mstsc's top-level window title. The full-screen connection
// bar is rendered separately and cannot be customized when mstsc.exe is
// launched as an independent process.
pub fn set_rdp_window_title(mut child: std::process::Child, name: String) {
    let name: String = name.chars().filter(|c| !c.is_control()).take(120).collect();
    if name.is_empty() {
        return;
    }
    let process_id = child.id();
    // mstsc owns the title and can restore "localhost" while connecting or
    // reconnecting. Follow only the process we launched and reapply the peer
    // name until it exits, so concurrent RDP sessions cannot rename each other.
    if let Err(err) = std::thread::Builder::new()
        .name("rdp-window-title".to_owned())
        .spawn(move || {
            let mut warned = false;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Err(err) => {
                        log::warn!("Failed to query mstsc process: {}", err);
                        break;
                    }
                    Ok(None) => match set_process_rdp_window_title(process_id, &name) {
                        Ok(()) => warned = false,
                        Err(err) if !warned => {
                            log::warn!("Failed to set RDP window title: {}", err);
                            warned = true;
                        }
                        Err(_) => {}
                    },
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        })
    {
        log::warn!("Failed to start RDP window title thread: {}", err);
    }
}

pub(super) fn set_process_rdp_window_title(process_id: DWORD, name: &str) -> io::Result<()> {
    struct Context {
        process_id: DWORD,
        title: Vec<u16>,
        error: Option<io::Error>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let context = &mut *(lparam as *mut Context);
        let mut window_process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut window_process_id);
        if window_process_id != context.process_id || IsWindowVisible(hwnd) == FALSE {
            return TRUE;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return TRUE;
        }
        let mut title = vec![0u16; len as usize + 1];
        let len = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as _);
        if len > 0 && String::from_utf16_lossy(&title[..len as usize]).contains("localhost") {
            if SetWindowTextW(hwnd, context.title.as_ptr()) == FALSE {
                context.error = Some(io::Error::last_os_error());
                return FALSE;
            }
        }
        TRUE
    }

    let mut context = Context {
        process_id,
        title: wide_string(name),
        error: None,
    };
    let enumerated =
        unsafe { EnumWindows(Some(enum_window), &mut context as *mut Context as LPARAM) };
    if let Some(err) = context.error {
        return Err(err);
    }
    if enumerated == FALSE {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// send message to currently shown window
pub fn send_message_to_hnwd(
    class_name: &str,
    window_name: &str,
    dw_data: usize,
    data: &str,
    show_window: bool,
) -> bool {
    unsafe {
        let class_name_utf16 = wide_string(class_name);
        let window_name_utf16 = wide_string(window_name);
        let window = FindWindowW(class_name_utf16.as_ptr(), window_name_utf16.as_ptr());
        if window.is_null() {
            log::warn!("no such window {}:{}", class_name, window_name);
            return false;
        }
        let mut data_struct = COPYDATASTRUCT::default();
        data_struct.dwData = dw_data;
        let mut data_zero: String = data.chars().chain(Some('\0').into_iter()).collect();
        println!("send {:?}", data_zero);
        data_struct.cbData = data_zero.len() as _;
        data_struct.lpData = data_zero.as_mut_ptr() as _;
        SendMessageW(
            window,
            WM_COPYDATA,
            0,
            &data_struct as *const COPYDATASTRUCT as _,
        );
        if show_window {
            ShowWindow(window, SW_NORMAL);
            SetForegroundWindow(window);
        }
    }
    return true;
}
