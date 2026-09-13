use super::input_service::set_clipboard_for_paste_sync;
use crate::uinput::service::{can_input_via_keysym, char_to_keysym, map_key};
use dbus::{blocking::SyncConnection, Path};
use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use hbb_common::{log, ResultType};
use scrap::wayland::pipewire::{get_portal, PwStreamInfo};
use scrap::wayland::remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop as remote_desktop_portal;
use std::collections::HashMap;
use std::sync::Arc;

pub mod client {
    use base::platform::linux::{DISPLAY_DESKTOP_KDE, XDG_CURRENT_DESKTOP};

    use super::*;

    mod keyboard;
    pub use keyboard::*;
    mod clipboard_text;
    use clipboard_text::*;

    const EVDEV_MOUSE_LEFT: i32 = 272;
    const EVDEV_MOUSE_RIGHT: i32 = 273;
    const EVDEV_MOUSE_MIDDLE: i32 = 274;

    const PRESSED_DOWN_STATE: u32 = 1;
    const PRESSED_UP_STATE: u32 = 0;

    lazy_static::lazy_static! {
        static ref SHOULD_SCALE_POINTER_COORDINATES: bool =
            std::env::var(XDG_CURRENT_DESKTOP)
                .map(|desktop| desktop == DISPLAY_DESKTOP_KDE || desktop_is_niri(&desktop))
                .unwrap_or(false);
    }

    pub struct RdpInputMouse {
        conn: Arc<SyncConnection>,
        session: Path<'static>,
        stream: PwStreamInfo,
        resolution: (usize, usize),
        scale: Option<f64>,
        position: (f64, f64),
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
        use super::desktop_is_niri;

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

    /// Send a keysym via RemoteDesktop portal.
    fn send_keysym(
        keysym: i32,
        down: bool,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
    ) -> ResultType<()> {
        let state: u32 = if down {
            PRESSED_DOWN_STATE
        } else {
            PRESSED_UP_STATE
        };
        let portal = get_portal(&conn);
        log::trace!(
            "send_keysym: calling notify_keyboard_keysym, keysym={:#x}, state={}",
            keysym,
            state
        );
        match remote_desktop_portal::notify_keyboard_keysym(
            &portal,
            session,
            HashMap::new(),
            keysym,
            state,
        ) {
            Ok(_) => {
                log::trace!("send_keysym: notify_keyboard_keysym succeeded");
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }

    fn get_raw_evdev_keycode(key: u16) -> i32 {
        // 8 is the offset between xkb and evdev
        let mut key = key as i32 - 8;
        // fix for right_meta key
        if key == 126 {
            key = 125;
        }
        key
    }

    fn handle_key(
        down: bool,
        key: Key,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
    ) -> ResultType<()> {
        let state: u32 = if down {
            PRESSED_DOWN_STATE
        } else {
            PRESSED_UP_STATE
        };
        let portal = get_portal(&conn);
        match key {
            Key::Raw(key) => {
                let key = get_raw_evdev_keycode(key);
                remote_desktop_portal::notify_keyboard_keycode(
                    &portal,
                    &session,
                    HashMap::new(),
                    key,
                    state,
                )?;
            }
            _ => {
                if let Ok((key, is_shift)) = map_key(&key) {
                    let shift_keycode = evdev::Key::KEY_LEFTSHIFT.code() as i32;
                    if down {
                        // Press: Shift down first, then key down
                        if is_shift {
                            if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                                &portal,
                                &session,
                                HashMap::new(),
                                shift_keycode,
                                state,
                            ) {
                                log::error!("handle_key: failed to press Shift: {:?}", e);
                                return Err(e.into());
                            }
                        }
                        if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                            &portal,
                            &session,
                            HashMap::new(),
                            key.code() as i32,
                            state,
                        ) {
                            log::error!("handle_key: failed to press key: {:?}", e);
                            // Best-effort: release Shift if it was pressed
                            if is_shift {
                                if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                                    &portal,
                                    &session,
                                    HashMap::new(),
                                    shift_keycode,
                                    PRESSED_UP_STATE,
                                ) {
                                    log::warn!(
                                        "handle_key: best-effort Shift release also failed: {:?}",
                                        e
                                    );
                                }
                            }
                            return Err(e.into());
                        }
                    } else {
                        // Release: key up first, then Shift up
                        if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                            &portal,
                            &session,
                            HashMap::new(),
                            key.code() as i32,
                            PRESSED_UP_STATE,
                        ) {
                            log::error!("handle_key: failed to release key: {:?}", e);
                            // Best-effort: still try to release Shift
                            if is_shift {
                                if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                                    &portal,
                                    &session,
                                    HashMap::new(),
                                    shift_keycode,
                                    PRESSED_UP_STATE,
                                ) {
                                    log::warn!(
                                        "handle_key: best-effort Shift release also failed: {:?}",
                                        e
                                    );
                                }
                            }
                            return Err(e.into());
                        }
                        if is_shift {
                            if let Err(e) = remote_desktop_portal::notify_keyboard_keycode(
                                &portal,
                                &session,
                                HashMap::new(),
                                shift_keycode,
                                PRESSED_UP_STATE,
                            ) {
                                log::error!("handle_key: failed to release Shift: {:?}", e);
                                return Err(e.into());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_mouse(
        down: bool,
        button: MouseButton,
        conn: Arc<SyncConnection>,
        session: &Path<'static>,
    ) {
        let portal = get_portal(&conn);
        let but_key = match button {
            MouseButton::Left => EVDEV_MOUSE_LEFT,
            MouseButton::Right => EVDEV_MOUSE_RIGHT,
            MouseButton::Middle => EVDEV_MOUSE_MIDDLE,
            _ => {
                return;
            }
        };
        let state: u32 = if down {
            PRESSED_DOWN_STATE
        } else {
            PRESSED_UP_STATE
        };
        let _ = remote_desktop_portal::notify_pointer_button(
            &portal,
            &session,
            HashMap::new(),
            but_key,
            state,
        );
    }
}
