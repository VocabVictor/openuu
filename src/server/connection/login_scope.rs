use super::*;

impl Connection {
    #[inline]
    pub fn is_permission_enabled_locally(enable_prefix_option: &str) -> bool {
        #[cfg(feature = "flutter")]
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        {
            let access_mode = Config::get_option("access-mode");
            if access_mode == "full" {
                return true;
            } else if access_mode == "view" {
                return false;
            }
        }
        config::option2bool(
            enable_prefix_option,
            &Config::get_option(enable_prefix_option),
        )
    }

    pub(super) fn permission(
        enable_prefix_option: &str,
        control_permissions: &Option<ControlPermissions>,
    ) -> bool {
        use hbb_common::rendezvous_proto::control_permissions::Permission;
        if let Some(control_permissions) = control_permissions {
            let permission = match enable_prefix_option {
                keys::OPTION_ENABLE_KEYBOARD => Some(Permission::keyboard),
                keys::OPTION_ENABLE_CLIPBOARD => Some(Permission::clipboard),
                keys::OPTION_ENABLE_FILE_TRANSFER => Some(Permission::file),
                keys::OPTION_ENABLE_AUDIO => Some(Permission::audio),
                keys::OPTION_ENABLE_CAMERA => Some(Permission::camera),
                keys::OPTION_ENABLE_TERMINAL => Some(Permission::terminal),
                keys::OPTION_ENABLE_TUNNEL => Some(Permission::tunnel),
                keys::OPTION_ENABLE_REMOTE_RESTART => Some(Permission::restart),
                keys::OPTION_ENABLE_RECORD_SESSION => Some(Permission::recording),
                keys::OPTION_ENABLE_BLOCK_INPUT => Some(Permission::block_input),
                keys::OPTION_ENABLE_PRIVACY_MODE => Some(Permission::privacy_mode),
                _ => None,
            };
            if let Some(permission) = permission {
                if let Some(enabled) =
                    crate::get_control_permission(control_permissions.permissions, permission)
                {
                    return enabled;
                }
            }
        }
        Self::is_permission_enabled_locally(enable_prefix_option)
    }

    pub(super) fn update_codec_on_login(&self) {
        use scrap::codec::{Encoder, EncodingUpdate::*};
        if let Some(o) = self.lr.clone().option.as_ref() {
            if let Some(q) = o.supported_decoding.clone().take() {
                Encoder::update(Update(self.inner.id(), q));
            } else {
                Encoder::update(NewOnlyVP9(self.inner.id()));
            }
        } else {
            Encoder::update(NewOnlyVP9(self.inner.id()));
        }
    }

    #[inline]
    pub(super) fn enable_trusted_devices() -> bool {
        config::option2bool(
            keys::OPTION_ENABLE_TRUSTED_DEVICES,
            &Config::get_option(keys::OPTION_ENABLE_TRUSTED_DEVICES),
        )
    }

    pub(super) fn reset_session_scope_for_login(&mut self) {
        self.file_transfer = None;
        self.view_camera = false;
        self.terminal = false;
        self.port_forward_address.clear();
        self.terminal_persistent = false;
    }

    // Approval and whitelist decisions must stay bound to the same controller identity and
    // session scope across authentication retries.
    pub(super) fn login_scope_digest(lr: &LoginRequest) -> [u8; 32] {
        let mut hasher = Sha256::new();
        // Length-prefixed so adjacent fields cannot alias.
        let mut push = |bytes: &[u8]| {
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        };
        push(lr.my_id.as_bytes());
        // Payloads are destructured exhaustively: a new field fails to compile until it is
        // either latched here or deliberately ignored.
        match lr.union.as_ref() {
            Some(login_request::Union::FileTransfer(ft)) => {
                let FileTransfer {
                    dir,
                    show_hidden,
                    special_fields: _,
                } = ft;
                push(b"file_transfer");
                push(dir.as_bytes());
                push(&[*show_hidden as u8]);
            }
            Some(login_request::Union::ViewCamera(vc)) => {
                let ViewCamera { special_fields: _ } = vc;
                push(b"view_camera");
            }
            Some(login_request::Union::Terminal(t)) => {
                let Terminal {
                    service_id,
                    special_fields: _,
                } = t;
                push(b"terminal");
                push(service_id.as_bytes());
            }
            Some(login_request::Union::PortForward(pf)) => {
                let PortForward {
                    host,
                    port,
                    multiplex,
                    special_fields: _,
                } = pf;
                push(b"port_forward");
                push(host.as_bytes());
                push(&port.to_le_bytes());
                push(&[*multiplex as u8]);
            }
            // Variants this build does not know execute as remote, so they latch as remote.
            None | Some(_) => push(b"remote"),
        }
        hasher.finalize().into()
    }

    // Logging only; security decisions compare digests.
    pub(super) fn login_scope_kind(lr: &LoginRequest) -> &'static str {
        match lr.union.as_ref() {
            Some(login_request::Union::FileTransfer(_)) => "file_transfer",
            Some(login_request::Union::ViewCamera(_)) => "view_camera",
            Some(login_request::Union::Terminal(_)) => "terminal",
            Some(login_request::Union::PortForward(_)) => "port_forward",
            _ => "remote",
        }
    }

    pub(super) async fn check_login_scope(&mut self, lr: &LoginRequest) -> bool {
        let requested = Self::login_scope_digest(lr);
        match self.login_scope {
            Some(initial) if initial != requested => {
                // self.lr still holds the first accepted request, whose scope is the latched one.
                log::warn!(
                    "Rejected login scope change: conn_id={}, initial={}, requested={}",
                    self.inner.id(),
                    Self::login_scope_kind(&self.lr),
                    Self::login_scope_kind(lr),
                );
                self.send_login_error("Connection not allowed").await;
                false
            }
            Some(_) => true,
            None => {
                self.login_scope = Some(requested);
                true
            }
        }
    }

    pub(super) async fn handle_login_request_without_validation(&mut self, lr: &LoginRequest) {
        self.lr = lr.clone();
        self.peer_argb = crate::str2color(&format!("{}{}", &lr.my_id, &lr.my_platform), 0xff);
        if let Some(o) = lr.option.as_ref() {
            self.options_in_login = Some(o.clone());
        }
        if self.require_2fa.is_some() && !lr.hwid.is_empty() && Self::enable_trusted_devices() {
            let devices = Config::get_trusted_devices();
            if let Some(device) = devices.iter().find(|d| d.hwid == lr.hwid) {
                if !device.outdate()
                    && device.id == lr.my_id
                    && device.name == lr.my_name
                    && device.platform == lr.my_platform
                {
                    log::info!("2FA bypassed by trusted devices");
                    self.set_conn_audit_two_factor(ConnAuditTwoFactor::TrustedDevice);
                    self.require_2fa = None;
                }
            }
        }
        self.video_ack_required = lr.video_ack_required;
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn try_start_cm_ipc(&mut self) {
        if let Some(p) = self.start_cm_ipc_para.take() {
            tokio::spawn(async move {
                #[cfg(windows)]
                let tx_from_cm_clone = p.tx_from_cm.clone();
                if let Err(err) = start_ipc(p.rx_to_cm, p.tx_from_cm).await {
                    log::warn!("ipc to connection manager exit: {}", err);
                    // https://github.com/rustdesk/rustdesk-server-pro/discussions/382#discussioncomment-10525725, cm may start failed
                    #[cfg(windows)]
                    if !crate::platform::is_prelogin()
                        && !err.to_string().contains(crate::platform::EXPLORER_EXE)
                        && !crate::hbbs_http::sync::is_pro()
                    {
                        allow_err!(tx_from_cm_clone.send(Data::CmErr(err.to_string())));
                    }
                }
            });
            #[cfg(all(windows, feature = "flutter"))]
            std::thread::spawn(move || {
                if crate::is_server() && !crate::check_process("--tray", false) {
                    crate::platform::run_as_user(vec!["--tray"]).ok();
                }
            });
        }
    }
}
