use super::*;

// This function is only used for the default connection session.
pub fn session_add_existed(
    peer_id: String,
    session_id: SessionID,
    displays: Vec<i32>,
    is_view_camera: bool,
) -> ResultType<()> {
    let conn_type = if is_view_camera {
        ConnType::VIEW_CAMERA
    } else {
        ConnType::DEFAULT_CONN
    };
    sessions::insert_peer_session_id(peer_id, conn_type, session_id, displays);
    Ok(())
}

/// Create a new remote session with the given id.
///
/// # Arguments
///
/// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
/// * `is_file_transfer` - If the session is used for file transfer.
/// * `is_view_camera` - If the session is used for view camera.
/// * `is_port_forward` - If the session is used for port forward.
pub fn session_add(
    session_id: &SessionID,
    id: &str,
    is_file_transfer: bool,
    is_view_camera: bool,
    is_port_forward: bool,
    is_rdp: bool,
    is_terminal: bool,
    switch_uuid: &str,
    force_relay: bool,
    password: String,
    is_shared_password: bool,
    conn_token: Option<String>,
) -> ResultType<FlutterSession> {
    let conn_type = if is_file_transfer {
        ConnType::FILE_TRANSFER
    } else if is_view_camera {
        ConnType::VIEW_CAMERA
    } else if is_terminal {
        ConnType::TERMINAL
    } else if is_port_forward {
        if is_rdp {
            ConnType::RDP
        } else {
            ConnType::PORT_FORWARD
        }
    } else {
        ConnType::DEFAULT_CONN
    };

    // to-do: check the same id session.
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        if session.lc.read().unwrap().conn_type != conn_type {
            bail!("same session id is found with different conn type?");
        }
        // The same session is added before?
        bail!("same session id is found");
    }

    LocalConfig::set_remote_id(&id);

    let mut preset_password = password.clone();
    let shared_password = if is_shared_password {
        // To achieve a flexible password application order, we don't treat shared password as a preset password.
        preset_password = Default::default();
        Some(password)
    } else {
        None
    };

    let session: Session<FlutterHandler> = Session {
        password: preset_password,
        server_keyboard_enabled: Arc::new(RwLock::new(true)),
        server_file_transfer_enabled: Arc::new(RwLock::new(true)),
        server_clipboard_enabled: Arc::new(RwLock::new(true)),
        reconnect_count: Arc::new(AtomicUsize::new(0)),
        ..Default::default()
    };

    let switch_uuid = if switch_uuid.is_empty() {
        None
    } else {
        Some(switch_uuid.to_string())
    };

    session.lc.write().unwrap().initialize(
        id.to_owned(),
        conn_type,
        switch_uuid,
        force_relay,
        get_adapter_luid(),
        shared_password,
        conn_token,
    );

    let session = Arc::new(session.clone());
    sessions::insert_session(session_id.to_owned(), conn_type, session.clone());

    Ok(session)
}

/// start a session with the given id.
///
/// # Arguments
///
/// * `id` - The identifier of the remote session with prefix. Regex: [\w]*[\_]*[\d]+
/// * `events2ui` - The events channel to ui.
pub fn session_start_(
    session_id: &SessionID,
    id: &str,
    event_stream: StreamSink<EventToUI>,
) -> ResultType<()> {
    // is_connected is used to indicate whether to start a peer connection. For two cases:
    // 1. "Move tab to new window"
    // 2. multi ui session within the same peer connection.
    let mut is_connected = false;
    let mut is_found = false;
    for s in sessions::get_sessions() {
        if let Some(h) = s.session_handlers.write().unwrap().get_mut(session_id) {
            is_connected = h.event_stream.is_some();
            try_send_close_event(&h.event_stream);
            h.event_stream = Some(event_stream);
            is_found = true;
            break;
        }
    }
    if !is_found {
        bail!(
            "No session with peer id {}, session id: {}",
            id,
            session_id.to_string()
        );
    }

    if let Some(session) = sessions::get_session_by_session_id(session_id) {
        let is_first_ui_session = session.session_handlers.read().unwrap().len() == 1;
        if !is_connected && is_first_ui_session {
            log::info!(
                "Session {} start, use texture render: {}",
                id,
                session.use_texture_render.load(Ordering::Relaxed)
            );
            let session = (*session).clone();
            std::thread::spawn(move || {
                let round = session.connection_round_state.lock().unwrap().new_round();
                io_loop(session, round);
            });
        }
        Ok(())
    } else {
        bail!("No session with peer id {}", id)
    }
}

#[inline]
pub(super) fn try_send_close_event(event_stream: &Option<StreamSink<EventToUI>>) {
    if let Some(stream) = &event_stream {
        stream.add(EventToUI::Event("close".to_owned()));
    }
}

pub(super) fn char_to_session_id(c: *const char) -> ResultType<SessionID> {
    if c.is_null() {
        bail!("Session id ptr is null");
    }
    let cstr = unsafe { std::ffi::CStr::from_ptr(c as _) };
    let str = cstr.to_str()?;
    SessionID::from_str(str).map_err(|e| anyhow!("{:?}", e))
}

pub fn session_get_rgba_size(session_id: SessionID, display: usize) -> usize {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        return session
            .display_rgbas
            .read()
            .unwrap()
            .get(&display)
            .map_or(0, |rgba| rgba.data.len());
    }
    0
}

#[no_mangle]
pub extern "C" fn session_get_rgba(session_uuid_str: *const char, display: usize) -> *const u8 {
    if let Ok(session_id) = char_to_session_id(session_uuid_str) {
        if let Some(s) = sessions::get_session_by_session_id(&session_id) {
            return s.ui_handler.get_rgba(display);
        }
    }

    std::ptr::null()
}

pub fn session_next_rgba(session_id: SessionID, display: usize) {
    if let Some(s) = sessions::get_session_by_session_id(&session_id) {
        return s.ui_handler.next_rgba(display);
    }
}

#[inline]
pub fn session_set_size(session_id: SessionID, display: usize, width: usize, height: usize) {
    for s in sessions::get_sessions() {
        if let Some(h) = s
            .ui_handler
            .session_handlers
            .write()
            .unwrap()
            .get_mut(&session_id)
        {
            // If the session is the first connection, displays is not set yet.
            // `displays`` is set while switching displays or adding a new session.
            if !h.displays.contains(&display) {
                h.displays.push(display);
            }
            h.renderer.set_size(display, width, height);
            break;
        }
    }
}

#[inline]
pub fn session_register_pixelbuffer_texture(session_id: SessionID, display: usize, ptr: usize) {
    for s in sessions::get_sessions() {
        if let Some(h) = s
            .ui_handler
            .session_handlers
            .read()
            .unwrap()
            .get(&session_id)
        {
            h.renderer.register_pixelbuffer_texture(display, ptr);
            break;
        }
    }
}

#[inline]
pub fn session_register_gpu_texture(_session_id: SessionID, _display: usize, _output_ptr: usize) {
    #[cfg(feature = "vram")]
    for s in sessions::get_sessions() {
        if let Some(h) = s
            .ui_handler
            .session_handlers
            .read()
            .unwrap()
            .get(&_session_id)
        {
            h.renderer.register_gpu_output(_display, _output_ptr);
            break;
        }
    }
}
