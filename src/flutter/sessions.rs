use super::*;

mod display;
pub use display::*;
use display::check_remove_unused_displays;

lazy_static::lazy_static! {
    // peer -> peer session, peer session -> ui sessions
    static ref SESSIONS: RwLock<HashMap<(String, ConnType), FlutterSession>> = Default::default();
}

#[inline]
pub fn get_session_count(peer_id: String, conn_type: ConnType) -> usize {
    SESSIONS
        .read()
        .unwrap()
        .get(&(peer_id, conn_type))
        .map(|s| s.ui_handler.session_handlers.read().unwrap().len())
        .unwrap_or(0)
}

#[inline]
pub fn get_peer_id_by_session_id(id: &SessionID, conn_type: ConnType) -> Option<String> {
    SESSIONS
        .read()
        .unwrap()
        .iter()
        .find_map(|((peer_id, t), s)| {
            if *t == conn_type
                && s.ui_handler
                    .session_handlers
                    .read()
                    .unwrap()
                    .contains_key(id)
            {
                Some(peer_id.clone())
            } else {
                None
            }
        })
}

#[inline]
pub fn get_session_by_session_id(id: &SessionID) -> Option<FlutterSession> {
    SESSIONS
        .read()
        .unwrap()
        .values()
        .find(|s| {
            s.ui_handler
                .session_handlers
                .read()
                .unwrap()
                .contains_key(id)
        })
        .cloned()
}

#[inline]
pub fn get_session_by_peer_id(peer_id: String, conn_type: ConnType) -> Option<FlutterSession> {
    SESSIONS.read().unwrap().get(&(peer_id, conn_type)).cloned()
}

#[inline]
pub fn remove_session_by_session_id(id: &SessionID) -> Option<FlutterSession> {
    let mut remove_peer_key = None;
    for (peer_key, s) in SESSIONS.write().unwrap().iter_mut() {
        let mut write_lock = s.ui_handler.session_handlers.write().unwrap();
        let remove_ret = write_lock.remove(id);
        match remove_ret {
            Some(_) => {
                if write_lock.is_empty() {
                    remove_peer_key = Some(peer_key.clone());
                } else {
                    check_remove_unused_displays(None, id, s, &write_lock);
                }
                break;
            }
            None => {}
        }
    }
    let s = SESSIONS.write().unwrap().remove(&remove_peer_key?);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    update_session_count_to_server();
    s
}

/// Close every client session, returning how many peer sessions were closed.
///
/// Used when the UI is gone but the process keeps running, e.g. the Android
/// task is swiped away from recents while a foreground service keeps the
/// process alive. The orphaned `io_loop` would otherwise keep answering
/// `TestDelay`, so the peer never hits its inactivity timeout and the
/// session stays established with no way to close it.
#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn close_all_sessions() -> usize {
    // Release held keys before draining: the release path sends through
    // `get_cur_session()`, which resolves against SESSIONS, so draining
    // first would take TO_RELEASE and then silently drop every key-up,
    // leaving the key stuck on the controlled side. A no-op when nothing
    // is held.
    crate::keyboard::release_remote_keys("map");
    // Drain so the map lock is released before closing each session.
    let sessions: Vec<FlutterSession> = SESSIONS
        .write()
        .unwrap()
        .drain()
        .map(|(_, session)| session)
        .collect();
    for session in sessions.iter() {
        let session_ids: Vec<SessionID> = session
            .ui_handler
            .session_handlers
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        for session_id in session_ids {
            session.close_event_stream(session_id);
        }
        session.close();
    }
    sessions.len()
}

/// Check if removing a session by session_id would result in removing the entire peer.
///
/// Returns:
/// - `true`: The session exists and removing it would leave the peer with no other sessions,
///           so the entire peer would be removed (equivalent to `remove_session_by_session_id` returning `Some`)
/// - `false`: The session doesn't exist, or it exists but the peer has other sessions,
///            so the peer would not be removed (equivalent to `remove_session_by_session_id` returning `None`)
#[inline]
pub fn would_remove_peer_by_session_id(id: &SessionID) -> bool {
    for (_peer_key, s) in SESSIONS.read().unwrap().iter() {
        let read_lock = s.ui_handler.session_handlers.read().unwrap();
        if read_lock.contains_key(id) {
            // Found the session, check if it's the only one for this peer
            return read_lock.len() == 1;
        }
    }
    // Session not found
    false
}

#[inline]
pub fn insert_session(session_id: SessionID, conn_type: ConnType, session: FlutterSession) {
    SESSIONS
        .write()
        .unwrap()
        .entry((session.get_id(), conn_type))
        .or_insert(session)
        .ui_handler
        .session_handlers
        .write()
        .unwrap()
        .insert(session_id, Default::default());
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    update_session_count_to_server();
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn update_session_count_to_server() {
    crate::ipc::update_controlling_session_count(SESSIONS.read().unwrap().len()).ok();
}

#[inline]
pub fn insert_peer_session_id(
    peer_id: String,
    conn_type: ConnType,
    session_id: SessionID,
    displays: Vec<i32>,
) -> bool {
    if let Some(s) = SESSIONS.read().unwrap().get(&(peer_id, conn_type)) {
        let mut h = SessionHandler::default();
        h.displays = displays.iter().map(|x| *x as usize).collect::<_>();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        let is_support_multi_ui_session = crate::common::is_support_multi_ui_session(
            &s.ui_handler.peer_info.read().unwrap().version,
        );
        #[cfg(any(target_os = "android", target_os = "ios"))]
        let is_support_multi_ui_session = false;
        h.renderer.is_support_multi_ui_session = is_support_multi_ui_session;
        let _ = s
            .ui_handler
            .session_handlers
            .write()
            .unwrap()
            .insert(session_id, h);
        // If the session is a single display session, it may be a software rgba rendered display.
        // If this is the second time the display is opened, the old valid flag may be true.
        if displays.len() == 1 {
            s.ui_handler.next_rgba(displays[0] as usize);
        }
        true
    } else {
        false
    }
}

#[inline]
pub fn get_sessions() -> Vec<FlutterSession> {
    SESSIONS.read().unwrap().values().cloned().collect()
}

#[inline]
#[cfg(not(target_os = "ios"))]
pub fn has_sessions_running(conn_type: ConnType) -> bool {
    SESSIONS.read().unwrap().iter().any(|((_, r#type), s)| {
        *r#type == conn_type && s.session_handlers.read().unwrap().len() != 0
    })
}

#[inline]
#[cfg(not(target_os = "ios"))]
pub fn has_connected_sessions_running(conn_type: ConnType) -> bool {
    SESSIONS.read().unwrap().iter().any(|((_, r#type), s)| {
        *r#type == conn_type
            && s.session_handlers.read().unwrap().len() != 0
            && s.connection_round_state.lock().unwrap().is_connected()
    })
}
