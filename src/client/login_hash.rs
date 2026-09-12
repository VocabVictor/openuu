use super::*;

/// Handle hash message sent by peer.
/// Hash will be used for login.
///
/// # Arguments
///
/// * `lc` - Login config.
/// * `hash` - Hash sent by peer.
/// * `interface` - [`Interface`] for sending data.
/// * `peer` - [`Stream`] for communicating with peer.
pub async fn handle_hash(
    lc: Arc<RwLock<LoginConfigHandler>>,
    password_preset: &str,
    hash: Hash,
    interface: &impl Interface,
    peer: &mut Stream,
) -> bool {
    lc.write().unwrap().hash = hash.clone();
    // Take care of password application order

    // switch_uuid
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let uuid = lc.write().unwrap().switch_uuid.take();
        if let Some(uuid) = uuid {
            if let Ok(uuid) = uuid::Uuid::from_str(&uuid) {
                let id = lc.read().unwrap().id.clone();
                if !request_local_switch_sides_uuid(
                    &id,
                    &uuid,
                    crate::ipc::SwitchSidesUuidAction::Consume,
                )
                .await
                {
                    log::warn!("Ignored untrusted switch_uuid");
                } else {
                    lc.write().unwrap().allow_switch_back_once();
                    send_switch_login_request(lc.clone(), peer, uuid).await;
                    lc.write().unwrap().password_source = Default::default();
                    return true;
                }
            }
        }
        // Incoming-only may connect out solely for a verified switch-back;
        // never fall through to password login, including on repeated hashes.
        if config::is_incoming_only() {
            interface.msgbox("error", "Connection Error", "Incoming only mode", "");
            let mut misc = Misc::new();
            misc.set_close_reason(
                "Connection not allowed in incoming-only mode".to_owned(),
            );
            let mut msg = Message::new();
            msg.set_misc(misc);
            allow_err!(peer.send(&msg).await);
            return false;
        }
    }
    // last password
    let mut password = lc.read().unwrap().password.clone();
    // preset password
    if password.is_empty() {
        if !password_preset.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(password_preset);
            hasher.update(&hash.salt);
            let res = hasher.finalize();
            password = res[..].into();
            lc.write().unwrap().password_source = Default::default();
        }
    }
    // shared password
    // Currently it's used only when click shared ab peer card
    let shared_password = lc.write().unwrap().shared_password.take();
    if let Some(shared_password) = shared_password {
        if !shared_password.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(shared_password.clone());
            hasher.update(&hash.salt);
            let res = hasher.finalize();
            password = res[..].into();
            lc.write().unwrap().password_source = PasswordSource::SharedAb(shared_password);
        }
    }
    // peer config password
    if password.is_empty() {
        password = lc.read().unwrap().config.password.clone();
        if !password.is_empty() {
            lc.write().unwrap().password_source = Default::default();
        }
    }
    // personal ab password
    if password.is_empty() {
        try_get_password_from_personal_ab(lc.clone(), &mut password);
    }

    if password.is_empty() {
        let p = crate::ui_interface::get_builtin_option(keys::OPTION_DEFAULT_CONNECT_PASSWORD);
        if !p.is_empty() {
            let mut hasher = Sha256::new();
            hasher.update(p.clone());
            hasher.update(&hash.salt);
            let res = hasher.finalize();
            password = res[..].into();
            lc.write().unwrap().password_source = PasswordSource::SharedAb(p); // reuse SharedAb here
        }
    }

    lc.write().unwrap().password = password.clone();

    let is_terminal_admin = lc.read().unwrap().is_terminal_admin;
    let is_terminal = lc.read().unwrap().conn_type.eq(&ConnType::TERMINAL);
    if is_terminal && is_terminal_admin {
        if password.is_empty() {
            interface.msgbox("terminal-admin-login-password", "", "", "");
        } else {
            interface.msgbox("terminal-admin-login", "", "", "");
        }
        lc.write().unwrap().hash = hash;
        return true;
    }

    let password = if password.is_empty() {
        // login without password, the remote side can click accept
        interface.msgbox("input-password", "Password Required", "", "");
        Vec::new()
    } else {
        let mut hasher = Sha256::new();
        hasher.update(&password);
        hasher.update(&hash.challenge);
        hasher.finalize()[..].into()
    };

    send_login(lc.clone(), String::new(), String::new(), password, peer).await;
    lc.write().unwrap().hash = hash;
    true
}

#[inline]
pub(super) fn try_get_password_from_personal_ab(lc: Arc<RwLock<LoginConfigHandler>>, password: &mut Vec<u8>) {
    let access_token = LocalConfig::get_option("access_token");
    let ab = config::Ab::load();
    if !access_token.is_empty() && access_token == ab.access_token {
        let id = lc.read().unwrap().id.clone();
        if let Some(ab) = ab.ab_entries.iter().find(|a| a.personal()) {
            if let Some(p) = ab
                .peers
                .iter()
                .find_map(|p| if p.id == id { Some(p) } else { None })
            {
                if let Ok(hash_password) = base64::decode(p.hash.clone(), base64::Variant::Original)
                {
                    if !hash_password.is_empty() {
                        *password = hash_password.clone();
                        lc.write().unwrap().password_source =
                            PasswordSource::PersonalAb(hash_password);
                    }
                }
            }
        }
    }
}

/// Send login message to peer.
///
/// # Arguments
///
/// * `lc` - Login config.
/// * `os_username` - OS username.
/// * `os_password` - OS password.
/// * `password` - Password.
/// * `peer` - [`Stream`] for communicating with peer.
pub(super) async fn send_login(
    lc: Arc<RwLock<LoginConfigHandler>>,
    os_username: String,
    os_password: String,
    password: Vec<u8>,
    peer: &mut Stream,
) {
    let msg_out = lc
        .read()
        .unwrap()
        .create_login_msg(os_username, os_password, password);
    allow_err!(peer.send(&msg_out).await);
}

/// Handle login request made from ui.
///
/// # Arguments
///
/// * `lc` - Login config.
/// * `os_username` - OS username.
/// * `os_password` - OS password.
/// * `password` - Password.
/// * `remember` - Whether to remember password.
/// * `peer` - [`Stream`] for communicating with peer.
pub async fn handle_login_from_ui(
    lc: Arc<RwLock<LoginConfigHandler>>,
    os_username: String,
    os_password: String,
    password: String,
    remember: bool,
    peer: &mut Stream,
) {
    let mut hash_password = if password.is_empty() {
        let mut password2 = lc.read().unwrap().password.clone();
        if password2.is_empty() {
            password2 = lc.read().unwrap().config.password.clone();
            if !password2.is_empty() {
                lc.write().unwrap().password_source = Default::default();
            }
        }
        password2
    } else {
        lc.write().unwrap().password_source = Default::default();
        let mut hasher = Sha256::new();
        hasher.update(password);
        hasher.update(&lc.read().unwrap().hash.salt);
        let res = hasher.finalize();
        lc.write().unwrap().remember = remember;
        res[..].into()
    };
    lc.write().unwrap().password = hash_password.clone();
    let mut hasher2 = Sha256::new();
    hasher2.update(&hash_password[..]);
    hasher2.update(&lc.read().unwrap().hash.challenge);
    hash_password = hasher2.finalize()[..].to_vec();

    send_login(lc.clone(), os_username, os_password, hash_password, peer).await;
}

pub(super) async fn send_switch_login_request(
    lc: Arc<RwLock<LoginConfigHandler>>,
    peer: &mut Stream,
    uuid: Uuid,
) {
    let mut msg_out = Message::new();
    msg_out.set_switch_sides_response(SwitchSidesResponse {
        uuid: Bytes::from(uuid.as_bytes().to_vec()),
        lr: hbb_common::protobuf::MessageField::some(
            lc.read()
                .unwrap()
                .create_login_msg("".to_owned(), "".to_owned(), vec![])
                .login_request()
                .to_owned(),
        ),
        ..Default::default()
    });
    allow_err!(peer.send(&msg_out).await);
}
