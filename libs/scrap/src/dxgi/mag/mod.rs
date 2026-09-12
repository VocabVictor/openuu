// logic from webrtc -- https://github.com/shiguredo/libwebrtc/blob/main/modules/desktop_capture/win/screen_capturer_win_magnifier.cc
#![allow(non_snake_case)]

use lazy_static;
use std::{
    ffi::CString,
    io::{Error, ErrorKind, Result},
    mem::size_of,
    sync::Mutex,
};
use winapi::{
    shared::{
        basetsd::SIZE_T,
        guiddef::{IsEqualGUID, GUID},
        minwindef::{BOOL, DWORD, FALSE, FARPROC, HINSTANCE, HMODULE, HRGN, TRUE, UINT},
        ntdef::{LONG, NULL},
        windef::{HWND, RECT},
        winerror::ERROR_CLASS_ALREADY_EXISTS,
    },
    um::{
        errhandlingapi::GetLastError,
        libloaderapi::{FreeLibrary, GetModuleHandleExA, GetProcAddress, LoadLibraryExA},
        winuser::*,
    },
};

pub const MW_FILTERMODE_EXCLUDE: u32 = 0;
pub const MW_FILTERMODE_INCLUDE: u32 = 1;
pub const GET_MODULE_HANDLE_EX_FLAG_PIN: u32 = 1;
pub const GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT: u32 = 2;
pub const GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS: u32 = 4;
pub const LOAD_LIBRARY_AS_DATAFILE: u32 = 2;
pub const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 8;
pub const LOAD_IGNORE_CODE_AUTHZ_LEVEL: u32 = 16;
pub const LOAD_LIBRARY_AS_IMAGE_RESOURCE: u32 = 32;
pub const LOAD_LIBRARY_AS_DATAFILE_EXCLUSIVE: u32 = 64;
pub const LOAD_LIBRARY_REQUIRE_SIGNED_TARGET: u32 = 128;
pub const LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR: u32 = 256;
pub const LOAD_LIBRARY_SEARCH_APPLICATION_DIR: u32 = 512;
pub const LOAD_LIBRARY_SEARCH_USER_DIRS: u32 = 1024;
pub const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 2048;
pub const LOAD_LIBRARY_SEARCH_DEFAULT_DIRS: u32 = 4096;
pub const LOAD_LIBRARY_SAFE_CURRENT_DIRS: u32 = 8192;
pub const LOAD_LIBRARY_SEARCH_SYSTEM32_NO_FORWARDER: u32 = 16384;
pub const LOAD_LIBRARY_OS_INTEGRITY_CONTINUITY: u32 = 32768;

mod mag_types;
pub use mag_types::*;
mod mag_interface;
use mag_interface::*;
mod capturer_new;
pub use capturer_new::*;

extern "C" {
    pub static GUID_WICPixelFormat32bppRGBA: GUID;
}

lazy_static::lazy_static! {
    static ref MAG_BUFFER: Mutex<(bool, Vec<u8>)> =  Default::default();
}

impl CapturerMag {
    pub(crate) fn exclude(&mut self, cls: &str, name: &str) -> Result<bool> {
        let mut hwnds = find_windows(cls, name)?;
        hwnds.sort_unstable_by_key(|hwnd| *hwnd as usize);
        self.excluded_window_target = Some((cls.to_owned(), name.to_owned()));
        if hwnds.is_empty() {
            self.excluded_windows.clear();
            return Ok(false);
        }

        self.exclude_windows(&mut hwnds)?;
        self.excluded_windows = hwnds;
        Ok(true)
    }

    fn refresh_excluded_windows(&mut self) -> Result<()> {
        let Some((cls, name)) = self.excluded_window_target.as_ref() else {
            return Ok(());
        };
        let mut hwnds = find_windows(cls, name)?;
        hwnds.sort_unstable_by_key(|hwnd| *hwnd as usize);
        // This runs from frame() because refreshed privacy overlays get new
        // HWNDs. It is only used on the legacy magnifier backend while privacy
        // mode is active; if it shows up as hot-path cost, throttle this check.
        // Keep the previous filter list while privacy windows are being recreated.
        if hwnds.is_empty() || hwnds == self.excluded_windows {
            return Ok(());
        }

        self.exclude_windows(&mut hwnds)?;
        self.excluded_windows = hwnds;
        Ok(())
    }

    fn exclude_windows(&mut self, hwnds: &mut [HWND]) -> Result<bool> {
        let count = hwnds.len() as _;
        unsafe {
            if let Some(set_window_filter_list_func) =
                self.mag_interface.set_window_filter_list_func
            {
                if FALSE
                    == set_window_filter_list_func(
                        self.magnifier_window,
                        MW_FILTERMODE_EXCLUDE,
                        count,
                        hwnds.as_mut_ptr(),
                    )
                {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Failed MagSetWindowFilterList for {} windows, error {}",
                            count,
                            Error::last_os_error()
                        ),
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, MagSetWindowFilterList should not be none",
                ));
            }
        }

        Ok(true)
    }

    pub(crate) fn get_rect(&self) -> ((i32, i32), usize, usize) {
        (
            (self.rect.left as _, self.rect.top as _),
            self.width as _,
            self.height as _,
        )
    }

    fn clear_data() {
        let mut lock = MAG_BUFFER.lock().unwrap();
        lock.0 = false;
        lock.1.clear();
    }

    pub(crate) fn frame(&mut self, data: &mut Vec<u8>) -> Result<()> {
        self.refresh_excluded_windows()?;
        Self::clear_data();

        unsafe {
            let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
            let w = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let h = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            if !(self.rect.left >= x as i32
                && self.rect.top >= y as i32
                && self.rect.right <= (x + w) as i32
                && self.rect.bottom <= (y + h) as i32)
            {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed Check screen rect ({}, {}, {} , {}) to ({}, {}, {}, {})",
                        self.rect.left,
                        self.rect.top,
                        self.rect.right,
                        self.rect.bottom,
                        x,
                        y,
                        x + w,
                        y + h
                    ),
                ));
            }

            if FALSE
                == SetWindowPos(
                    self.magnifier_window,
                    HWND_TOP,
                    self.rect.left,
                    self.rect.top,
                    self.rect.right - self.rect.left,
                    self.rect.bottom - self.rect.top,
                    0,
                )
            {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!(
                        "Failed SetWindowPos (x, y, w , h) - ({}, {}, {}, {}), error {}",
                        self.rect.left,
                        self.rect.top,
                        self.rect.right - self.rect.left,
                        self.rect.bottom - self.rect.top,
                        Error::last_os_error()
                    ),
                ));
            }

            // on_gag_image_scaling_callback will be called and fill in the
            // frame before set_window_source_func_ returns.
            if let Some(set_window_source_func) = self.mag_interface.set_window_source_func {
                if FALSE == set_window_source_func(self.magnifier_window, self.rect) {
                    return Err(Error::new(
                        ErrorKind::Other,
                        format!(
                            "Failed to MagSetWindowSource, error {}",
                            Error::last_os_error()
                        ),
                    ));
                }
            } else {
                return Err(Error::new(
                    ErrorKind::Other,
                    "Unreachable, set_window_source_func should not be none",
                ));
            }
        }

        let mut lock = MAG_BUFFER.lock().unwrap();
        if !lock.0 {
            return Err(Error::new(
                ErrorKind::Other,
                "No data captured by magnifier",
            ));
        }

        data.resize(lock.1.len(), 0);
        unsafe {
            std::ptr::copy_nonoverlapping(&mut lock.1[0], &mut data[0], data.len());
        }

        Ok(())
    }

    fn destroy_windows(&mut self) {
        if !self.magnifier_window.is_null() {
            unsafe {
                if FALSE == DestroyWindow(self.magnifier_window) {
                    //
                    println!(
                        "Failed DestroyWindow magnifier window, error {}",
                        Error::last_os_error()
                    )
                }
            }
        }
        self.magnifier_window = NULL as _;

        if !self.host_window.is_null() {
            unsafe {
                if FALSE == DestroyWindow(self.host_window) {
                    //
                    println!(
                        "Failed DestroyWindow host window, error {}",
                        Error::last_os_error()
                    )
                }
            }
        }
        self.host_window = NULL as _;
    }

    unsafe extern "C" fn on_gag_image_scaling_callback(
        _hwnd: HWND,
        srcdata: *mut ::std::os::raw::c_void,
        srcheader: MAGIMAGEHEADER,
        _destdata: *mut ::std::os::raw::c_void,
        _destheader: MAGIMAGEHEADER,
        _unclipped: RECT,
        _clipped: RECT,
        _dirty: HRGN,
    ) -> BOOL {
        Self::clear_data();

        if !IsEqualGUID(&srcheader.format, &GUID_WICPixelFormat32bppRGBA) {
            // log warning?
            return FALSE;
        }
        let mut lock = MAG_BUFFER.lock().unwrap();
        lock.1.resize(srcheader.cbSize, 0);
        std::ptr::copy_nonoverlapping(srcdata as _, &mut lock.1[0], srcheader.cbSize);
        lock.0 = true;
        TRUE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {
        let mut capture_mag = CapturerMag::new((0, 0), 1920, 1080).unwrap();
        capture_mag.exclude("", "RustDeskPrivacyWindow").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1000 * 10));
        let mut data = Vec::new();
        capture_mag.frame(&mut data).unwrap();
        println!("capture data len: {}", data.len());
    }
}
