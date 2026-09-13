use super::*;

impl Connection {
    pub(super) fn handle_mouse_event(&mut self, #[allow(unused_mut)] mut me: MouseEvent) {
        if self.is_authed_view_camera_conn() {
            return;
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if let Err(e) = call_main_service_pointer_input("mouse", me.mask, me.x, me.y) {
            log::debug!("call_main_service_pointer_input fail:{}", e);
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.peer_keyboard_enabled() {
            if is_left_up(&me) {
                CLICK_TIME.store(get_time(), Ordering::SeqCst);
            } else {
                MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);
            }
            #[cfg(target_os = "macos")]
            self.retina.on_mouse_event(&mut me, self.display_idx);
            self.input_mouse(
                me,
                self.inner.id(),
                self.lr.my_name.clone(),
                self.peer_argb,
                true,
                self.show_my_cursor,
            );
        } else if self.show_my_cursor {
            #[cfg(target_os = "macos")]
            self.retina.on_mouse_event(&mut me, self.display_idx);
            self.input_mouse(
                me,
                self.inner.id(),
                self.lr.my_name.clone(),
                self.peer_argb,
                false,
                true,
            );
        }
        self.update_auto_disconnect_timer();
    }

    pub(super) fn handle_pointer_device_event(&mut self, pde: PointerDeviceEvent) {
        if self.is_authed_view_camera_conn() {
            return;
        }
        #[cfg(any(target_os = "android", target_os = "ios"))]
        if let Err(e) = match pde.union {
            Some(pointer_device_event::Union::TouchEvent(touch)) => match touch.union {
                Some(touch_event::Union::PanStart(pan_start)) => {
                    call_main_service_pointer_input(
                        "touch",
                        4,
                        pan_start.x,
                        pan_start.y,
                    )
                }
                Some(touch_event::Union::PanUpdate(pan_update)) => {
                    call_main_service_pointer_input(
                        "touch",
                        5,
                        pan_update.x,
                        pan_update.y,
                    )
                }
                Some(touch_event::Union::PanEnd(pan_end)) => {
                    call_main_service_pointer_input("touch", 6, pan_end.x, pan_end.y)
                }
                _ => Ok(()),
            },
            _ => Ok(()),
        } {
            log::debug!("call_main_service_pointer_input fail:{}", e);
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.peer_keyboard_enabled() {
            MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);
            self.input_pointer(pde, self.inner.id());
        }
        self.update_auto_disconnect_timer();
    }

    #[cfg(any(target_os = "android"))]
    pub(super) fn handle_key_event(&mut self, #[allow(unused_mut)] mut me: KeyEvent) {
        if self.is_authed_view_camera_conn() {
            return;
        }
        let key = match me.mode.enum_value() {
            Ok(KeyboardMode::Map) => {
                Some(crate::keyboard::keycode_to_rdev_key(me.chr()))
            }
            Ok(KeyboardMode::Translate) => {
                if let Some(key_event::Union::Chr(code)) = me.union {
                    Some(crate::keyboard::keycode_to_rdev_key(code & 0x0000FFFF))
                } else {
                    None
                }
            }
            _ => None,
        }
        .filter(crate::keyboard::is_modifier);

        let is_press =
            (me.press || me.down) && !(crate::is_modifier(&me) || key.is_some());

        if let Some(key) = key {
            if is_press {
                self.pressed_modifiers.insert(key);
            } else {
                self.pressed_modifiers.remove(&key);
            }
        }

        let mut modifiers = vec![];

        for key in self.pressed_modifiers.iter() {
            if let Some(control_key) = map_key_to_control_key(key) {
                modifiers.push(EnumOrUnknown::new(control_key));
            }
        }

        me.modifiers = modifiers;

        let encode_result = me.write_to_bytes();

        match encode_result {
            Ok(data) => {
                let result = call_main_service_key_event(&data);
                if let Err(e) = result {
                    log::debug!("call_main_service_key_event fail: {}", e);
                }
            }
            Err(e) => {
                log::debug!("encode key event fail: {}", e);
            }
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn handle_key_event(&mut self, me: KeyEvent) {
        if self.is_authed_view_camera_conn() {
            return;
        }
        if self.peer_keyboard_enabled() {
            if is_enter(&me) {
                CLICK_TIME.store(get_time(), Ordering::SeqCst);
            }
            // https://github.com/rustdesk/rustdesk/issues/8633
            MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);

            let key = match me.mode.enum_value() {
                Ok(KeyboardMode::Map) => {
                    Some(crate::keyboard::keycode_to_rdev_key(me.chr()))
                }
                Ok(KeyboardMode::Translate) => {
                    if let Some(key_event::Union::Chr(code)) = me.union {
                        Some(crate::keyboard::keycode_to_rdev_key(code & 0x0000FFFF))
                    } else {
                        None
                    }
                }
                _ => None,
            }
            .filter(crate::keyboard::is_modifier);

            // handle all down as press
            // fix unexpected repeating key on remote linux, seems also fix abnormal alt/shift, which
            // make sure all key are released
            // https://github.com/rustdesk/rustdesk/issues/6793
            let is_press = if cfg!(target_os = "linux") {
                (me.press || me.down) && !(crate::is_modifier(&me) || key.is_some())
            } else {
                me.press
            };

            if let Some(key) = key {
                if is_press {
                    self.pressed_modifiers.insert(key);
                } else {
                    self.pressed_modifiers.remove(&key);
                }
            }

            if is_press {
                match me.union {
                    Some(key_event::Union::Unicode(_))
                    | Some(key_event::Union::Seq(_)) => {
                        self.input_key(me, false);
                    }
                    _ => {
                        self.input_key(me, true);
                    }
                }
            } else {
                self.input_key(me, false);
            }
        }
        self.update_auto_disconnect_timer();
    }
}
