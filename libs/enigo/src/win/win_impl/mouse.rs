use super::*;

impl MouseControllable for Enigo {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mouse_move_to(&mut self, x: i32, y: i32) {
        mouse_event(
            MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
            0,
            (x - unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) }) * 65535
                / unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) },
            (y - unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) }) * 65535
                / unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) },
        );
    }

    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        mouse_event(MOUSEEVENTF_MOVE, 0, x, y);
    }

    fn mouse_down(&mut self, button: MouseButton) -> crate::ResultType {
        let res = mouse_event(
            match button {
                MouseButton::Left => MOUSEEVENTF_LEFTDOWN,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEDOWN,
                MouseButton::Right => MOUSEEVENTF_RIGHTDOWN,
                MouseButton::Back => MOUSEEVENTF_XDOWN,
                MouseButton::Forward => MOUSEEVENTF_XDOWN,
                _ => {
                    log::info!("Unsupported button {:?}", button);
                    return Ok(());
                }
            },
            match button {
                MouseButton::Back => XBUTTON1 as u32,
                MouseButton::Forward => XBUTTON2 as u32,
                _ => 0,
            },
            0,
            0,
        );
        if res == 0 {
            let err = get_error();
            if !err.is_empty() {
                return Err(err.into());
            }
        }
        Ok(())
    }

    fn mouse_up(&mut self, button: MouseButton) {
        mouse_event(
            match button {
                MouseButton::Left => MOUSEEVENTF_LEFTUP,
                MouseButton::Middle => MOUSEEVENTF_MIDDLEUP,
                MouseButton::Right => MOUSEEVENTF_RIGHTUP,
                MouseButton::Back => MOUSEEVENTF_XUP,
                MouseButton::Forward => MOUSEEVENTF_XUP,
                _ => {
                    log::info!("Unsupported button {:?}", button);
                    return;
                }
            },
            match button {
                MouseButton::Back => XBUTTON1 as _,
                MouseButton::Forward => XBUTTON2 as _,
                _ => 0,
            },
            0,
            0,
        );
    }

    fn mouse_click(&mut self, button: MouseButton) {
        self.mouse_down(button).ok();
        self.mouse_up(button);
    }

    fn mouse_scroll_x(&mut self, length: i32) {
        mouse_event(MOUSEEVENTF_HWHEEL, length as _, 0, 0);
    }

    fn mouse_scroll_y(&mut self, length: i32) {
        mouse_event(MOUSEEVENTF_WHEEL, length as _, 0, 0);
    }
}
