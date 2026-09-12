// ALIVE_CONNS: all connections, including unauthorized connections
// AUTHED_CONNS: all authorized connections
// CONTROL_PERMISSIONS_ARRAY: all non-None control permissions

use super::*;
pub struct ConnectionID(i32);

impl ConnectionID {
    pub fn new(id: i32) -> Self {
        ALIVE_CONNS.lock().unwrap().push(id);
        Self(id)
    }
}

impl Drop for ConnectionID {
    fn drop(&mut self) {
        let mut active_conns_lock = ALIVE_CONNS.lock().unwrap();
        active_conns_lock.retain(|&c| c != self.0);
    }
}

pub struct AuthedConnID(i32, AuthConnType);

impl AuthedConnID {
    pub(super) fn is_newer_session_remote(c: &AuthedConn, id: i32, key: &SessionKey) -> bool {
        c.conn_id > id && c.conn_type == AuthConnType::Remote && &c.session_key == key
    }

    /// Whether a newer remote control connection of this session has replaced this one. A
    /// controlling peer whose link dies reconnects while the connection it left behind runs
    /// on here until its own timeout; locking for that one would lock a session that has
    /// already resumed on its replacement.
    pub fn session_reconnected(id: i32, key: &SessionKey) -> bool {
        let conns = AUTHED_CONNS.lock().unwrap();
        conns
            .iter()
            .any(|c| Self::is_newer_session_remote(c, id, key))
    }

    pub fn new(
        conn_id: i32,
        conn_type: AuthConnType,
        session_key: SessionKey,
        sender: mpsc::UnboundedSender<Data>,
        lr: LoginRequest,
    ) -> Self {
        AUTHED_CONNS.lock().unwrap().push(AuthedConn {
            conn_id,
            conn_type,
            session_key,
            sender,
        });
        Self::check_wake_lock();
        use std::sync::Once;
        static _ONCE: Once = Once::new();
        _ONCE.call_once(|| {
            shutdown_hooks::add_shutdown_hook(connection_shutdown_hook);
        });
        if conn_type == AuthConnType::Remote || conn_type == AuthConnType::ViewCamera {
            video_service::VIDEO_QOS
                .lock()
                .unwrap()
                .on_connection_open(conn_id);
        }
        Self(conn_id, conn_type)
    }

    fn check_wake_lock() {
        let conn_count = AUTHED_CONNS.lock().unwrap().len();
        let remote_count = AUTHED_CONNS
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.conn_type == AuthConnType::Remote)
            .count();
        allow_err!(WAKELOCK_SENDER
            .lock()
            .unwrap()
            .send((conn_count, remote_count)));
    }

    pub fn check_wake_lock_on_setting_changed() {
        let current =
            config::Config::get_bool_option(keys::OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS);
        let cached = *WAKELOCK_KEEP_AWAKE_OPTION.lock().unwrap();
        if cached != Some(current) {
            Self::check_wake_lock();
        }
    }

    #[cfg(windows)]
    pub fn non_port_forward_conn_count() -> usize {
        AUTHED_CONNS
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.conn_type != AuthConnType::PortForward)
            .count()
    }

    pub fn check_remove_session(conn_id: i32, key: SessionKey) {
        let mut lock = SESSIONS.lock().unwrap();
        let contains = lock.contains_key(&key);
        if contains {
            // No two remote connections with the same session key, just for ensure.
            let is_remote = AUTHED_CONNS
                .lock()
                .unwrap()
                .iter()
                .any(|c| c.conn_id == conn_id && c.conn_type == AuthConnType::Remote);
            // If there are 2 connections with the same peer_id and session_id, a remote connection and a file transfer or port forward connection,
            // If any of the connections is closed allowing retry, this will not be called;
            // If the file transfer/port forward connection is closed with no retry, the session should be kept for remote control menu action;
            // If the remote connection is closed with no retry, keep the session is not reasonable in case there is a retry button in the remote side, and ignore network fluctuations.
            let another_remote = AUTHED_CONNS.lock().unwrap().iter().any(|c| {
                c.conn_id != conn_id
                    && c.session_key == key
                    && c.conn_type == AuthConnType::Remote
            });
            if is_remote || !another_remote {
                lock.remove(&key);
                log::info!("remove session");
            } else {
                // Keep the session if there is another remote connection with same peer_id and session_id.
                log::info!("skip remove session");
            }
        }
    }

    pub fn update_or_insert_session(
        key: SessionKey,
        password: Option<String>,
        tfa: Option<bool>,
    ) {
        let mut lock = SESSIONS.lock().unwrap();
        let session = lock.get_mut(&key);
        if let Some(session) = session {
            if let Some(password) = password {
                session.random_password = password;
            }
            if let Some(tfa) = tfa {
                session.tfa = tfa;
            }
        } else {
            lock.insert(
                key,
                Session {
                    random_password: password.unwrap_or_default(),
                    tfa: tfa.unwrap_or_default(),
                    last_recv_time: Arc::new(Mutex::new(Instant::now())),
                },
            );
        }
    }

    pub fn set_session_2fa(key: SessionKey) {
        let mut lock = SESSIONS.lock().unwrap();
        let session = lock.get_mut(&key);
        if let Some(session) = session {
            session.tfa = true;
        } else {
            lock.insert(
                key,
                Session {
                    last_recv_time: Arc::new(Mutex::new(Instant::now())),
                    random_password: "".to_owned(),
                    tfa: true,
                },
            );
        }
    }

    pub fn conn_type(&self) -> AuthConnType {
        self.1
    }
}

impl Drop for AuthedConnID {
    fn drop(&mut self) {
        if self.1 == AuthConnType::Remote || self.1 == AuthConnType::ViewCamera {
            scrap::codec::Encoder::update(scrap::codec::EncodingUpdate::Remove(self.0));
            video_service::VIDEO_QOS
                .lock()
                .unwrap()
                .on_connection_close(self.0);
        }
        // Clear per-connection state to avoid stale behavior if conn ids are reused.
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        clear_relative_mouse_active(self.0);
        AUTHED_CONNS.lock().unwrap().retain(|c| c.conn_id != self.0);
        let remote_count = AUTHED_CONNS
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.conn_type == AuthConnType::Remote)
            .count();
        if remote_count == 0 {
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            {
                *WALLPAPER_REMOVER.lock().unwrap() = None;
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            display_service::restore_resolutions();
            #[cfg(windows)]
            let _ = virtual_display_manager::reset_all();
            #[cfg(target_os = "linux")]
            scrap::wayland::pipewire::try_close_session();
        }
        Self::check_wake_lock();
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            use crate::whiteboard;
            whiteboard::unregister_whiteboard(whiteboard::get_key_cursor(self.0));
        }
    }
}

pub struct ControlPermissionsID {
    id: i32,
    control_permissions: Option<ControlPermissions>,
}

impl Drop for ControlPermissionsID {
    fn drop(&mut self) {
        if self.control_permissions.is_some() {
            let mut lock = CONTROL_PERMISSIONS_ARRAY.lock().unwrap();
            lock.retain(|(conn_id, _)| *conn_id != self.id);
        }
    }
}
impl ControlPermissionsID {
    pub fn new(id: i32, control_permissions: &Option<ControlPermissions>) -> Self {
        if let Some(s) = control_permissions {
            CONTROL_PERMISSIONS_ARRAY
                .lock()
                .unwrap()
                .push((id, s.clone()));
        }
        Self {
            id,
            control_permissions: control_permissions.clone(),
        }
    }
}
