use super::*;

impl Capturer {
    pub fn is_gdi(&self) -> bool {
        self.gdi_capturer.is_some()
    }

    pub fn set_gdi(&mut self) -> bool {
        self.gdi_capturer = self.display.create_gdi();
        self.is_gdi()
    }

    pub fn cancel_gdi(&mut self) {
        self.gdi_buffer = Vec::new();
        self.gdi_capturer.take();
    }

    #[cfg(feature = "vram")]
    pub fn set_output_texture(&mut self, texture: bool) {
        self.output_texture = texture;
    }

    unsafe fn load_frame(&mut self, timeout: UINT) -> io::Result<(*const u8, i32)> {
        let mut frame = ptr::null_mut();
        #[allow(invalid_value)]
        let mut info = mem::MaybeUninit::uninit().assume_init();

        wrap_hresult((*self.duplication.0).AcquireNextFrame(timeout, &mut info, &mut frame))?;
        let frame = ComPtr(frame);

        if *info.LastPresentTime.QuadPart() == 0 {
            return Err(std::io::ErrorKind::WouldBlock.into());
        }

        #[allow(invalid_value)]
        let mut rect = mem::MaybeUninit::uninit().assume_init();
        let mapped = if self.fastlane {
            // Refused when the desktop image is not in system memory. The flag that put us
            // on this path is what the duplication reported when it was created, and a
            // driver is free to keep the image elsewhere afterwards; the copy below works
            // either way. Giving up on the map as a capture error instead costs the
            // session its duplication: the caller falls back to GDI, which compares and
            // copies the whole frame on every capture from then on.
            wrap_hresult((*self.duplication.0).MapDesktopSurface(&mut rect))
                .inspect_err(|err| {
                    hbb_common::log::info!(
                        "dxgi: the desktop surface cannot be mapped ({err}), copying it instead"
                    );
                    self.fastlane = false;
                })
                .is_ok()
        } else {
            false
        };
        if !mapped {
            self.surface = ComPtr(self.ohgodwhat(frame.0)?);
            wrap_hresult((*self.surface.0).Map(&mut rect, DXGI_MAP_READ))?;
        }
        Ok((rect.pBits, rect.Pitch))
    }

    // copy from GPU memory to system memory
    unsafe fn ohgodwhat(&mut self, frame: *mut IDXGIResource) -> io::Result<*mut IDXGISurface> {
        let mut texture: *mut ID3D11Texture2D = ptr::null_mut();
        (*frame).QueryInterface(
            &IID_ID3D11Texture2D,
            &mut texture as *mut *mut _ as *mut *mut _,
        );
        let texture = ComPtr(texture);

        #[allow(invalid_value)]
        let mut texture_desc = mem::MaybeUninit::uninit().assume_init();
        (*texture.0).GetDesc(&mut texture_desc);

        texture_desc.Usage = D3D11_USAGE_STAGING;
        texture_desc.BindFlags = 0;
        texture_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ;
        texture_desc.MiscFlags = 0;

        let mut readable = ptr::null_mut();
        wrap_hresult((*self.device.0).CreateTexture2D(
            &mut texture_desc,
            ptr::null(),
            &mut readable,
        ))?;
        (*readable).SetEvictionPriority(DXGI_RESOURCE_PRIORITY_MAXIMUM);
        let readable = ComPtr(readable);

        let mut surface = ptr::null_mut();
        (*readable.0).QueryInterface(
            &IID_IDXGISurface,
            &mut surface as *mut *mut _ as *mut *mut _,
        );

        (*self.context.0).CopyResource(readable.0 as *mut _, texture.0 as *mut _);

        Ok(surface)
    }

    pub fn frame<'a>(&'a mut self, timeout: UINT) -> io::Result<Frame<'a>> {
        if self.output_texture {
            Ok(Frame::Texture(self.get_texture(timeout)?))
        } else {
            let width = self.width;
            let height = self.height;
            Ok(Frame::PixelBuffer(PixelBuffer::with_BGRA(
                self.get_pixelbuffer(timeout)?,
                width,
                height,
            )))
        }
    }

    pub(super) fn get_pixelbuffer<'a>(&'a mut self, timeout: UINT) -> io::Result<&'a [u8]> {
        unsafe {
            // Release last frame.
            // No error checking needed because we don't care.
            // None of the errors crash anyway.
            let result = {
                if let Some(gdi_capturer) = &self.gdi_capturer {
                    match gdi_capturer.frame(&mut self.gdi_buffer) {
                        Ok(_) => {
                            crate::would_block_if_equal(
                                &mut self.saved_raw_data,
                                &self.gdi_buffer,
                            )?;
                            &self.gdi_buffer
                        }
                        Err(err) => {
                            return Err(io::Error::new(io::ErrorKind::Other, err.to_string()));
                        }
                    }
                } else {
                    self.unmap();
                    let r = self.load_frame(timeout)?;
                    let rotate = match self.display.rotation() {
                        DXGI_MODE_ROTATION_IDENTITY | DXGI_MODE_ROTATION_UNSPECIFIED => kRotate0,
                        DXGI_MODE_ROTATION_ROTATE90 => kRotate90,
                        DXGI_MODE_ROTATION_ROTATE180 => kRotate180,
                        DXGI_MODE_ROTATION_ROTATE270 => kRotate270,
                        _ => {
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                "Unknown rotation".to_string(),
                            ));
                        }
                    };
                    if rotate == kRotate0 {
                        slice::from_raw_parts(r.0, r.1 as usize * self.height)
                    } else {
                        self.rotated.resize(self.width * self.height * 4, 0);
                        crate::common::ARGBRotate(
                            r.0,
                            r.1,
                            self.rotated.as_mut_ptr(),
                            4 * self.width as i32,
                            if rotate == kRotate180 {
                                self.width
                            } else {
                                self.height
                            } as _,
                            if rotate != kRotate180 {
                                self.width
                            } else {
                                self.height
                            } as _,
                            rotate,
                        );
                        &self.rotated[..]
                    }
                }
            };
            Ok(result)
        }
    }

    pub(super) fn get_texture(&mut self, timeout: UINT) -> io::Result<(*mut c_void, usize)> {
        unsafe {
            if self.duplication.0.is_null() {
                return Err(std::io::ErrorKind::AddrNotAvailable.into());
            }
            (*self.duplication.0).ReleaseFrame();
            let mut frame = ptr::null_mut();
            #[allow(invalid_value)]
            let mut info = mem::MaybeUninit::uninit().assume_init();

            wrap_hresult((*self.duplication.0).AcquireNextFrame(timeout, &mut info, &mut frame))?;
            let frame = ComPtr(frame);

            if info.AccumulatedFrames == 0 || *info.LastPresentTime.QuadPart() == 0 {
                return Err(std::io::ErrorKind::WouldBlock.into());
            }

            let mut texture: *mut ID3D11Texture2D = ptr::null_mut();
            (*frame.0).QueryInterface(
                &IID_ID3D11Texture2D,
                &mut texture as *mut *mut _ as *mut *mut _,
            );
            let texture = ComPtr(texture);
            self.texture = texture;

            let mut final_texture = self.texture.0 as *mut c_void;
            let mut rotation = match self.display.rotation() {
                DXGI_MODE_ROTATION_ROTATE90 => 90,
                DXGI_MODE_ROTATION_ROTATE180 => 180,
                DXGI_MODE_ROTATION_ROTATE270 => 270,
                _ => 0,
            };
            if rotation != 0
                && !self.texture.is_null()
                && !self.rotate.video_context.is_null()
                && !self.rotate.video_device.is_null()
                && !self.rotate.video_processor_enum.is_null()
                && !self.rotate.video_processor.is_null()
            {
                let mut desc: D3D11_TEXTURE2D_DESC = mem::zeroed();
                (*self.texture.0).GetDesc(&mut desc);
                if rotation == 90 || rotation == 270 {
                    let tmp = desc.Width;
                    desc.Width = desc.Height;
                    desc.Height = tmp;
                }
                if !self.rotate.texture.1 {
                    self.rotate.texture.1 = true;
                    let mut rotated_texture: *mut ID3D11Texture2D = ptr::null_mut();
                    desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED;
                    (*self.device.0).CreateTexture2D(&desc, ptr::null(), &mut rotated_texture);
                    self.rotate.texture.0 = ComPtr(rotated_texture);
                }
                if !self.rotate.texture.0.is_null()
                    && desc.Width == self.width as u32
                    && desc.Height == self.height as u32
                {
                    let input_view_desc = D3D11_VIDEO_PROCESSOR_INPUT_VIEW_DESC {
                        FourCC: 0,
                        ViewDimension: D3D11_VPIV_DIMENSION_TEXTURE2D,
                        Texture2D: D3D11_TEX2D_VPIV {
                            ArraySlice: 0,
                            MipSlice: 0,
                        },
                    };
                    let mut input_view = ptr::null_mut();
                    (*self.rotate.video_device.0).CreateVideoProcessorInputView(
                        self.texture.0 as *mut _,
                        self.rotate.video_processor_enum.0 as *mut _,
                        &input_view_desc,
                        &mut input_view,
                    );
                    if !input_view.is_null() {
                        let input_view = ComPtr(input_view);
                        let mut output_view_desc: D3D11_VIDEO_PROCESSOR_OUTPUT_VIEW_DESC =
                            mem::zeroed();
                        output_view_desc.ViewDimension = D3D11_VPOV_DIMENSION_TEXTURE2D;
                        output_view_desc.u.Texture2D_mut().MipSlice = 0;
                        let mut output_view = ptr::null_mut();
                        (*self.rotate.video_device.0).CreateVideoProcessorOutputView(
                            self.rotate.texture.0 .0 as *mut _,
                            self.rotate.video_processor_enum.0 as *mut _,
                            &output_view_desc,
                            &mut output_view,
                        );
                        if !output_view.is_null() {
                            let output_view = ComPtr(output_view);
                            let mut stream_data: D3D11_VIDEO_PROCESSOR_STREAM = mem::zeroed();
                            stream_data.Enable = TRUE;
                            stream_data.pInputSurface = input_view.0;
                            (*self.rotate.video_context.0).VideoProcessorBlt(
                                self.rotate.video_processor.0,
                                output_view.0,
                                0,
                                1,
                                &stream_data,
                            );
                            final_texture = self.rotate.texture.0 .0 as *mut c_void;
                            rotation = 0;
                        }
                    }
                }
            }
            Ok((final_texture, rotation))
        }
    }

    pub(super) fn unmap(&self) {
        unsafe {
            (*self.duplication.0).ReleaseFrame();
            if self.fastlane {
                (*self.duplication.0).UnMapDesktopSurface();
            } else {
                if !self.surface.is_null() {
                    (*self.surface.0).Unmap();
                }
            }
        }
    }

    pub fn device(&self) -> AdapterDevice {
        AdapterDevice {
            device: self.device.0 as _,
            vendor_id: self.adapter_desc1.VendorId,
            luid: ((self.adapter_desc1.AdapterLuid.HighPart as i64) << 32)
                | self.adapter_desc1.AdapterLuid.LowPart as i64,
        }
    }
}

impl Drop for Capturer {
    fn drop(&mut self) {
        if !self.duplication.is_null() {
            self.unmap();
        }
    }
}
