use super::*;

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn check_click_time(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::ClickTime(0)));
    };
}

#[inline]
pub fn get_click_time() -> i64 {
    CLICK_TIME.load(Ordering::SeqCst)
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn authorize(id: i32) {
    if let Some(client) = CLIENTS.write().unwrap().get_mut(&id) {
        client.authorized = true;
        allow_err!(client.tx.send(Data::Authorize));
    };
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn close(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::Close));
    };
}

/// Like `close`, but says the CM's WINDOW closed rather than a person disconnecting this peer.
/// See `ipc::Data::CmWindowClosed`.
#[cfg(target_os = "linux")]
pub fn close_window(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::CmWindowClosed));
    };
}

#[inline]
pub fn remove(id: i32) {
    CLIENTS.write().unwrap().remove(&id);
}

// server mode send chat to peer
#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn send_chat(id: i32, text: String) {
    let clients = CLIENTS.read().unwrap();
    if let Some(client) = clients.get(&id) {
        allow_err!(client.tx.send(Data::ChatMessage { text }));
    }
}

#[inline]
#[cfg(not(any(target_os = "ios")))]
pub fn switch_permission(id: i32, name: String, enabled: bool) {
    #[cfg(target_os = "android")]
    let is_keyboard_permission = name == "keyboard";
    #[cfg(not(target_os = "android"))]
    let is_keyboard_permission = false;
    if !option2bool(
        OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW,
        &crate::get_builtin_option(OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW),
    ) && !is_keyboard_permission
    {
        log::info!(
            "blocked cm switch_permission by policy, conn_id={}, permission={}, enabled={}",
            id,
            name,
            enabled
        );
        return;
    }
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::SwitchPermission { name, enabled }));
    };
}

#[inline]
#[cfg(target_os = "android")]
pub fn switch_permission_all(name: String, enabled: bool) {
    if name != "keyboard"
        && !option2bool(
            OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW,
            &crate::get_builtin_option(OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW),
        )
    {
        log::info!(
            "blocked cm switch_permission_all by policy, permission={}, enabled={}",
            name,
            enabled
        );
        return;
    }
    for (_, client) in CLIENTS.read().unwrap().iter() {
        allow_err!(client.tx.send(Data::SwitchPermission {
            name: name.clone(),
            enabled
        }));
    }
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn get_clients_state() -> String {
    let clients = CLIENTS.read().unwrap();
    let res = Vec::from_iter(clients.values().cloned());
    serde_json::to_string(&res).unwrap_or("".into())
}

#[inline]
pub fn get_clients_length() -> usize {
    let clients = CLIENTS.read().unwrap();
    clients.len()
}

#[inline]
#[cfg(target_os = "android")]
pub fn has_active_clients() -> bool {
    let clients = CLIENTS.read().unwrap();
    clients.values().any(|c| !c.disconnected)
}

#[inline]
#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn switch_back(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        allow_err!(client.tx.send(Data::SwitchSidesBack));
    };
}

#[cfg(windows)]
pub(super) fn cm_inner_send(id: i32, data: Data) {
    let lock = CLIENTS.read().unwrap();
    if id != 0 {
        if let Some(s) = lock.get(&id) {
            allow_err!(s.tx.send(data));
        }
    } else {
        for s in lock.values() {
            allow_err!(s.tx.send(data.clone()));
        }
    }
}

pub fn can_elevate() -> bool {
    #[cfg(windows)]
    return !crate::platform::is_installed();
    #[cfg(not(windows))]
    return false;
}

pub fn elevate_portable(_id: i32) {
    #[cfg(windows)]
    {
        let lock = CLIENTS.read().unwrap();
        if let Some(s) = lock.get(&_id) {
            allow_err!(s.tx.send(ipc::Data::DataPortableService(
                ipc::DataPortableService::RequestStart
            )));
        }
    }
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn handle_incoming_voice_call(id: i32, accept: bool) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        // Not handled in iOS yet.
        #[cfg(not(any(target_os = "ios")))]
        allow_err!(client.tx.send(Data::VoiceCallResponse(accept)));
    };
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn close_voice_call(id: i32) {
    if let Some(client) = CLIENTS.read().unwrap().get(&id) {
        // Not handled in iOS yet.
        #[cfg(not(any(target_os = "ios")))]
        allow_err!(client.tx.send(Data::CloseVoiceCall("".to_owned())));
    };
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn quit_cm() {
    // in case of std::process::exit not work
    log::info!("quit cm");
    CLIENTS.write().unwrap().clear();
    // `quit_gui()` ends the process on Windows and macOS, but on Linux it calls
    // `gtk_main_quit()`, which has no effect in the Flutter connection manager:
    // `flutter/linux/main.cc` runs `g_application_run()` (GtkApplication), so
    // `gtk_main()` is never called. Exit directly instead, otherwise this
    // process keeps running while no longer serving the `_cm` ipc endpoint, so
    // the server can't reuse it and spawns one more connection manager.
    //
    // NOTE: a client merely disconnecting does not come here, the Flutter side
    // closes the window then, so this is a fallback rather than an explanation
    // for the stale processes of #15698.
    #[cfg(all(target_os = "linux", feature = "flutter"))]
    std::process::exit(0);
    #[cfg(not(all(target_os = "linux", feature = "flutter")))]
    crate::platform::quit_gui();
}
