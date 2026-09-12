use super::*;

pub type REFWICPixelFormatGUID = *const GUID;
pub type WICPixelFormatGUID = GUID;

#[allow(non_snake_case)]
#[repr(C)]
#[derive(Copy, Clone)]
pub struct tagMAGIMAGEHEADER {
    pub width: UINT,
    pub height: UINT,
    pub format: WICPixelFormatGUID,
    pub stride: UINT,
    pub offset: UINT,
    pub cbSize: SIZE_T,
}
pub type MAGIMAGEHEADER = tagMAGIMAGEHEADER;
pub type PMAGIMAGEHEADER = *mut tagMAGIMAGEHEADER;

// Function types
pub type MagImageScalingCallback = ::std::option::Option<
    unsafe extern "C" fn(
        hwnd: HWND,
        srcdata: *mut ::std::os::raw::c_void,
        srcheader: MAGIMAGEHEADER,
        destdata: *mut ::std::os::raw::c_void,
        destheader: MAGIMAGEHEADER,
        unclipped: RECT,
        clipped: RECT,
        dirty: HRGN,
    ) -> BOOL,
>;

extern "C" {
    pub fn MagShowSystemCursor(fShowCursor: BOOL) -> BOOL;
}
pub type MagInitializeFunc = ::std::option::Option<unsafe extern "C" fn() -> BOOL>;
pub type MagUninitializeFunc = ::std::option::Option<unsafe extern "C" fn() -> BOOL>;
pub type MagSetWindowSourceFunc =
    ::std::option::Option<unsafe extern "C" fn(hwnd: HWND, rect: RECT) -> BOOL>;
pub type MagSetWindowFilterListFunc = ::std::option::Option<
    unsafe extern "C" fn(
        hwnd: HWND,
        dwFilterMode: DWORD,
        count: ::std::os::raw::c_int,
        pHWND: *mut HWND,
    ) -> BOOL,
>;
pub type MagSetImageScalingCallbackFunc = ::std::option::Option<
    unsafe extern "C" fn(hwnd: HWND, callback: MagImageScalingCallback) -> BOOL,
>;
