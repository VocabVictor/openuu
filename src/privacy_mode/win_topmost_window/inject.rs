use super::*;

pub(super) unsafe fn inject_dll<'a>(hproc: HANDLE, hthread: HANDLE, dll_file: &'a str) -> ResultType<()> {
    let dll_file_utf16: Vec<u16> = dll_file.encode_utf16().chain(Some(0).into_iter()).collect();

    let buf = VirtualAllocEx(
        hproc,
        NULL as _,
        dll_file_utf16.len() * 2,
        MEM_COMMIT,
        PAGE_READWRITE,
    );
    if buf.is_null() {
        bail!("Failed VirtualAllocEx");
    }

    let mut written: usize = 0;
    if 0 == WriteProcessMemory(
        hproc,
        buf,
        dll_file_utf16.as_ptr() as _,
        dll_file_utf16.len() * 2,
        &mut written,
    ) {
        bail!("Failed WriteProcessMemory");
    }

    let kernel32_modulename = CString::new("kernel32")?;
    let hmodule = GetModuleHandleA(kernel32_modulename.as_ptr() as _);
    if hmodule.is_null() {
        bail!("Failed GetModuleHandleA");
    }

    let load_librarya_name = CString::new("LoadLibraryW")?;
    let load_librarya = GetProcAddress(hmodule, load_librarya_name.as_ptr() as _);
    if load_librarya.is_null() {
        bail!("Failed GetProcAddress of LoadLibraryW");
    }

    if 0 == QueueUserAPC(Some(std::mem::transmute(load_librarya)), hthread, buf as _) {
        bail!("Failed QueueUserAPC");
    }

    Ok(())
}

pub(super) fn wait_find_privacy_hwnds(msecs: u128) -> ResultType<Vec<HWND>> {
    wait_find_privacy_hwnds_impl(msecs, false)
}

pub(super) fn wait_find_visible_privacy_hwnds(msecs: u128) -> ResultType<Vec<HWND>> {
    wait_find_privacy_hwnds_impl(msecs, true)
}

fn privacy_window_wait_millis(base_millis: u128, monitor_count: usize) -> u128 {
    if base_millis == 0 {
        return 0;
    }
    // Privacy Mode 1 creates one overlay per monitor. Keep the single-monitor
    // wait as the base and add time for each extra overlay before coverage
    // verification times out.
    base_millis
        + (monitor_count.saturating_sub(1) as u128) * PRIVACY_WINDOW_WAIT_EXTRA_MONITOR_MILLIS
}

fn wait_find_privacy_hwnds_impl(msecs: u128, require_visible: bool) -> ResultType<Vec<HWND>> {
    // This verifies initial turn-on coverage. If displays change during this
    // short poll window, the DLL refreshes overlays asynchronously, while this
    // check may still time out against the geometry sampled here.
    let monitor_rects = get_monitor_rects()?;
    if monitor_rects.is_empty() {
        bail!("No privacy monitor found");
    }
    let msecs = privacy_window_wait_millis(msecs, monitor_rects.len());

    let tm_begin = Instant::now();
    loop {
        let hwnds = find_privacy_hwnds()?;
        let visible_hwnds = if require_visible {
            filter_visible_hwnds(&hwnds)
        } else {
            Vec::new()
        };
        let covered_hwnds = if require_visible {
            visible_hwnds.as_slice()
        } else {
            hwnds.as_slice()
        };
        let covered = count_covered_monitors(covered_hwnds, &monitor_rects);
        if covered == monitor_rects.len() {
            return Ok(if require_visible {
                visible_hwnds
            } else {
                hwnds
            });
        }

        if msecs == 0 || tm_begin.elapsed().as_millis() > msecs {
            let visible = if require_visible { "visible " } else { "" };
            bail!(
                "Expected {}privacy windows to cover {} monitors, covered {}, found {}",
                visible,
                monitor_rects.len(),
                covered,
                hwnds.len(),
            );
        }

        std::thread::sleep(Duration::from_millis(PRIVACY_WINDOW_POLL_INTERVAL_MILLIS));
    }
}

pub(super) fn find_privacy_hwnds() -> ResultType<Vec<HWND>> {
    let class_name = CString::new(PRIVACY_WINDOW_CLASS)?;
    let wndname = CString::new(PRIVACY_WINDOW_NAME)?;
    let mut hwnds = Vec::new();
    unsafe {
        let mut after = NULL as _;
        loop {
            let hwnd = FindWindowExA(
                NULL as _,
                after,
                class_name.as_ptr() as _,
                wndname.as_ptr() as _,
            );
            if hwnd.is_null() {
                break;
            }
            hwnds.push(hwnd);
            after = hwnd;
        }
    }
    Ok(hwnds)
}

fn filter_visible_hwnds(hwnds: &[HWND]) -> Vec<HWND> {
    hwnds
        .iter()
        .copied()
        .filter(|hwnd| unsafe { FALSE != IsWindowVisible(*hwnd) })
        .collect()
}

pub(super) fn set_privacy_windows_visible(hwnds: &[HWND], show: bool) -> ResultType<usize> {
    if hwnds.is_empty() {
        return Ok(0);
    };
    let message = if show {
        WM_RUSTDESK_SHOW_WINDOWS
    } else {
        WM_RUSTDESK_HIDE_WINDOWS
    };
    let mut posted = 0;
    let mut first_error = None;
    for &hwnd in hwnds {
        unsafe {
            if FALSE == PostMessageA(hwnd, message, 0, 0) {
                if first_error.is_none() {
                    first_error = Some(Error::last_os_error());
                }
            } else {
                posted += 1;
            }
        }
    }
    if let Some(error) = first_error {
        bail!(
            "Failed to post privacy window visibility message to all privacy windows, posted {}/{}, first error {}",
            posted,
            hwnds.len(),
            error,
        );
    }
    Ok(posted)
}

fn get_monitor_rects() -> ResultType<Vec<RECT>> {
    let mut rects = Vec::new();
    unsafe {
        if FALSE
            == EnumDisplayMonitors(
                NULL as _,
                NULL as _,
                Some(enum_monitor_rect_proc),
                &mut rects as *mut Vec<RECT> as LPARAM,
            )
        {
            bail!(
                "Failed EnumDisplayMonitors, error {}",
                Error::last_os_error()
            );
        }
    }
    Ok(rects)
}

unsafe extern "system" fn enum_monitor_rect_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let rects = &mut *(lparam as *mut Vec<RECT>);
    let mut monitor_info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as _,
        rcMonitor: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };
    if FALSE == GetMonitorInfoA(hmon, &mut monitor_info) {
        return FALSE;
    }
    rects.push(monitor_info.rcMonitor);
    TRUE
}

fn count_covered_monitors(hwnds: &[HWND], monitor_rects: &[RECT]) -> usize {
    let mut covered = 0;
    for monitor_rect in monitor_rects {
        for hwnd in hwnds {
            let mut window_rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            unsafe {
                if FALSE == GetWindowRect(*hwnd, &mut window_rect) {
                    log::warn!(
                        "Failed GetWindowRect for privacy window, error {}",
                        Error::last_os_error()
                    );
                    continue;
                }
            }
            if rect_covers(&window_rect, monitor_rect) {
                covered += 1;
                break;
            }
        }
    }
    covered
}

fn rect_covers(window_rect: &RECT, monitor_rect: &RECT) -> bool {
    window_rect.left <= monitor_rect.left
        && window_rect.top <= monitor_rect.top
        && window_rect.right >= monitor_rect.right
        && window_rect.bottom >= monitor_rect.bottom
}
