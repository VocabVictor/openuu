use super::*;

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

    pub(super) fn refresh_excluded_windows(&mut self) -> Result<()> {
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

    pub(super) fn exclude_windows(&mut self, hwnds: &mut [HWND]) -> Result<bool> {
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

    pub(super) fn clear_data() {
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

    pub(super) fn destroy_windows(&mut self) {
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

    pub(super) unsafe extern "C" fn on_gag_image_scaling_callback(
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
