use std::{io, mem, ptr, slice};
pub mod gdi;
pub use gdi::CapturerGDI;
pub mod mag;

use winapi::{
    shared::{
        dxgi::*,
        dxgi1_2::*,
        dxgitype::*,
        minwindef::{DWORD, FALSE, TRUE, UINT},
        ntdef::LONG,
        windef::{HMONITOR, RECT},
        winerror::*,
        // dxgiformat::{DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_420_OPAQUE},
    },
    um::{
        d3d11::*, d3dcommon::D3D_DRIVER_TYPE_UNKNOWN, unknwnbase::IUnknown, wingdi::*,
        winnt::HRESULT, winuser::*,
    },
};

use crate::RotationMode::*;

use crate::{AdapterDevice, Frame, PixelBuffer};
use std::ffi::c_void;

mod capturer_new;
mod capturer_frame;
mod displays;
pub use displays::*;
mod display;
pub use display::*;

pub struct ComPtr<T>(*mut T);
impl<T> ComPtr<T> {
    fn is_null(&self) -> bool {
        self.0.is_null()
    }
}
impl<T> Drop for ComPtr<T> {
    fn drop(&mut self) {
        unsafe {
            if !self.is_null() {
                (*(self.0 as *mut IUnknown)).Release();
            }
        }
    }
}

pub struct Capturer {
    device: ComPtr<ID3D11Device>,
    display: Display,
    context: ComPtr<ID3D11DeviceContext>,
    duplication: ComPtr<IDXGIOutputDuplication>,
    fastlane: bool,
    surface: ComPtr<IDXGISurface>,
    texture: ComPtr<ID3D11Texture2D>,
    width: usize,
    height: usize,
    rotated: Vec<u8>,
    gdi_capturer: Option<CapturerGDI>,
    gdi_buffer: Vec<u8>,
    saved_raw_data: Vec<u8>, // for faster compare and copy
    output_texture: bool,
    adapter_desc1: DXGI_ADAPTER_DESC1,
    rotate: Rotate,
}

impl Capturer {
}
