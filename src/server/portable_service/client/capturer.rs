use super::*;

pub struct CapturerPortable {
    pub(super) width: usize,
    pub(super) height: usize,
}

impl CapturerPortable {
    pub fn new(current_display: usize) -> Self
    where
        Self: Sized,
    {
        let mut option = SHMEM.lock().unwrap();
        if let Some(shmem) = option.as_mut() {
            unsafe {
                libc::memset(
                    shmem.as_ptr().add(ADDR_CURSOR_PARA) as _,
                    0,
                    shmem.len().saturating_sub(ADDR_CURSOR_PARA) as _,
                );
            }
            utils::set_para(
                shmem,
                CapturerPara {
                    recreate: true,
                    current_display,
                    timeout_ms: 33,
                },
            );
            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
        }
        let (mut width, mut height) = (0, 0);
        if let Ok(displays) = display_service::try_get_displays() {
            if let Some(display) = displays.get(current_display) {
                width = display.width();
                height = display.height();
            }
        }
        CapturerPortable { width, height }
    }
}

impl TraitCapturer for CapturerPortable {
    fn frame<'a>(&'a mut self, timeout: Duration) -> std::io::Result<Frame<'a>> {
        let mut lock = SHMEM.lock().unwrap();
        let shmem = lock.as_mut().ok_or(std::io::Error::new(
            std::io::ErrorKind::Other,
            "shmem dropped".to_string(),
        ))?;
        unsafe {
            let base = shmem.as_ptr();
            let para_ptr = base.add(ADDR_CAPTURER_PARA);
            let para = para_ptr as *const CapturerPara;
            if timeout.as_millis() != (*para).timeout_ms as _ {
                utils::set_para(
                    shmem,
                    CapturerPara {
                        recreate: (*para).recreate,
                        current_display: (*para).current_display,
                        timeout_ms: timeout.as_millis() as _,
                    },
                );
            }
            if utils::counter_ready(base.add(ADDR_CAPTURE_FRAME_COUNTER)) {
                let frame_info_ptr = shmem.as_ptr().add(ADDR_CAPTURE_FRAME_INFO);
                let frame_info = frame_info_ptr as *const FrameInfo;
                let frame_len = (*frame_info).length;
                if !is_valid_capture_frame_length(shmem.len(), frame_len) {
                    log::error!(
                        "Portable service frame length exceeds shared memory capacity: frame_len={}, shmem_len={}, frame_addr={}",
                        frame_len,
                        shmem.len(),
                        ADDR_CAPTURE_FRAME
                    );
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "invalid portable service frame length".to_string(),
                    ));
                }
                if (*frame_info).width != self.width || (*frame_info).height != self.height {
                    log::info!(
                        "skip frame, ({},{}) != ({},{})",
                        (*frame_info).width,
                        (*frame_info).height,
                        self.width,
                        self.height,
                    );
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "wouldblock error".to_string(),
                    ));
                }
                let frame_ptr = base.add(ADDR_CAPTURE_FRAME);
                let data = slice::from_raw_parts(frame_ptr, frame_len);
                Ok(Frame::PixelBuffer(PixelBuffer::with_BGRA(
                    data,
                    self.width,
                    self.height,
                )))
            } else {
                let ptr = base.add(ADDR_CAPTURE_WOULDBLOCK);
                let wouldblock = utils::ptr_to_i32(ptr);
                if wouldblock == TRUE {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::WouldBlock,
                        "wouldblock error".to_string(),
                    ))
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "other error".to_string(),
                    ))
                }
            }
        }
    }

    // control by itself
    fn is_gdi(&self) -> bool {
        true
    }

    fn set_gdi(&mut self) -> bool {
        true
    }

    #[cfg(feature = "vram")]
    fn device(&self) -> AdapterDevice {
        AdapterDevice::default()
    }

    #[cfg(feature = "vram")]
    fn set_output_texture(&mut self, _texture: bool) {}
}
