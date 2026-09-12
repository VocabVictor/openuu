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
mod capturer_frame;
#[cfg(test)]
mod tests;

extern "C" {
    pub static GUID_WICPixelFormat32bppRGBA: GUID;
}

lazy_static::lazy_static! {
    static ref MAG_BUFFER: Mutex<(bool, Vec<u8>)> =  Default::default();
}

impl CapturerMag {
}
