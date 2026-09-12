use super::*;

pub struct CapturerMag {
    pub(super) mag_interface: MagInterface,
    pub(super) host_window: HWND,
    pub(super) magnifier_window: HWND,

    pub(super) magnifier_host_class: CString,
    pub(super) host_window_name: CString,
    pub(super) magnifier_window_class: CString,
    pub(super) magnifier_window_name: CString,

    pub(super) rect: RECT,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) excluded_window_target: Option<(String, String)>,
    pub(super) excluded_windows: Vec<HWND>,
}

impl Drop for CapturerMag {
    fn drop(&mut self) {
        self.destroy_windows();
        self.mag_interface.uninit();
    }
}

impl CapturerMag {
    pub(crate) fn is_supported() -> bool {
        MagInterface::new().is_ok()
    }

    // This captures through the legacy Windows Magnification API. Do not infer
    // multi-monitor capture support from privacy overlay coverage: WebRTC also
    // disables its magnifier capturer when SM_CMONITORS != 1.
    // https://webrtc.googlesource.com/src/+/1845922d5a1bf9c27deeffb4a8c8daea124434c1/modules/desktop_capture/win/screen_capturer_win_magnifier.cc
    pub(crate) fn new(origin: (i32, i32), width: usize, height: usize) -> Result<Self> {
        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if !(origin.0 >= x as i32
                && origin.1 >= y as i32
                && width <= w as usize
                && height <= h as usize)
            {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed Check screen rect ({}, {}, {} , {}) to ({}, {}, {}, {})",
                        origin.0,
                        origin.1,
                        origin.0 + width as i32,
                        origin.1 + height as i32,
                        x,
                        y,
                        x + w,
                        y + h
                    ),
                ));
            }
        }

        let mut s = Self {
            mag_interface: MagInterface::new()?,
            host_window: 0 as _,
            magnifier_window: 0 as _,
            magnifier_host_class: CString::new("ScreenCapturerWinMagnifierHost")?,
            host_window_name: CString::new("MagnifierHost")?,
            magnifier_window_class: CString::new("Magnifier")?,
            magnifier_window_name: CString::new("MagnifierWindow")?,
            rect: RECT {
                left: origin.0 as _,
                top: origin.1 as _,
                right: origin.0 + width as LONG,
                bottom: origin.1 + height as LONG,
            },
            width,
            height,
            excluded_window_target: None,
            excluded_windows: Vec::new(),
        };

        unsafe {
            let mut instance = 0 as HMODULE;
            if 0 == GetModuleHandleExA(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
                    | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                DefWindowProcA as _,
                &mut instance as _,
            ) {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed to GetModuleHandleExA, error {}",
                        Error::last_os_error()
                    ),
                ));
            }

            // Register the host window class. See the MSDN documentation of the
            // Magnification API for more information.
            let wcex = WNDCLASSEXA {
                cbSize: size_of::<WNDCLASSEXA>() as _,
                style: 0,
                lpfnWndProc: Some(DefWindowProcA),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: instance,
                hIcon: 0 as _,
                hCursor: LoadCursorA(NULL as _, IDC_ARROW as _),
                hbrBackground: 0 as _,
                lpszClassName: s.magnifier_host_class.as_ptr() as _,
                lpszMenuName: 0 as _,
                hIconSm: 0 as _,
            };

            // Ignore the error which may happen when the class is already registered.
            if 0 == RegisterClassExA(&wcex) {
                let code = GetLastError();
                if code != ERROR_CLASS_ALREADY_EXISTS {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Failed to RegisterClassExA, error {}",
                            Error::from_raw_os_error(code as _)
                        ),
                    ));
                }
            }

            // Create the host window.
            s.host_window = CreateWindowExA(
                WS_EX_LAYERED,
                s.magnifier_host_class.as_ptr(),
                s.host_window_name.as_ptr(),
                WS_POPUP,
                0,
                0,
                0,
                0,
                NULL as _,
                NULL as _,
                instance,
                NULL,
            );
            if s.host_window.is_null() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed to CreateWindowExA host_window, error {}",
                        Error::last_os_error()
                    ),
                ));
            }

            // Create the magnifier control.
            s.magnifier_window = CreateWindowExA(
                0,
                s.magnifier_window_class.as_ptr(),
                s.magnifier_window_name.as_ptr(),
                WS_CHILD | WS_VISIBLE,
                0,
                0,
                0,
                0,
                s.host_window,
                NULL as _,
                instance,
                NULL,
            );
            if s.magnifier_window.is_null() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed CreateWindowA magnifier_window, error {}",
                        Error::last_os_error()
                    ),
                ));
            }

            // Hide the host window.
            let _ = ShowWindow(s.host_window, SW_HIDE);

            // Set the scaling callback to receive captured image.
            if let Some(set_callback_func) = s.mag_interface.set_image_scaling_callback_func {
                if FALSE
                    == set_callback_func(
                        s.magnifier_window,
                        Some(Self::on_gag_image_scaling_callback),
                    )
                {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Failed to MagSetImageScalingCallback, error {}",
                            Error::last_os_error()
                        ),
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, set_image_scaling_callback_func should not be none",
                ));
            }
        }

        Ok(s)
    }
}
