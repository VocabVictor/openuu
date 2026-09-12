use super::*;

#[derive(Copy, Clone)]
pub(super) struct LoginErrorMsgBox {
    msgtype: &'static str,
    title: &'static str,
    text: &'static str,
    link: &'static str,
    try_again: bool,
}

lazy_static::lazy_static! {
    static ref LOGIN_ERROR_MAP: Arc<HashMap<&'static str, LoginErrorMsgBox>> = {
        let map = HashMap::from([(LOGIN_SCREEN_WAYLAND, LoginErrorMsgBox{
            msgtype: "error",
            title: "Login Error",
            text: "Login screen using Wayland is not supported",
            link: "https://rustdesk.com/docs/en/manual/linux/#login-screen",
            try_again: true,
        }), (LOGIN_MSG_NO_PASSWORD_ACCESS, LoginErrorMsgBox{
            msgtype: "wait-remote-accept-nook",
            title: "Prompt",
            text: "Please wait for the remote side to accept your session request...",
            link: "",
            try_again: true,
        })]);
        Arc::new(map)
    };
}

/// Handle login error.
/// Return true if the password is wrong, return false if there's an actual error.
pub fn handle_login_error(
    lc: Arc<RwLock<LoginConfigHandler>>,
    err: &str,
    interface: &impl Interface,
) -> bool {
    if err == LOGIN_MSG_PASSWORD_EMPTY {
        lc.write().unwrap().password = Default::default();
        interface.msgbox("input-password", "Password Required", "", "");
        true
    } else if err == LOGIN_MSG_PASSWORD_WRONG {
        lc.write().unwrap().password = Default::default();
        interface.msgbox("re-input-password", err, "Do you want to enter again?", "");
        true
    } else if err == LOGIN_MSG_2FA_WRONG || err == REQUIRE_2FA {
        let enabled = lc.read().unwrap().get_option("trust-this-device") == "Y";
        if enabled {
            lc.write()
                .unwrap()
                .set_option("trust-this-device".to_string(), "".to_string());
        }
        interface.msgbox("input-2fa", err, "", "");
        true
    } else if LOGIN_ERROR_MAP.contains_key(err) {
        if let Some(msgbox_info) = LOGIN_ERROR_MAP.get(err) {
            interface.msgbox(
                msgbox_info.msgtype,
                msgbox_info.title,
                msgbox_info.text,
                msgbox_info.link,
            );
            msgbox_info.try_again
        } else {
            // unreachable!
            false
        }
    } else {
        if err.contains(SCRAP_X11_REQUIRED) {
            interface.msgbox("error", "Login Error", err, SCRAP_X11_REF_URL);
        } else {
            interface.msgbox("error", "Login Error", err, "");
        }
        false
    }
}

// "Switch sides" requires the incoming-only client to connect back to its
// controlling peer; verify the local pending uuid before opening the connection.
#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) async fn is_switch_sides_back(conn_type: ConnType, interface: &impl Interface) -> bool {
    if conn_type != ConnType::DEFAULT_CONN {
        return false;
    }
    let (id, uuid) = {
        let lch = interface.get_lch();
        let lc = lch.read().unwrap();
        let Some(uuid) = lc.switch_uuid.as_deref() else {
            return false;
        };
        let Ok(uuid) = Uuid::parse_str(uuid) else {
            return false;
        };
        (lc.id.clone(), uuid)
    };
    if !request_local_switch_sides_uuid(
        &id,
        &uuid,
        crate::ipc::SwitchSidesUuidAction::Check,
    )
    .await
    {
        return false;
    }
    let lch = interface.get_lch();
    let lc = lch.read().unwrap();
    let current_uuid = lc
        .switch_uuid
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    lc.id == id && current_uuid.as_ref() == Some(&uuid)
}

#[cfg(not(all(feature = "flutter", not(any(target_os = "android", target_os = "ios")))))]
pub(super) async fn is_switch_sides_back(_conn_type: ConnType, _interface: &impl Interface) -> bool {
    false
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) async fn request_local_switch_sides_uuid(
    id: &str,
    uuid: &Uuid,
    action: crate::ipc::SwitchSidesUuidAction,
) -> bool {
    let Ok(mut conn) = crate::ipc::connect(1000, "").await else {
        return false;
    };
    let uuid = uuid.to_string();
    if conn
        .send(&crate::ipc::Data::SwitchSidesUuid(
            uuid.clone(),
            id.to_owned(),
            action,
            None,
        ))
        .await
        .is_err()
    {
        return false;
    }
    match conn.next_timeout(1000).await {
        Ok(Some(crate::ipc::Data::SwitchSidesUuid(
            returned_uuid,
            returned_id,
            returned_action,
            Some(true),
        ))) => {
            returned_uuid == uuid && returned_id == id && returned_action == action
        }
        _ => false,
    }
}
