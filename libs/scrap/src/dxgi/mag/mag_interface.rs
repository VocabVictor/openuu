use super::*;

pub(super) fn find_windows(cls: &str, name: &str) -> Result<Vec<HWND>> {
    let name_c = CString::new(name)?;
    let cls_c = if cls.is_empty() {
        None
    } else {
        Some(CString::new(cls)?)
    };
    let mut hwnds = Vec::new();
    unsafe {
        let mut after = NULL as _;
        loop {
            let hwnd = FindWindowExA(
                NULL as _,
                after,
                cls_c.as_ref().map_or(NULL as _, |c| c.as_ptr()),
                name_c.as_ptr(),
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

#[repr(C)]
#[derive(Debug, Clone)]
pub(super) struct MagInterface {
    pub(super) init_succeeded: bool,
    pub(super) lib_handle: HINSTANCE,
    pub mag_initialize_func: MagInitializeFunc,
    pub mag_uninitialize_func: MagUninitializeFunc,
    pub set_window_source_func: MagSetWindowSourceFunc,
    pub set_window_filter_list_func: MagSetWindowFilterListFunc,
    pub set_image_scaling_callback_func: MagSetImageScalingCallbackFunc,
}

// NOTE: MagInitialize and MagUninitialize should not be called in global init and uninit.
// If so, strange errors occur.
impl MagInterface {
    pub(super) fn new() -> Result<Self> {
        let mut s = MagInterface {
            init_succeeded: false,
            lib_handle: NULL as _,
            mag_initialize_func: None,
            mag_uninitialize_func: None,
            set_window_source_func: None,
            set_window_filter_list_func: None,
            set_image_scaling_callback_func: None,
        };
        s.init_succeeded = false;
        unsafe {
            // load lib
            let lib_file_name = "Magnification.dll";
            let lib_file_name_c = CString::new(lib_file_name)?;
            s.lib_handle = LoadLibraryExA(
                lib_file_name_c.as_ptr() as _,
                NULL,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            );
            if s.lib_handle.is_null() {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed to LoadLibraryExA {}, error {}",
                        lib_file_name,
                        Error::last_os_error()
                    ),
                ));
            };

            // load functions
            s.mag_initialize_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagInitialize",
            )?));
            s.mag_uninitialize_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagUninitialize",
            )?));
            s.set_window_source_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagSetWindowSource",
            )?));
            s.set_window_filter_list_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagSetWindowFilterList",
            )?));
            s.set_image_scaling_callback_func = Some(std::mem::transmute(Self::load_func(
                s.lib_handle,
                "MagSetImageScalingCallback",
            )?));

            // MagInitialize
            if let Some(init_func) = s.mag_initialize_func {
                if FALSE == init_func() {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!("Failed to MagInitialize, error {}", Error::last_os_error()),
                    ));
                } else {
                    s.init_succeeded = true;
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, mag_initialize_func should not be none",
                ));
            }
        }
        Ok(s)
    }

    unsafe fn load_func(lib_module: HMODULE, func_name: &str) -> Result<FARPROC> {
        let func_name_c = CString::new(func_name)?;
        let func = GetProcAddress(lib_module, func_name_c.as_ptr() as _);
        if func.is_null() {
            return Err(Error::new(
                ErrorKind::Other,
                format!(
                    "Failed to GetProcAddress {}, error {}",
                    func_name,
                    Error::last_os_error()
                ),
            ));
        }
        Ok(func)
    }

    pub(super) fn uninit(&mut self) {
        if self.init_succeeded {
            if let Some(uninit_func) = self.mag_uninitialize_func {
                unsafe {
                    if FALSE == uninit_func() {
                        println!("Failed MagUninitialize, error {}", Error::last_os_error())
                    }
                }
            }
            if !self.lib_handle.is_null() {
                unsafe {
                    if FALSE == FreeLibrary(self.lib_handle) {
                        println!("Failed FreeLibrary, error {}", Error::last_os_error())
                    }
                }
                self.lib_handle = NULL as _;
            }
        }
        self.init_succeeded = false;
    }
}

impl Drop for MagInterface {
    fn drop(&mut self) {
        self.uninit();
    }
}
