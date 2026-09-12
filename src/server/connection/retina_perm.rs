use super::*;

#[cfg(target_os = "macos")]
#[derive(Debug, Default)]
pub(super) struct Retina {
    pub(super) displays: Vec<DisplayInfo>,
}

#[cfg(target_os = "macos")]
impl Retina {
    #[inline]
    pub(super) fn set_displays(&mut self, displays: &Vec<DisplayInfo>) {
        self.displays = displays.clone();
    }

    #[inline]
    pub(super) fn on_mouse_event(&mut self, e: &mut MouseEvent, current: usize) {
        let evt_type = e.mask & crate::input::MOUSE_TYPE_MASK;
        // Delta-based events do not contain absolute coordinates.
        // Avoid applying Retina coordinate scaling to them.
        if evt_type == crate::input::MOUSE_TYPE_WHEEL
            || evt_type == crate::input::MOUSE_TYPE_TRACKPAD
            || evt_type == crate::input::MOUSE_TYPE_MOVE_RELATIVE
        {
            return;
        }
        let Some(d) = self.displays.get(current) else {
            return;
        };
        let s = d.scale;
        if s > 1.0 && e.x >= d.x && e.y >= d.y && e.x < d.x + d.width && e.y < d.y + d.height {
            e.x = d.x + ((e.x - d.x) as f64 / s) as i32;
            e.y = d.y + ((e.y - d.y) as f64 / s) as i32;
        }
    }

    #[inline]
    pub(super) fn on_cursor_pos(&mut self, pos: &CursorPosition, current: usize) -> Option<Message> {
        let Some(d) = self.displays.get(current) else {
            return None;
        };
        let s = d.scale;
        if s > 1.0
            && pos.x >= d.x
            && pos.y >= d.y
            && (pos.x - d.x) as f64 * s < d.width as f64
            && (pos.y - d.y) as f64 * s < d.height as f64
        {
            let mut pos = pos.clone();
            pos.x = d.x + ((pos.x - d.x) as f64 * s) as i32;
            pos.y = d.y + ((pos.y - d.y) as f64 * s) as i32;
            let mut msg = Message::new();
            msg.set_cursor_position(pos);
            return Some(msg);
        }
        None
    }
}

/// Get control permission state from CONTROL_PERMISSIONS_ARRAY.
/// Returns: Some(false) if any disable, Some(true) if any enable (and no disable), None if not set.
pub fn get_control_permission_state(
    permission: hbb_common::rendezvous_proto::control_permissions::Permission,
    disable_if_has_disabled: bool,
) -> Option<bool> {
    let control_permissions = CONTROL_PERMISSIONS_ARRAY.lock().unwrap();
    let mut has_enable = false;
    let mut has_disable = false;
    for (_, cp) in control_permissions.iter() {
        match crate::get_control_permission(cp.permissions, permission) {
            Some(false) => has_disable = true,
            Some(true) => has_enable = true,
            None => {}
        }
    }
    if disable_if_has_disabled {
        if has_disable {
            Some(false)
        } else if has_enable {
            Some(true)
        } else {
            None
        }
    } else {
        if has_enable {
            Some(true)
        } else if has_disable {
            Some(false)
        } else {
            None
        }
    }
}

pub struct AuthedConn {
    pub conn_id: i32,
    pub conn_type: AuthConnType,
    pub session_key: SessionKey,
    pub sender: mpsc::UnboundedSender<Data>,
}
