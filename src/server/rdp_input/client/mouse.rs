use super::*;

lazy_static::lazy_static! {
    pub(super) static ref SHOULD_SCALE_POINTER_COORDINATES: bool =
        std::env::var(XDG_CURRENT_DESKTOP)
            .map(|desktop| desktop == DISPLAY_DESKTOP_KDE || desktop_is_niri(&desktop))
            .unwrap_or(false);
}

pub struct RdpInputMouse {
    pub(super) conn: Arc<SyncConnection>,
    pub(super) session: Path<'static>,
    pub(super) stream: PwStreamInfo,
    pub(super) resolution: (usize, usize),
    pub(super) scale: Option<f64>,
    pub(super) position: (f64, f64),
}

impl RdpInputMouse {
    pub fn new(
        conn: Arc<SyncConnection>,
        session: Path<'static>,
        stream: PwStreamInfo,
        resolution: (usize, usize),
    ) -> ResultType<Self> {
        // https://github.com/rustdesk/rustdesk/pull/9019#issuecomment-2295252388
        // There may be a bug in Rdp input on Gnome util Ubuntu 24.04 (Gnome 46)
        //
        // eg. Resolution 800x600, Fractional scale: 200% (logic size: 400x300)
        // https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.impl.portal.RemoteDesktop.html#:~:text=new%20pointer%20position-,in%20the%20streams%20logical%20coordinate%20space,-.
        // Then (x,y) in `mouse_move_to()` and `mouse_move_relative()` should be scaled to the logic size(stream.get_size()), which is from (0,0) to (400,300).
        // For Ubuntu 24.04(Gnome 46), (x,y) is restricted from (0,0) to (400,300), but the actual range in screen is:
        // Logic coordinate from (0,0) to (200x150).
        // Or physical coordinate from (0,0) to (400,300).
        let scale = if *SHOULD_SCALE_POINTER_COORDINATES {
            if resolution.0 == 0 || stream.get_size().0 == 0 {
                Some(1.0f64)
            } else {
                Some(resolution.0 as f64 / stream.get_size().0 as f64)
            }
        } else {
            None
        };
        let pos = stream.get_position();
        Ok(Self {
            conn,
            session,
            stream,
            resolution,
            scale,
            position: (pos.0 as f64, pos.1 as f64),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::desktop_is_niri;

    #[test]
    fn detects_niri_in_desktop_list() {
        assert!(desktop_is_niri("niri"));
        assert!(desktop_is_niri("NIRI"));
        assert!(desktop_is_niri("GNOME:niri"));
        assert!(!desktop_is_niri("GNOME"));
    }
}

impl MouseControllable for RdpInputMouse {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mouse_move_to(&mut self, x: i32, y: i32) {
        let x = if let Some(s) = self.scale {
            x as f64 / s
        } else {
            x as f64
        };
        let y = if let Some(s) = self.scale {
            y as f64 / s
        } else {
            y as f64
        };
        let x = x - self.position.0;
        let y = y - self.position.1;
        let portal = get_portal(&self.conn);
        let _ = remote_desktop_portal::notify_pointer_motion_absolute(
            &portal,
            &self.session,
            HashMap::new(),
            self.stream.path as u32,
            x,
            y,
        );
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        let x = if let Some(s) = self.scale {
            x as f64 / s
        } else {
            x as f64
        };
        let y = if let Some(s) = self.scale {
            y as f64 / s
        } else {
            y as f64
        };
        let portal = get_portal(&self.conn);
        let _ = remote_desktop_portal::notify_pointer_motion(
            &portal,
            &self.session,
            HashMap::new(),
            x,
            y,
        );
    }
    fn mouse_down(&mut self, button: MouseButton) -> enigo::ResultType {
        handle_mouse(true, button, self.conn.clone(), &self.session);
        Ok(())
    }
    fn mouse_up(&mut self, button: MouseButton) {
        handle_mouse(false, button, self.conn.clone(), &self.session);
    }
    fn mouse_click(&mut self, button: MouseButton) {
        handle_mouse(true, button, self.conn.clone(), &self.session);
        handle_mouse(false, button, self.conn.clone(), &self.session);
    }
    fn mouse_scroll_x(&mut self, length: i32) {
        let portal = get_portal(&self.conn);
        let _ = remote_desktop_portal::notify_pointer_axis(
            &portal,
            &self.session,
            HashMap::new(),
            length as f64,
            0 as f64,
        );
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        let portal = get_portal(&self.conn);
        let _ = remote_desktop_portal::notify_pointer_axis(
            &portal,
            &self.session,
            HashMap::new(),
            0 as f64,
            length as f64,
        );
    }
}
