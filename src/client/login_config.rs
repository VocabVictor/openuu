use super::*;

// The source of sent password
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum PasswordSource {
    PersonalAb(Vec<u8>),
    SharedAb(String),
    Undefined,
}

impl Default for PasswordSource {
    fn default() -> Self {
        PasswordSource::Undefined
    }
}

impl PasswordSource {
    // Whether the password is personal ab password
    pub fn is_personal_ab(&self, password: &[u8]) -> bool {
        if password.is_empty() {
            return false;
        }
        match self {
            PasswordSource::PersonalAb(p) => p == password,
            _ => false,
        }
    }

    // Whether the password is shared ab password
    pub fn is_shared_ab(&self, password: &[u8], hash: &Hash) -> bool {
        if password.is_empty() {
            return false;
        }
        match self {
            PasswordSource::SharedAb(p) => Self::equal(p, password, hash),
            _ => false,
        }
    }

    //  Whether the password equals to the connected password
    pub(super) fn equal(password: &str, connected_password: &[u8], hash: &Hash) -> bool {
        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(&hash.salt);
        let res = hasher.finalize();
        connected_password[..] == res[..]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ConnToken {
    pub(super) password: Vec<u8>,
    pub(super) password_source: PasswordSource,
    pub(super) session_id: u64,
}

/// Login config handler for [`Client`].
#[derive(Default)]
pub struct LoginConfigHandler {
    pub view_only_session: bool,
    pub(super) id: String,
    pub conn_type: ConnType,
    pub is_terminal_admin: bool,
    pub(super) hash: Hash,
    pub(super) password: Vec<u8>, // remember password for reconnect
    pub remember: bool,
    pub(super) config: PeerConfig,
    pub port_forward: (String, i32),
    /// This login's `multiplex`, filled with `port_forward` under the turn
    /// lock. `port_forward_mux` says whether a mapping probes for the tunnel;
    /// one the probe latched to the raw pipe logs in without asking, so an
    /// upgraded peer keeps giving it the raw pipe.
    pub(crate) port_forward_multiplex: bool,
    /// Set once per window, before its mappings start: every accept's claim
    /// reads it.
    pub(crate) port_forward_mux: bool,
    /// Held by a port-forward mapping from filling `port_forward` and `hash`
    /// until its login is built from them; a window's mappings log in
    /// concurrently.
    pub(crate) port_forward_login_turn: Arc<hbb_common::tokio::sync::Mutex<()>>,
    pub version: i64,
    pub(super) features: Option<Features>,
    pub session_id: u64, // used for local <-> server communication
    pub supported_encoding: SupportedEncoding,
    pub(super) restarting_remote_device: bool,
    // Start time of the restart grace window. On Windows the peer may briefly
    // reconnect before the real reboot disconnect.
    pub(super) restart_remote_device_at: Option<Instant>,
    pub force_relay: bool,
    // force_relay minus the WebSocket transport. ws forces relay for classic punching but says
    // nothing about ICE, so every WebRTC decision keys off this: policy means Relay-only ICE,
    // transport may still go direct.
    pub policy_relay: bool,
    // The peer-scoped part of it, and the only part that may be written back to the peer.
    pub peer_relay: bool,
    pub direct: Option<bool>,
    pub received: bool,
    pub(super) switch_uuid: Option<String>,
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) switch_back_allowed: bool,
    pub save_ab_password_to_recent: bool, // true: connected with ab password
    pub other_server: Option<(String, String, String)>,
    pub custom_fps: Arc<Mutex<Option<usize>>>,
    pub last_auto_fps: Option<usize>,
    pub adapter_luid: Option<i64>,
    pub mark_unsupported: Vec<CodecFormat>,
    pub selected_windows_session_id: Option<u32>,
    pub peer_info: Option<PeerInfo>,
    pub(super) password_source: PasswordSource, // where the sent password comes from
    pub(super) shared_password: Option<String>, // Store the shared password
    pub enable_trusted_devices: bool,
    pub record_state: bool,
    pub record_permission: bool,
}

impl Deref for LoginConfigHandler {
    type Target = PeerConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl LoginConfigHandler {
    pub(crate) fn set_hash(&mut self, hash: Hash) {
        self.hash = hash;
    }

    /// Initialize the login config handler.
    ///
    /// # Arguments
    ///
    /// * `id` - id of peer
    /// * `conn_type` - Connection type enum.
    pub fn initialize(
        &mut self,
        id: String,
        conn_type: ConnType,
        switch_uuid: Option<String>,
        mut force_relay: bool,
        adapter_luid: Option<i64>,
        shared_password: Option<String>,
        conn_token: Option<String>,
    ) {
        let mut id = id;
        if id.contains("@") {
            let mut v = id.split("@");
            let raw_id: &str = v.next().unwrap_or_default();
            let mut server_key = v.next().unwrap_or_default().split('?');
            let server = server_key.next().unwrap_or_default();
            let args = server_key.next().unwrap_or_default();
            let key = if server == PUBLIC_SERVER {
                config::RS_PUB_KEY.to_owned()
            } else {
                let mut args_map: HashMap<String, &str> = HashMap::new();
                for arg in args.split('&') {
                    if let Some(kv) = arg.find('=') {
                        let k = arg[0..kv].to_lowercase();
                        let v = &arg[kv + 1..];
                        args_map.insert(k, v);
                    }
                }
                let key = args_map.remove("key").unwrap_or_default();
                key.to_owned()
            };

            // here we can check <id>/r@server
            let real_id = crate::ui_interface::handle_relay_id(raw_id).to_string();
            if real_id != raw_id {
                force_relay = true;
            }
            self.other_server = Some((real_id.clone(), server.to_owned(), key));
            id = format!("{real_id}@{server}");
        } else {
            let real_id = crate::ui_interface::handle_relay_id(&id);
            if real_id != id {
                force_relay = true;
                id = real_id.to_owned();
            }
        }

        self.id = id;
        self.conn_type = conn_type;
        let config = self.load_config();
        self.remember = !config.password.is_empty();
        self.config = config;

        let conn_token = conn_token
            .map(|x| serde_json::from_str::<ConnToken>(&x).ok())
            .flatten();
        let mut sid = 0;
        if let Some(token) = conn_token {
            sid = token.session_id;
            self.password = token.password; // use as last password
            self.password_source = token.password_source;
        }
        if sid == 0 {
            sid = rand::random();
            if sid == 0 {
                // you won the lottery
                sid = 1;
            }
        }
        self.session_id = sid;
        self.supported_encoding = Default::default();
        self.clear_restarting_remote_device();
        // Three scopes: what was decided about this PEER, what this CLIENT is set up as (proxy),
        // and what its TRANSPORT forces (ws). Only the first may be written back to the peer's
        // config — persisting the others would make a local setup a permanent peer property.
        self.peer_relay =
            config::option2bool("force-always-relay", &self.get_option("force-always-relay"))
                || force_relay;
        self.policy_relay = self.peer_relay || Config::is_proxy();
        self.force_relay = self.policy_relay || use_ws();
        if let Some((real_id, server, key)) = &self.other_server {
            let other_server_key = self.get_option("other-server-key");
            if !other_server_key.is_empty() && key.is_empty() {
                self.other_server = Some((real_id.to_owned(), server.to_owned(), other_server_key));
            }
        }

        self.direct = None;
        self.received = false;
        #[cfg(feature = "flutter")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            self.switch_back_allowed = false;
        }
        self.switch_uuid = switch_uuid;
        self.adapter_luid = adapter_luid;
        self.selected_windows_session_id = None;
        self.shared_password = shared_password;
        self.record_state = false;
        self.record_permission = true;

        // `std::env::remove_var("IS_TERMINAL_ADMIN");` is called in `session_add_sync()` - `flutter_ffi.rs`.
        let is_terminal_admin = conn_type == ConnType::TERMINAL
            && std::env::var("IS_TERMINAL_ADMIN").map_or(false, |v| v == "Y");
        self.is_terminal_admin = is_terminal_admin;
    }

    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn allow_switch_back_once(&mut self) {
        self.switch_back_allowed = true;
    }

    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn consume_switch_back_permission(&mut self) -> bool {
        if self.switch_back_allowed {
            self.switch_back_allowed = false;
            true
        } else {
            false
        }
    }

    /// Check if the client should auto login.
    /// Return password if the client should auto login, otherwise return empty string.
    pub fn should_auto_login(&self) -> String {
        let l = self.lock_after_session_end.v;
        let a = !self.get_option("auto-login").is_empty();
        let p = self.get_option("os-password");
        if !p.is_empty() && l && a {
            p
        } else {
            "".to_owned()
        }
    }

    /// Load [`PeerConfig`].
    pub fn load_config(&self) -> PeerConfig {
        debug_assert!(self.id.len() > 0);
        PeerConfig::load(&self.id)
    }

    /// Save a [`PeerConfig`] into the handler.
    ///
    /// # Arguments
    ///
    /// * `config` - [`PeerConfig`] to save.
    pub fn save_config(&mut self, config: PeerConfig) {
        config.store(&self.id);
        self.config = config;
    }
}
