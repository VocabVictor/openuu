use super::*;

impl Connection {
    // Returns whether this connection should be kept alive.
    // `true` does not necessarily mean authorization succeeded (e.g. REQUIRE_2FA case).
    pub(super) async fn send_logon_response_and_keep_alive(&mut self) -> bool {
        if self.authorized {
            return true;
        }
        if self.require_2fa.is_some() && !self.is_recent_session(true) && !self.from_switch {
            self.require_2fa.as_ref().map(|totp| {
                let bot = crate::auth_2fa::TelegramBot::get();
                let bot = match bot {
                    Ok(Some(bot)) => bot,
                    Err(err) => {
                        log::error!("Failed to get telegram bot: {}", err);
                        return;
                    }
                    _ => return,
                };
                let code = totp.generate_current();
                if let Ok(code) = code {
                    let text = format!(
                        "2FA code: {}\n\nA new connection has been established to your device with ID {}. The source IP address is {}.",
                        code,
                        Config::get_id(),
                        self.ip,
                    );
                    tokio::spawn(async move {
                        if let Err(err) =
                            crate::auth_2fa::send_2fa_code_to_telegram(&text, bot).await
                        {
                            log::error!("Failed to send 2fa code to telegram bot: {}", err);
                        }
                    });
                }
            });
            self.awaiting_2fa = true;
            self.send_login_error(crate::client::REQUIRE_2FA).await;
            // Keep the connection alive so the client can continue with 2FA.
            return true;
        }
        self.awaiting_2fa = false;
        if let Some(keep_alive) = self.prepare_terminal_login_for_authorization().await {
            return keep_alive;
        }
        if !self.connect_port_forward_if_needed().await {
            return false;
        }
        self.authorized = true;
        // Releases the budget `check_id_whitelist` charges against this address: only a peer
        // that got this far proved more than a self-reported id.
        self.clear_id_whitelist_failures();
        let (conn_type, auth_conn_type) = if self.file_transfer.is_some() {
            (1, AuthConnType::FileTransfer)
        } else if self.is_port_forward() {
            (2, AuthConnType::PortForward)
        } else if self.view_camera {
            (3, AuthConnType::ViewCamera)
        } else if self.terminal {
            (4, AuthConnType::Terminal)
        } else {
            (0, AuthConnType::Remote)
        };
        self.authed_conn_id = Some(self::raii::AuthedConnID::new(
            self.inner.id(),
            auth_conn_type,
            self.session_key(),
            self.tx_from_authed.clone(),
            self.lr.clone(),
        ));
        self.session_last_recv_time = SESSIONS
            .lock()
            .unwrap()
            .get(&self.session_key())
            .map(|s| s.last_recv_time.clone());
        self.normalize_conn_audit_auth_fields();
        let mut audit = json!({"peer": ((&self.lr.my_id, &self.lr.my_name)), "type": conn_type});
        if self.conn_audit_primary_auth != ConnAuditPrimaryAuth::None {
            audit["primary_auth"] = json!(self.conn_audit_primary_auth.as_i64());
        }
        if self.conn_audit_two_factor != ConnAuditTwoFactor::None {
            audit["two_factor"] = json!(self.conn_audit_two_factor.as_i64());
        }
        self.post_conn_audit(audit);
        #[allow(unused_mut)]
        let mut username = crate::platform::get_active_username();
        let mut res = LoginResponse::new();
        let mut pi = PeerInfo {
            username: username.clone(),
            version: VERSION.to_owned(),
            ..Default::default()
        };

        #[cfg(not(target_os = "android"))]
        {
            pi.hostname = crate::whoami_hostname();
            pi.platform = hbb_common::whoami::platform().to_string();
        }
        #[cfg(target_os = "android")]
        {
            pi.hostname = DEVICE_NAME.lock().unwrap().clone();
            pi.platform = "Android".into();
        }
        #[cfg(all(target_os = "macos", not(feature = "unix-file-copy-paste")))]
        let mut platform_additions = serde_json::Map::new();
        #[cfg(any(
            target_os = "windows",
            target_os = "linux",
            all(target_os = "macos", feature = "unix-file-copy-paste")
        ))]
        let mut platform_additions = serde_json::Map::new();
        #[cfg(target_os = "linux")]
        {
            if crate::platform::current_is_wayland() {
                platform_additions.insert("is_wayland".into(), json!(true));
            }
        }
        #[cfg(target_os = "windows")]
        {
            platform_additions.insert(
                "is_installed".into(),
                json!(crate::platform::is_installed()),
            );
            if crate::platform::is_installed() {
                platform_additions.extend(virtual_display_manager::get_platform_additions());
            }
            platform_additions.insert(
                "supported_privacy_mode_impl".into(),
                json!(privacy_mode::get_supported_privacy_mode_impl()),
            );
        }
        #[cfg(target_os = "macos")]
        {
            platform_additions.insert(
                "supported_privacy_mode_impl".into(),
                json!(privacy_mode::get_supported_privacy_mode_impl()),
            );
        }

        #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
        {
            let is_both_windows = cfg!(target_os = "windows")
                && self.lr.my_platform == hbb_common::whoami::Platform::Windows.to_string();
            #[cfg(feature = "unix-file-copy-paste")]
            let is_unix_and_peer_supported = crate::is_support_file_copy_paste(&self.lr.version);
            #[cfg(not(feature = "unix-file-copy-paste"))]
            let is_unix_and_peer_supported = false;
            let is_both_macos = cfg!(target_os = "macos")
                && self.lr.my_platform == hbb_common::whoami::Platform::MacOS.to_string();
            let is_peer_support_paste_if_macos =
                crate::is_support_file_paste_if_macos(&self.lr.version);
            let has_file_clipboard = is_both_windows
                || (is_unix_and_peer_supported
                    && (!is_both_macos || is_peer_support_paste_if_macos));
            platform_additions.insert("has_file_clipboard".into(), json!(has_file_clipboard));
        }

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            platform_additions.insert("support_view_camera".into(), json!(true));
        }

        #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
        if !platform_additions.is_empty() {
            pi.platform_additions = serde_json::to_string(&platform_additions).unwrap_or("".into());
        }

        if self.is_port_forward() {
            pi.features = Some(Features {
                port_forward_mux: self.port_forward_mux.is_some(),
                ..Default::default()
            })
            .into();
            let mut msg_out = Message::new();
            res.set_peer_info(pi);
            msg_out.set_login_response(res);
            self.send(msg_out).await;
            return true;
        }
        #[cfg(target_os = "linux")]
        if self.is_remote() {
            let mut msg = "".to_string();
            // Refuse only while nothing can capture a Wayland greeter: the DRM path can.
            if crate::platform::linux::is_login_screen_wayland() && !drm_can_serve_login_screen() {
                msg = crate::client::LOGIN_SCREEN_WAYLAND.to_owned()
            } else {
                let dtype = crate::platform::linux::get_display_server();
                if dtype != crate::platform::linux::DISPLAY_SERVER_X11
                    && dtype != crate::platform::linux::DISPLAY_SERVER_WAYLAND
                {
                    msg = format!(
                        "Unsupported display server type \"{}\", x11 or wayland expected",
                        dtype
                    );
                }
            }
            if !msg.is_empty() {
                res.set_error(msg);
                let mut msg_out = Message::new();
                msg_out.set_login_response(res);
                self.send(msg_out).await;
                return true;
            }
        }
        #[allow(unused_mut)]
        let mut sas_enabled = false;
        #[cfg(windows)]
        if crate::platform::is_root() {
            sas_enabled = true;
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if self.file_transfer.is_some() {
            if crate::platform::is_prelogin() {
                // }|| self.tx_to_cm.send(ipc::Data::Test).is_err() {
                username = "".to_owned();
            }
        }
        // Terminal feature is supported on desktop only
        #[allow(unused_mut)]
        let mut terminal = cfg!(not(any(target_os = "android", target_os = "ios")));
        #[cfg(target_os = "windows")]
        {
            terminal = terminal && portable_pty::win::check_support().is_ok();
        }
        pi.username = username;
        pi.sas_enabled = sas_enabled;
        pi.features = Some(Features {
            quick_launch: cfg!(any(target_os = "windows", target_os = "macos", target_os = "linux")),
            file_transfer_pause: true,
            privacy_mode: privacy_mode::is_privacy_mode_supported(),
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            terminal,
            ..Default::default()
        })
        .into();

        let mut sub_service = false;
        #[allow(unused_mut)]
        let mut wait_session_id_confirm = false;
        #[cfg(windows)]
        if !self.terminal {
            self.handle_windows_specific_session(&mut pi, &mut wait_session_id_confirm);
        }
        if self.file_transfer.is_some() || self.terminal {
            res.set_peer_info(pi);
        } else if self.view_camera {
            let supported_encoding = scrap::codec::Encoder::supported_encoding();
            self.last_supported_encoding = Some(supported_encoding.clone());
            log::info!("peer info supported_encoding: {:?}", supported_encoding);
            pi.encoding = Some(supported_encoding).into();

            pi.displays = camera::Cameras::all_info().unwrap_or(Vec::new());
            pi.current_display = camera::PRIMARY_CAMERA_IDX as _;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            {
                pi.resolutions = Some(SupportedResolutions {
                    resolutions: camera::Cameras::get_camera_resolution(
                        pi.current_display as usize,
                    )
                    .ok()
                    .into_iter()
                    .collect(),
                    ..Default::default()
                })
                .into();
            }
            res.set_peer_info(pi);
            self.update_codec_on_login();
        } else {
            let supported_encoding = scrap::codec::Encoder::supported_encoding();
            self.last_supported_encoding = Some(supported_encoding.clone());
            log::info!("peer info supported_encoding: {:?}", supported_encoding);
            pi.encoding = Some(supported_encoding).into();
            if let Some(msg_out) = super::super::display_service::is_inited_msg() {
                self.send(msg_out).await;
            }

            try_activate_screen();

            match super::super::display_service::update_get_sync_displays_on_login().await {
                Err(err) => {
                    res.set_error(format!("{}", err));
                }
                Ok((displays, primary_display_idx)) => {
                    // For compatibility with old versions, we need to send the displays to the peer.
                    // But the displays may be updated later, before creating the video capturer.
                    #[cfg(target_os = "macos")]
                    {
                        self.retina.set_displays(&displays);
                    }
                    // A separate primary lookup here could race with display hot-plug.
                    self.display_idx = primary_display_idx;
                    pi.displays = displays;
                    pi.current_display = self.display_idx as _;
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    {
                        pi.resolutions = Some(SupportedResolutions {
                            resolutions: pi
                                .displays
                                .get(self.display_idx)
                                .map(|d| crate::platform::resolutions(&d.name))
                                .unwrap_or(vec![]),
                            ..Default::default()
                        })
                        .into();
                    }
                    res.set_peer_info(pi);
                    sub_service = true;

                    #[cfg(target_os = "linux")]
                    {
                        // use rdp_input when uinput is not available in wayland. Ex: flatpak
                        if input_service::wayland_use_rdp_input() {
                            let _ = setup_rdp_input().await;
                        }
                    }
                }
            }
            self.on_remote_authorized();
        }
        let mut msg_out = Message::new();
        msg_out.set_login_response(res);
        self.send(msg_out).await;
        self.update_scoped_login_options().await;
        if let Some((dir, show_hidden)) = self.file_transfer.clone() {
            self.keyboard = false;
            let is_existing_dir = !dir.is_empty() && std::path::Path::new(&dir).is_dir();
            let is_allowed_dir =
                is_existing_dir && crate::common::is_peer_path_allowed(&dir, false);
            #[cfg(target_os = "android")]
            if is_existing_dir && !is_allowed_dir {
                log::warn!(
                    "Use the app workspace because the initial file-transfer directory is outside it: {}",
                    dir
                );
            }
            let dir = if is_allowed_dir { &dir } else { "" };
            if !wait_session_id_confirm {
                self.read_dir(dir, show_hidden);
            } else {
                self.delayed_read_dir = Some((dir.to_owned(), show_hidden));
            }
        } else if self.terminal {
            self.keyboard = false;
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            self.init_terminal_service().await;
        } else if self.view_camera {
            if !wait_session_id_confirm {
                self.try_sub_camera_displays();
            }
            self.keyboard = false;
            self.send_permission(Permission::Keyboard, false).await;
        } else if sub_service {
            if !wait_session_id_confirm {
                self.try_sub_monitor_services();
            }
        }
        true
    }
}
