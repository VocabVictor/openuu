use super::*;

pub(super) fn check_remove_unused_displays(
    current: Option<usize>,
    session_id: &SessionID,
    session: &FlutterSession,
    handlers: &HashMap<SessionID, SessionHandler>,
) {
    // Set capture displays if some are not used any more.
    let mut remains_displays = HashSet::new();
    if let Some(current) = current {
        remains_displays.insert(current);
    }
    for (k, h) in handlers.iter() {
        if k == session_id {
            continue;
        }
        remains_displays.extend(
            h.renderer
                .map_display_sessions
                .read()
                .unwrap()
                .keys()
                .cloned(),
        );
    }
    if !remains_displays.is_empty() {
        session.capture_displays(
            vec![],
            vec![],
            remains_displays.iter().map(|d| *d as i32).collect(),
        );
    }
}

pub fn session_switch_display(is_desktop: bool, session_id: SessionID, value: Vec<i32>) {
    for s in SESSIONS.read().unwrap().values() {
        let mut write_lock = s.ui_handler.session_handlers.write().unwrap();
        if let Some(h) = write_lock.get_mut(&session_id) {
            h.displays = value.iter().map(|x| *x as usize).collect::<_>();
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let displays_refresh = value.clone();
            if value.len() == 1 {
                // Switch display.
                // This operation will also cause the peer to send a switch display message.
                // The switch display message will contain `SupportedResolutions`, which is useful when changing resolutions.
                s.switch_display(value[0]);
                // Reset the valid flag of the display.
                s.next_rgba(value[0] as usize);

                if !is_desktop {
                    s.capture_displays(vec![], vec![], value);
                } else {
                    // Check if other displays are needed.
                    if value.len() == 1 {
                        check_remove_unused_displays(
                            Some(value[0] as _),
                            &session_id,
                            &s,
                            &write_lock,
                        );
                    }
                }
            } else {
                // Try capture all displays.
                s.capture_displays(vec![], vec![], value);
            }
            // When switching display, we also need to send "Refresh display" message.
            // On the controlled side:
            // 1. If this display is not currently captured -> Refresh -> Message "Refresh display" is not required.
            // One more key frame (first frame) will be sent because the refresh message.
            // 2. If this display is currently captured -> Not refresh -> Message "Refresh display" is required.
            // Without the message, the control side cannot see the latest display image.
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                let is_support_multi_ui_session = crate::common::is_support_multi_ui_session(
                    &s.ui_handler.peer_info.read().unwrap().version,
                );
                if is_support_multi_ui_session {
                    for display in displays_refresh.iter() {
                        s.refresh_video(*display);
                    }
                }
            }
            break;
        }
    }
}
