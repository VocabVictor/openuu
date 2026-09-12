use hbb_common::{log, thiserror};
use std::{
    ffi::OsStr,
    io,
    ops::{Deref, DerefMut},
    os::windows::ffi::OsStrExt,
    ptr::null_mut,
    result::Result,
};
use winapi::{
    shared::{
        guiddef::GUID,
        minwindef::{BOOL, DWORD, FALSE, MAX_PATH, PBOOL, TRUE},
        ntdef::{HANDLE, LPCWSTR, NULL},
        windef::HWND,
        winerror::{ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS},
    },
    um::{
        cfgmgr32::MAX_DEVICE_ID_LEN,
        fileapi::{CreateFileW, OPEN_EXISTING},
        handleapi::{CloseHandle, INVALID_HANDLE_VALUE},
        ioapiset::DeviceIoControl,
        setupapi::*,
        winnt::{GENERIC_READ, GENERIC_WRITE},
    },
};

mod install;
pub use install::*;
mod uninstall;
pub use uninstall::*;
mod device_io;
pub use device_io::*;
#[link(name = "Newdev")]
extern "system" {
    fn UpdateDriverForPlugAndPlayDevicesW(
        hwnd_parent: HWND,
        hardware_id: LPCWSTR,
        full_inf_path: LPCWSTR,
        install_flags: DWORD,
        b_reboot_required: PBOOL,
    ) -> BOOL;
}

#[derive(thiserror::Error, Debug)]
pub enum DeviceError {
    #[error("Failed to call {0}, {1:?}")]
    WinApiLastErr(String, io::Error),
    #[error("Failed to call {0}, returns {1}")]
    WinApiErrCode(String, DWORD),
    #[error("{0}")]
    Raw(String),
}

impl DeviceError {
    #[inline]
    fn new_api_last_err(api: &str) -> Self {
        Self::WinApiLastErr(api.to_string(), io::Error::last_os_error())
    }
}

struct DeviceInfo(HDEVINFO);

impl DeviceInfo {
    fn setup_di_create_device_info_list(class_guid: &mut GUID) -> Result<Self, DeviceError> {
        let dev_info = unsafe { SetupDiCreateDeviceInfoList(class_guid, null_mut()) };
        if dev_info == null_mut() {
            return Err(DeviceError::new_api_last_err("SetupDiCreateDeviceInfoList"));
        }

        Ok(Self(dev_info))
    }

    fn setup_di_get_class_devs_ex_w(
        class_guid: *const GUID,
        flags: DWORD,
    ) -> Result<Self, DeviceError> {
        let dev_info = unsafe {
            SetupDiGetClassDevsExW(
                class_guid,
                null_mut(),
                null_mut(),
                flags,
                null_mut(),
                null_mut(),
                null_mut(),
            )
        };
        if dev_info == null_mut() {
            return Err(DeviceError::new_api_last_err("SetupDiGetClassDevsExW"));
        }
        Ok(Self(dev_info))
    }
}

impl Drop for DeviceInfo {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

impl Deref for DeviceInfo {
    type Target = HDEVINFO;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DeviceInfo {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
