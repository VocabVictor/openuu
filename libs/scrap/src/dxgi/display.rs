use super::*;

pub struct Display {
    pub(super) inner: ComPtr<IDXGIOutput1>,
    pub(super) adapter: ComPtr<IDXGIAdapter1>,
    pub(super) desc: DXGI_OUTPUT_DESC,
    pub(super) gdi: bool,
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

    pub(super) fn create_gdi(&self) -> Option<CapturerGDI> {
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

pub(super) fn wrap_hresult(x: HRESULT) -> io::Result<()> {
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

pub(super) struct Rotate {
    pub(super) video_context: ComPtr<ID3D11VideoContext>,
    pub(super) video_device: ComPtr<ID3D11VideoDevice>,
    pub(super) video_processor_enum: ComPtr<ID3D11VideoProcessorEnumerator>,
    pub(super) video_processor: ComPtr<ID3D11VideoProcessor>,
    pub(super) texture: (ComPtr<ID3D11Texture2D>, bool),
}
