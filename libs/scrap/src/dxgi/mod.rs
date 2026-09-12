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

pub struct Displays {
    factory: ComPtr<IDXGIFactory1>,
    adapter: ComPtr<IDXGIAdapter1>,
    /// Index of the CURRENT adapter.
    nadapter: UINT,
    /// Index of the NEXT display to fetch.
    ndisplay: UINT,
}

impl Displays {
    pub fn new() -> io::Result<Displays> {
        let mut factory = ptr::null_mut();
        wrap_hresult(unsafe { CreateDXGIFactory1(&IID_IDXGIFactory1, &mut factory) })?;

        let factory = factory as *mut IDXGIFactory1;
        let mut adapter = ptr::null_mut();
        unsafe {
            // On error, our adapter is null, so it's fine.
            (*factory).EnumAdapters1(0, &mut adapter);
        };

        Ok(Displays {
            factory: ComPtr(factory),
            adapter: ComPtr(adapter),
            nadapter: 0,
            ndisplay: 0,
        })
    }

    pub fn get_from_gdi() -> Vec<Display> {
        let mut all = Vec::new();
        let mut i: DWORD = 0;
        loop {
            #[allow(invalid_value)]
            let mut d: DISPLAY_DEVICEW = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            d.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as _;
            let ok = unsafe { EnumDisplayDevicesW(std::ptr::null(), i, &mut d as _, 0) };
            if ok == FALSE {
                break;
            }
            i += 1;
            if 0 == (d.StateFlags & DISPLAY_DEVICE_ACTIVE)
                || (d.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER) > 0
            {
                continue;
            }
            // let is_primary = (d.StateFlags & DISPLAY_DEVICE_PRIMARY_DEVICE) > 0;
            let mut disp = Display {
                inner: ComPtr(std::ptr::null_mut()),
                adapter: ComPtr(std::ptr::null_mut()),
                desc: unsafe { std::mem::zeroed() },
                gdi: true,
            };
            disp.desc.DeviceName = d.DeviceName;
            #[allow(invalid_value)]
            let mut m: DEVMODEW = unsafe { std::mem::MaybeUninit::uninit().assume_init() };
            m.dmSize = std::mem::size_of::<DEVMODEW>() as _;
            m.dmDriverExtra = 0;
            let ok = unsafe {
                EnumDisplaySettingsExW(
                    disp.desc.DeviceName.as_ptr(),
                    ENUM_CURRENT_SETTINGS,
                    &mut m as _,
                    0,
                )
            };
            if ok == FALSE {
                continue;
            }
            disp.desc.DesktopCoordinates.left = unsafe { m.u1.s2().dmPosition.x };
            disp.desc.DesktopCoordinates.top = unsafe { m.u1.s2().dmPosition.y };
            disp.desc.DesktopCoordinates.right =
                disp.desc.DesktopCoordinates.left + m.dmPelsWidth as i32;
            disp.desc.DesktopCoordinates.bottom =
                disp.desc.DesktopCoordinates.top + m.dmPelsHeight as i32;
            disp.desc.AttachedToDesktop = 1;
            all.push(disp);
        }
        all
    }

    // No Adapter => Some(None)
    // Non-Empty Adapter => Some(Some(OUTPUT))
    // End of Adapter => None
    fn read_and_invalidate(&mut self) -> Option<Option<Display>> {
        // If there is no adapter, there is nothing left for us to do.

        if self.adapter.is_null() {
            return Some(None);
        }

        // Otherwise, we get the next output of the current adapter.

        let output = unsafe {
            let mut output = ptr::null_mut();
            (*self.adapter.0).EnumOutputs(self.ndisplay, &mut output);
            ComPtr(output)
        };

        // If the current adapter is done, we free it.
        // We return None so the caller gets the next adapter and tries again.

        if output.is_null() {
            self.adapter = ComPtr(ptr::null_mut());
            return None;
        }

        // Advance to the next display.

        self.ndisplay += 1;

        // We get the display's details.

        let desc = unsafe {
            #[allow(invalid_value)]
            let mut desc = mem::MaybeUninit::uninit().assume_init();
            (*output.0).GetDesc(&mut desc);
            desc
        };

        // We cast it up to the version needed for desktop duplication.

        let mut inner: *mut IDXGIOutput1 = ptr::null_mut();
        unsafe {
            (*output.0).QueryInterface(&IID_IDXGIOutput1, &mut inner as *mut *mut _ as *mut *mut _);
        }

        // If it's null, we have an error.
        // So we act like the adapter is done.

        if inner.is_null() {
            self.adapter = ComPtr(ptr::null_mut());
            return None;
        }

        unsafe {
            (*self.adapter.0).AddRef();
        }

        Some(Some(Display {
            inner: ComPtr(inner),
            adapter: ComPtr(self.adapter.0),
            desc,
            gdi: false,
        }))
    }
}

impl Iterator for Displays {
    type Item = Display;
    fn next(&mut self) -> Option<Display> {
        if let Some(res) = self.read_and_invalidate() {
            res
        } else {
            // We need to replace the adapter.

            self.ndisplay = 0;
            self.nadapter += 1;

            self.adapter = unsafe {
                let mut adapter = ptr::null_mut();
                (*self.factory.0).EnumAdapters1(self.nadapter, &mut adapter);
                ComPtr(adapter)
            };

            if let Some(res) = self.read_and_invalidate() {
                res
            } else {
                // All subsequent adapters will also be empty.
                None
            }
        }
    }
}

pub struct Display {
    inner: ComPtr<IDXGIOutput1>,
    adapter: ComPtr<IDXGIAdapter1>,
    desc: DXGI_OUTPUT_DESC,
    gdi: bool,
}

// optimized for updated region
// https://github.com/dchapyshev/aspia/blob/master/source/base/desktop/win/dxgi_output_duplicator.cc
// rotation
// https://github.com/bryal/dxgcap-rs/blob/master/src/lib.rs

impl Display {
    pub fn width(&self) -> LONG {
        self.desc.DesktopCoordinates.right - self.desc.DesktopCoordinates.left
    }

    pub fn height(&self) -> LONG {
        self.desc.DesktopCoordinates.bottom - self.desc.DesktopCoordinates.top
    }

    pub fn attached_to_desktop(&self) -> bool {
        self.desc.AttachedToDesktop != 0
    }

    pub fn rotation(&self) -> DXGI_MODE_ROTATION {
        self.desc.Rotation
    }

    fn create_gdi(&self) -> Option<CapturerGDI> {
        if let Ok(res) = CapturerGDI::new(self.name(), self.width(), self.height()) {
            Some(res)
        } else {
            None
        }
    }

    pub fn hmonitor(&self) -> HMONITOR {
        self.desc.Monitor
    }

    pub fn name(&self) -> &[u16] {
        let s = &self.desc.DeviceName;
        let i = s.iter().position(|&x| x == 0).unwrap_or(s.len());
        &s[..i]
    }

    pub fn is_online(&self) -> bool {
        self.desc.AttachedToDesktop != 0
    }

    pub fn origin(&self) -> (LONG, LONG) {
        (
            self.desc.DesktopCoordinates.left,
            self.desc.DesktopCoordinates.top,
        )
    }

    #[cfg(feature = "vram")]
    pub fn adapter_luid(&self) -> Option<i64> {
        unsafe {
            if !self.adapter.is_null() {
                #[allow(invalid_value)]
                let mut adapter_desc1 = mem::MaybeUninit::uninit().assume_init();
                if wrap_hresult((*self.adapter.0).GetDesc1(&mut adapter_desc1)).is_ok() {
                    let luid = ((adapter_desc1.AdapterLuid.HighPart as i64) << 32)
                        | adapter_desc1.AdapterLuid.LowPart as i64;
                    return Some(luid);
                }
            }
            None
        }
    }
}

fn wrap_hresult(x: HRESULT) -> io::Result<()> {
    use std::io::ErrorKind::*;
    Err((match x {
        S_OK => return Ok(()),
        DXGI_ERROR_ACCESS_LOST => ConnectionReset,
        DXGI_ERROR_WAIT_TIMEOUT => TimedOut,
        DXGI_ERROR_INVALID_CALL => InvalidData,
        E_ACCESSDENIED => PermissionDenied,
        DXGI_ERROR_UNSUPPORTED => ConnectionRefused,
        DXGI_ERROR_NOT_CURRENTLY_AVAILABLE => Interrupted,
        DXGI_ERROR_SESSION_DISCONNECTED => ConnectionAborted,
        E_INVALIDARG => InvalidInput,
        _ => {
            // 0x8000ffff https://www.auslogics.com/en/articles/windows-10-update-error-0x8000ffff-fixed/
            return Err(io::Error::new(Other, format!("Error code: {:#X}", x)));
        }
    })
    .into())
}

struct Rotate {
    video_context: ComPtr<ID3D11VideoContext>,
    video_device: ComPtr<ID3D11VideoDevice>,
    video_processor_enum: ComPtr<ID3D11VideoProcessorEnumerator>,
    video_processor: ComPtr<ID3D11VideoProcessor>,
    texture: (ComPtr<ID3D11Texture2D>, bool),
}
