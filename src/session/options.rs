use super::*;

impl<T: InvokeUiSession> Session<T> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn get_permission_config(&self) -> SessionPermissionConfig {
        SessionPermissionConfig {
            lc: self.lc.clone(),
            server_keyboard_enabled: self.server_keyboard_enabled.clone(),
            server_file_transfer_enabled: self.server_file_transfer_enabled.clone(),
            server_clipboard_enabled: self.server_clipboard_enabled.clone(),
        }
    }

    pub fn is_file_transfer(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::FILE_TRANSFER)
    }

    pub fn is_default(&self) -> bool {
        self.lc
            .read()
            .unwrap()
            .conn_type
            .eq(&ConnType::DEFAULT_CONN)
    }

    pub fn is_view_camera(&self) -> bool {
        self.lc.read().unwrap().conn_type.eq(&ConnType::VIEW_CAMERA)
    }

    pub fn is_terminal(&self) -> bool {
        self.lc.read().unwrap().conn_type.eq(&ConnType::TERMINAL)
    }

    pub fn is_port_forward(&self) -> bool {
        let conn_type = self.lc.read().unwrap().conn_type;
        conn_type == ConnType::PORT_FORWARD || conn_type == ConnType::RDP
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn is_rdp(&self) -> bool {
        self.lc.read().unwrap().conn_type.eq(&ConnType::RDP)
    }

    #[cfg(feature = "flutter")]
    pub fn is_multi_ui_session(&self) -> bool {
        self.ui_handler.is_multi_ui_session()
    }

    pub fn get_view_style(&self) -> String {
        self.lc.read().unwrap().view_style.clone()
    }

    pub fn get_scroll_style(&self) -> String {
        self.lc.read().unwrap().scroll_style.clone()
    }

    pub fn get_edge_scroll_edge_thickness(&self) -> i32 {
        self.lc.read().unwrap().edge_scroll_edge_thickness
    }

    pub fn get_image_quality(&self) -> String {
        self.lc.read().unwrap().image_quality.clone()
    }

    pub fn get_custom_image_quality(&self) -> Vec<i32> {
        self.lc.read().unwrap().custom_image_quality.clone()
    }

    pub fn get_peer_version(&self) -> i64 {
        self.lc.read().unwrap().version.clone()
    }

    pub fn get_trackpad_speed(&self) -> i32 {
        self.lc.read().unwrap().trackpad_speed
    }

    pub fn fallback_keyboard_mode(&self) -> String {
        let peer_version = self.get_peer_version();
        let platform = self.peer_platform();

        let supported_modes = get_supported_keyboard_modes(peer_version, &platform);
        if let Some(mode) = supported_modes.first() {
            return mode.to_string();
        } else {
            if self.get_peer_version() >= get_version_number("1.2.0") {
                return KeyboardMode::Map.to_string();
            } else {
                return KeyboardMode::Legacy.to_string();
            }
        }
    }

    // Caution: This function must be called after peer info is received.
    pub fn get_keyboard_mode(&self) -> String {
        let mode = self.lc.read().unwrap().keyboard_mode.clone();
        let keyboard_mode = KeyboardMode::from_str(&mode);

        // Note: peer_version is 0 before peer info is received.
        let peer_version = self.get_peer_version();
        let platform = self.peer_platform();

        // Saved keyboard mode still exists in this version.
        if let Ok(mode) = keyboard_mode {
            if is_keyboard_mode_supported(&mode, peer_version, &platform) {
                return mode.to_string();
            }
        }
        self.fallback_keyboard_mode()
    }

    pub fn is_keyboard_mode_supported(&self, mode: String) -> bool {
        if let Ok(mode) = KeyboardMode::from_str(&mode[..]) {
            crate::common::is_keyboard_mode_supported(
                &mode,
                self.get_peer_version(),
                &self.peer_platform(),
            )
        } else {
            false
        }
    }

    pub fn save_keyboard_mode(&self, value: String) {
        self.lc.write().unwrap().save_keyboard_mode(value);
    }

    pub fn get_reverse_mouse_wheel(&self) -> String {
        self.lc.read().unwrap().reverse_mouse_wheel.clone()
    }

    pub fn get_displays_as_individual_windows(&self) -> String {
        self.lc
            .read()
            .unwrap()
            .displays_as_individual_windows
            .clone()
    }

    pub fn get_use_all_my_displays_for_the_remote_session(&self) -> String {
        self.lc
            .read()
            .unwrap()
            .use_all_my_displays_for_the_remote_session
            .clone()
    }

    pub fn save_reverse_mouse_wheel(&self, value: String) {
        self.lc.write().unwrap().save_reverse_mouse_wheel(value);
    }

    pub fn save_displays_as_individual_windows(&self, value: String) {
        self.lc
            .write()
            .unwrap()
            .save_displays_as_individual_windows(value);
    }

    pub fn save_use_all_my_displays_for_the_remote_session(&self, value: String) {
        self.lc
            .write()
            .unwrap()
            .save_use_all_my_displays_for_the_remote_session(value);
    }

    pub fn save_view_style(&self, value: String) {
        self.lc.write().unwrap().save_view_style(value);
    }

    pub fn save_scroll_style(&self, value: String) {
        self.lc.write().unwrap().save_scroll_style(value);
    }

    pub fn save_edge_scroll_edge_thickness(&self, value: i32) {
        self.lc
            .write()
            .unwrap()
            .save_edge_scroll_edge_thickness(value);
    }

    pub fn save_flutter_option(&self, k: String, v: String) {
        self.lc.write().unwrap().save_ui_flutter(k, v);
    }

    pub fn get_flutter_option(&self, k: String) -> String {
        self.lc.read().unwrap().get_ui_flutter(&k)
    }

    pub fn toggle_option(&self, name: String) {
        let msg = self.lc.write().unwrap().toggle_option(name.clone());
        if let Some(msg) = msg {
            self.send(Data::Message(msg));
        }
    }

    pub fn toggle_privacy_mode(&self, impl_key: String, on: bool) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        let mut misc = Misc::new();
        misc.set_toggle_privacy_mode(TogglePrivacyMode {
            impl_key,
            on,
            ..Default::default()
        });
        let mut msg_out = Message::new();
        msg_out.set_misc(misc);
        self.send(Data::Message(msg_out));
    }

    pub fn get_toggle_option(&self, name: String) -> bool {
        self.lc.read().unwrap().get_toggle_option(&name)
    }

    #[cfg(not(target_os = "ios"))]
    pub fn is_text_clipboard_required(&self) -> bool {
        *self.server_clipboard_enabled.read().unwrap()
            && *self.server_keyboard_enabled.read().unwrap()
            && !self.lc.read().unwrap().disable_clipboard.v
            && !self.lc.read().unwrap().get_toggle_option("view-only")
    }

    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    pub fn is_file_clipboard_required(&self) -> bool {
        let lc = self.lc.read().unwrap();
        *self.server_keyboard_enabled.read().unwrap()
            && *self.server_file_transfer_enabled.read().unwrap()
            && lc.enable_file_copy_paste.v
            && !lc.get_toggle_option("view-only")
    }
}
