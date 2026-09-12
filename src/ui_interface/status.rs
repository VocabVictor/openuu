use super::*;

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_mouse_time() -> f64 {
    UI_STATUS.lock().unwrap().mouse_time as f64
}

#[inline]
pub fn check_mouse_time() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let sender = SENDER.lock().unwrap();
        allow_err!(sender.send(ipc::Data::MouseMoveTime(0)));
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_connect_status() -> UiStatus {
    UI_STATUS.lock().unwrap().clone()
}

#[inline]
pub fn temporary_password() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return password_security::temporary_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return TEMPORARY_PASSWD.lock().unwrap().clone();
}

#[inline]
pub fn update_temporary_password() {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    password_security::update_temporary_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    allow_err!(ipc::update_temporary_password());
}

#[inline]
pub fn is_permanent_password_set() -> bool {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Config::has_permanent_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let daemon_is_set = ipc::is_permanent_password_set();
        // `daemon_is_set` is authoritative for the return value. Local storage is only used to
        // decide whether we should attempt a sync to clear stale user-side state.
        let local_storage_is_empty = if daemon_is_set {
            true
        } else {
            let (storage, _) = Config::get_local_permanent_password_storage_and_salt();
            storage.is_empty()
        };
        if daemon_is_set || !local_storage_is_empty {
            allow_err!(ipc::sync_permanent_password_storage_from_daemon());
        }
        daemon_is_set
    }
}

#[inline]
pub fn is_local_permanent_password_set() -> bool {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Config::has_local_permanent_password();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        allow_err!(ipc::sync_permanent_password_storage_from_daemon());
        Config::has_local_permanent_password()
    }
}

pub fn set_permanent_password_with_result(password: String) -> bool {
    if config::Config::is_disable_change_permanent_password() {
        return false;
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        return config::Config::set_permanent_password(&password);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        match crate::ipc::set_permanent_password_with_ack(password) {
            Ok(ok) => ok,
            Err(err) => {
                log::warn!("Failed to set permanent password via IPC: {err}");
                false
            }
        }
    }
}

// Make sure `SENDER` is inited here.
#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn start_option_status_sync() {
    let _sender = SENDER.lock().unwrap();
}

// not call directly
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) fn check_connect_status(reconnect: bool) -> mpsc::UnboundedSender<ipc::Data> {
    let (tx, rx) = mpsc::unbounded_channel::<ipc::Data>();
    std::thread::spawn(move || check_connect_status_(reconnect, rx));
    tx
}

// notice: avoiding create ipc connection repeatedly,
// because windows named pipe has serious memory leak issue.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
async fn check_connect_status_(reconnect: bool, rx: mpsc::UnboundedReceiver<ipc::Data>) {
    let mut rx = rx;
    let mut mouse_time = 0;
    #[cfg(feature = "flutter")]
    let mut video_conn_count = 0;
    let is_cm = crate::common::is_cm();

    loop {
        if let Ok(mut c) = ipc::connect(1000, "").await {
            let mut timer = crate::rustdesk_interval(time::interval(time::Duration::from_secs(1)));
            loop {
                tokio::select! {
                    res = c.next() => {
                        match res {
                            Err(err) => {
                                log::error!("ipc connection closed: {}", err);
                                if is_cm {
                                    crate::ui_cm_interface::quit_cm();
                                }
                                break;
                            }
                            #[cfg(not(any(target_os = "android", target_os = "ios")))]
                            Ok(Some(ipc::Data::MouseMoveTime(v))) => {
                                mouse_time = v;
                                UI_STATUS.lock().unwrap().mouse_time = v;
                            }
                            Ok(Some(ipc::Data::Options(Some(v)))) => {
                                *OPTIONS.lock().unwrap() = v;
                                *OPTION_SYNCED.lock().unwrap() = true;
                            }
                            Ok(Some(ipc::Data::Config((name, Some(value))))) => {
                                if name == "id" {
                                } else if name == "temporary-password" {
                                    *TEMPORARY_PASSWD.lock().unwrap() = value;
                                }
                            }
                            #[cfg(feature = "flutter")]
                            Ok(Some(ipc::Data::VideoConnCount(Some(n)))) => {
                                video_conn_count = n;
                            }
                            Ok(Some(ipc::Data::OnlineStatus(Some((mut x, _c))))) => {
                                if x > 0 {
                                    x = 1
                                }
                                *UI_STATUS.lock().unwrap() = UiStatus {
                                    status_num: x as _,
                                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                                    mouse_time,
                                    #[cfg(feature = "flutter")]
                                    video_conn_count,
                                };
                            }
                            Ok(Some(ipc::Data::ControlPermissionsRemoteModify(v))) => {
                                *IS_REMOTE_MODIFY_ENABLED_BY_CONTROL_PERMISSIONS.lock().unwrap() = v;
                            }
                            #[cfg(target_os = "windows")]
                            Ok(Some(ipc::Data::FileTransferEnabledState(v))) => {
                                if let Some(enabled) = v {
                                    let mut lock = IS_FILE_TRANSFER_ENABLED.lock().unwrap();
                                    if *lock != v {
                                        clipboard::ContextSend::enable(enabled);
                                        *lock = v;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Some(data) = rx.recv() => {
                        allow_err!(c.send(&data).await);
                    }
                    _ = timer.tick() => {
                        c.send(&ipc::Data::OnlineStatus(None)).await.ok();
                        c.send(&ipc::Data::Options(None)).await.ok();
                        c.send(&ipc::Data::Config(("id".to_owned(), None))).await.ok();
                        c.send(&ipc::Data::Config(("temporary-password".to_owned(), None))).await.ok();
                        #[cfg(feature = "flutter")]
                        c.send(&ipc::Data::VideoConnCount(None)).await.ok();
                        c.send(&ipc::Data::ControlPermissionsRemoteModify(None)).await.ok();
                        #[cfg(target_os = "windows")]
                        c.send(&ipc::Data::FileTransferEnabledState(None)).await.ok();
                    }
                }
            }
        }
        if !reconnect {
            OPTIONS
                .lock()
                .unwrap()
                .insert("ipc-closed".to_owned(), "Y".to_owned());
            break;
        }
        *UI_STATUS.lock().unwrap() = UiStatus {
            status_num: -1,
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            mouse_time,
            #[cfg(feature = "flutter")]
            video_conn_count,
        };
        sleep(1.).await;
    }
}

#[allow(dead_code)]
pub fn option_synced() -> bool {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        OPTION_SYNCED.lock().unwrap().clone()
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        true
    }
}

#[cfg(any(target_os = "android", feature = "flutter"))]
#[cfg(not(any(target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
pub(crate) async fn send_to_cm(data: &ipc::Data) {
    if let Ok(mut c) = ipc::connect(1000, "_cm").await {
        c.send(data).await.ok();
    }
}
