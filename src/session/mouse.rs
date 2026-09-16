use super::*;

impl<T: InvokeUiSession> Session<T> {
    pub fn send_touch_scale(&self, scale: i32, alt: bool, ctrl: bool, shift: bool, command: bool) {
        let scale_evt = TouchScaleUpdate {
            scale,
            ..Default::default()
        };
        let mut touch_evt = TouchEvent::new();
        touch_evt.set_scale_update(scale_evt);
        let mut evt = PointerDeviceEvent::new();
        evt.set_touch_event(touch_evt);
        send_pointer_device_event(evt, alt, ctrl, shift, command, self);
    }

    pub fn send_touch_pan_event(
        &self,
        event: &str,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        let mut touch_evt = TouchEvent::new();
        match event {
            "pan_start" => {
                touch_evt.set_pan_start(TouchPanStart {
                    x,
                    y,
                    ..Default::default()
                });
            }
            "pan_update" => {
                let (x, y) = self.get_scroll_xy((x, y));
                touch_evt.set_pan_update(TouchPanUpdate {
                    x,
                    y,
                    ..Default::default()
                });
            }
            "pan_end" => {
                touch_evt.set_pan_end(TouchPanEnd {
                    x,
                    y,
                    ..Default::default()
                });
            }
            _ => {
                log::warn!("unknown touch pan event: {}", event);
                return;
            }
        };
        let mut evt = PointerDeviceEvent::new();
        evt.set_touch_event(touch_evt);
        send_pointer_device_event(evt, alt, ctrl, shift, command, self);
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn is_scroll_reverse_mode(&self) -> bool {
        self.lc.read().unwrap().reverse_mouse_wheel.eq("Y")
    }

    #[inline]
    fn get_scroll_xy(&self, xy: (i32, i32)) -> (i32, i32) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.is_scroll_reverse_mode() {
            return (-xy.0, -xy.1);
        }
        xy
    }

    pub fn send_mouse(
        &self,
        mut mask: i32,
        x: i32,
        y: i32,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
    ) {
        #[allow(unused_mut)]
        let mut command = command;
        #[cfg(windows)]
        {
            if !command && crate::platform::windows::get_win_key_state() {
                command = true;
            }
        }

        // Compute event type once using MOUSE_TYPE_MASK for reuse
        let event_type = mask & MOUSE_TYPE_MASK;
        let (x, y) = if event_type == MOUSE_TYPE_WHEEL || event_type == MOUSE_TYPE_TRACKPAD {
            self.get_scroll_xy((x, y))
        } else {
            (x, y)
        };

        // #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let (alt, ctrl, shift, command) =
            keyboard::client::get_modifiers_state(alt, ctrl, shift, command);
        let is_left = (mask & (MOUSE_BUTTON_LEFT << 3)) > 0;
        let is_right = (mask & (MOUSE_BUTTON_RIGHT << 3)) > 0;
        if is_left ^ is_right {
            let swap_lr = self.get_toggle_option("swap-left-right-mouse".to_string());
            if swap_lr {
                if is_left {
                    mask = (mask & (!(MOUSE_BUTTON_LEFT << 3))) | (MOUSE_BUTTON_RIGHT << 3);
                } else {
                    mask = (mask & (!(MOUSE_BUTTON_RIGHT << 3))) | (MOUSE_BUTTON_LEFT << 3);
                }
            }
        }

        send_mouse(mask, x, y, alt, ctrl, shift, command, self);
        // on macos, ctrl + left button down = right button down, up won't emit, so we need to
        // emit up myself if peer is not macos
        // to-do: how about ctrl + left from win to macos
        if cfg!(target_os = "macos") {
            let buttons = mask >> 3;
            if buttons == MOUSE_BUTTON_LEFT
                && event_type == MOUSE_TYPE_DOWN
                && ctrl
                && self.peer_platform() != "Mac OS"
            {
                self.send_mouse(
                    (MOUSE_BUTTON_LEFT << 3 | MOUSE_TYPE_UP) as _,
                    x,
                    y,
                    alt,
                    ctrl,
                    shift,
                    command,
                );
            }
        }
    }
}
