use super::*;

impl Default for VideoRenderer {
    fn default() -> Self {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let on_rgba_func = match &*TEXTURE_RGBA_RENDERER_PLUGIN {
            Ok(lib) => {
                let find_sym_res = unsafe {
                    lib.symbol::<FlutterRgbaRendererPluginOnRgba>("FlutterRgbaRendererPluginOnRgba")
                };
                match find_sym_res {
                    Ok(sym) => Some(sym),
                    Err(e) => {
                        log::error!("Failed to find symbol FlutterRgbaRendererPluginOnRgba, {e}");
                        None
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load texture rgba renderer plugin, {e}");
                None
            }
        };
        #[cfg(feature = "vram")]
        let on_texture_func = match &*TEXTURE_GPU_RENDERER_PLUGIN {
            Ok(lib) => {
                let find_sym_res = unsafe {
                    lib.symbol::<FlutterGpuTextureRendererPluginCApiSetTexture>(
                        "FlutterGpuTextureRendererPluginCApiSetTexture",
                    )
                };
                match find_sym_res {
                    Ok(sym) => Some(sym),
                    Err(e) => {
                        log::error!("Failed to find symbol FlutterGpuTextureRendererPluginCApiSetTexture, {e}");
                        None
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to load texture gpu renderer plugin, {e}");
                None
            }
        };

        Self {
            map_display_sessions: Default::default(),
            is_support_multi_ui_session: false,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            on_rgba_func,
            #[cfg(feature = "vram")]
            on_texture_func,
        }
    }
}

impl VideoRenderer {
    #[inline]
    pub(super) fn set_size(&mut self, display: usize, width: usize, height: usize) {
        let mut sessions_lock = self.map_display_sessions.write().unwrap();
        if let Some(info) = sessions_lock.get_mut(&display) {
            info.size = (width, height);
            info.notify_render_type = None;
        } else {
            sessions_lock.insert(
                display,
                DisplaySessionInfo {
                    texture_rgba_ptr: usize::default(),
                    size: (width, height),
                    #[cfg(feature = "vram")]
                    gpu_output_ptr: usize::default(),
                    notify_render_type: None,
                },
            );
        }
    }

    pub(super) fn register_pixelbuffer_texture(&self, display: usize, ptr: usize) {
        let mut sessions_lock = self.map_display_sessions.write().unwrap();
        if ptr == 0 {
            if let Some(info) = sessions_lock.get_mut(&display) {
                if info.texture_rgba_ptr != usize::default() {
                    info.texture_rgba_ptr = usize::default();
                }
                #[cfg(feature = "vram")]
                if info.gpu_output_ptr != usize::default() {
                    return;
                }
            }
            sessions_lock.remove(&display);
        } else {
            if let Some(info) = sessions_lock.get_mut(&display) {
                if info.texture_rgba_ptr != usize::default()
                    && info.texture_rgba_ptr != ptr as TextureRgbaPtr
                {
                    log::warn!(
                        "texture_rgba_ptr is not null and not equal to ptr, replace {} to {}",
                        info.texture_rgba_ptr,
                        ptr
                    );
                }
                info.texture_rgba_ptr = ptr as _;
                info.notify_render_type = None;
            } else {
                if ptr != 0 {
                    sessions_lock.insert(
                        display,
                        DisplaySessionInfo {
                            texture_rgba_ptr: ptr as _,
                            size: (0, 0),
                            #[cfg(feature = "vram")]
                            gpu_output_ptr: usize::default(),
                            notify_render_type: None,
                        },
                    );
                }
            }
        }
    }

    #[cfg(feature = "vram")]
    pub fn register_gpu_output(&self, display: usize, ptr: usize) {
        let mut sessions_lock = self.map_display_sessions.write().unwrap();
        if ptr == 0 {
            if let Some(info) = sessions_lock.get_mut(&display) {
                if info.gpu_output_ptr != usize::default() {
                    info.gpu_output_ptr = usize::default();
                }
                if info.texture_rgba_ptr != usize::default() {
                    return;
                }
            }
            sessions_lock.remove(&display);
        } else {
            if let Some(info) = sessions_lock.get_mut(&display) {
                if info.gpu_output_ptr != usize::default() && info.gpu_output_ptr != ptr {
                    log::error!(
                        "gpu_output_ptr is not null and not equal to ptr, relace {} to {}",
                        info.gpu_output_ptr,
                        ptr
                    );
                }
                info.gpu_output_ptr = ptr as _;
                info.notify_render_type = None;
            } else {
                if ptr != usize::default() {
                    sessions_lock.insert(
                        display,
                        DisplaySessionInfo {
                            texture_rgba_ptr: usize::default(),
                            size: (0, 0),
                            gpu_output_ptr: ptr,
                            notify_render_type: None,
                        },
                    );
                }
            }
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn on_rgba(&self, display: usize, rgba: &scrap::ImageRgb) -> bool {
        let mut write_lock = self.map_display_sessions.write().unwrap();
        let opt_info = if !self.is_support_multi_ui_session {
            write_lock.values_mut().next()
        } else {
            write_lock.get_mut(&display)
        };
        let Some(info) = opt_info else {
            return false;
        };
        if info.texture_rgba_ptr == usize::default() {
            return false;
        }

        if info.size.0 != rgba.w || info.size.1 != rgba.h {
            log::error!(
                "width/height mismatch: ({},{}) != ({},{})",
                info.size.0,
                info.size.1,
                rgba.w,
                rgba.h
            );
            // Peer info's handling is async and may be late than video frame's handling
            // Allow peer info not set, but not allow wrong width/height for correct local cursor position
            if info.size != (0, 0) {
                return false;
            }
        }
        if let Some(func) = &self.on_rgba_func {
            unsafe {
                func(
                    info.texture_rgba_ptr as _,
                    rgba.raw.as_ptr() as _,
                    rgba.raw.len() as _,
                    rgba.w as _,
                    rgba.h as _,
                    rgba.align() as _,
                )
            };
        }
        if info.notify_render_type != Some(RenderType::PixelBuffer) {
            info.notify_render_type = Some(RenderType::PixelBuffer);
            true
        } else {
            false
        }
    }

    #[cfg(feature = "vram")]
    pub fn on_texture(&self, display: usize, texture: *mut c_void) -> bool {
        let mut write_lock = self.map_display_sessions.write().unwrap();
        let opt_info = if !self.is_support_multi_ui_session {
            write_lock.values_mut().next()
        } else {
            write_lock.get_mut(&display)
        };
        let Some(info) = opt_info else {
            return false;
        };
        if info.gpu_output_ptr == usize::default() {
            return false;
        }
        if let Some(func) = &self.on_texture_func {
            unsafe { func(info.gpu_output_ptr as _, texture) };
        }
        if info.notify_render_type != Some(RenderType::Texture) {
            info.notify_render_type = Some(RenderType::Texture);
            true
        } else {
            false
        }
    }

    pub fn reset_all_display_render_type(&self) {
        let mut write_lock = self.map_display_sessions.write().unwrap();
        write_lock
            .values_mut()
            .map(|v| v.notify_render_type = None)
            .count();
    }
}
