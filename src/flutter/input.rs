use super::*;

#[inline]
fn session_send_touch_scale(
    session_id: SessionID,
    v: &serde_json::Value,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    match v.get("v").and_then(|s| s.as_i64()) {
        Some(scale) => {
            if let Some(session) = sessions::get_session_by_session_id(&session_id) {
                session.send_touch_scale(scale as _, alt, ctrl, shift, command);
            }
        }
        None => {}
    }
}

#[inline]
fn session_send_touch_pan(
    session_id: SessionID,
    v: &serde_json::Value,
    pan_event: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    match v.get("v") {
        Some(v) => match (
            v.get("x").and_then(|x| x.as_i64()),
            v.get("y").and_then(|y| y.as_i64()),
        ) {
            (Some(x), Some(y)) => {
                if let Some(session) = sessions::get_session_by_session_id(&session_id) {
                    session
                        .send_touch_pan_event(pan_event, x as _, y as _, alt, ctrl, shift, command);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn session_send_touch_event(
    session_id: SessionID,
    v: &serde_json::Value,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    match v.get("t").and_then(|t| t.as_str()) {
        Some("scale") => session_send_touch_scale(session_id, v, alt, ctrl, shift, command),
        Some(pan_event) => {
            session_send_touch_pan(session_id, v, pan_event, alt, ctrl, shift, command)
        }
        _ => {}
    }
}

pub fn session_send_pointer(session_id: SessionID, msg: String) {
    if let Ok(m) = serde_json::from_str::<HashMap<String, serde_json::Value>>(&msg) {
        let alt = m.get("alt").is_some();
        let ctrl = m.get("ctrl").is_some();
        let shift = m.get("shift").is_some();
        let command = m.get("command").is_some();
        match (m.get("k"), m.get("v")) {
            (Some(k), Some(v)) => match k.as_str() {
                Some("touch") => session_send_touch_event(session_id, v, alt, ctrl, shift, command),
                _ => {}
            },
            _ => {}
        }
    }
}

#[inline]
pub fn session_on_waiting_for_image_dialog_show(session_id: SessionID) {
    for s in sessions::get_sessions() {
        if let Some(h) = s.session_handlers.write().unwrap().get_mut(&session_id) {
            h.on_waiting_for_image_dialog_show();
        }
    }
}
