use super::*;

pub struct Displays {
    pub(super) factory: ComPtr<IDXGIFactory1>,
    pub(super) adapter: ComPtr<IDXGIAdapter1>,
    /// Index of the CURRENT adapter.
    pub(super) nadapter: UINT,
    /// Index of the NEXT display to fetch.
    pub(super) ndisplay: UINT,
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
    pub(super) fn read_and_invalidate(&mut self) -> Option<Option<Display>> {
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
