use super::*;

define_windows_service!(ffi_service_main, service_main);

pub(super) fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_service(arguments) {
        log::error!("run_service failed: {}", e);
    }
}

pub fn start_os_service() {
    if let Err(e) =
        windows_service::service_dispatcher::start(crate::get_app_name(), ffi_service_main)
    {
        log::error!("start_service failed: {}", e);
    }
}

pub(super) const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

/// How long the service loop waits for an IPC connection before looking at the sessions
/// again. Windows tells the service when a session changes, so this is a backstop rather
/// than how a change is noticed: a session that appears without an event waits this long.
const SESSION_POLL: u64 = 1_000;

/// End the service loop's wait so it looks at the sessions now. Connecting is enough: the
/// loop is waiting for an IPC connection and any connection ends that wait, so this needs
/// no message of its own and nothing on the loop's side.
///
/// Called from the service control handler, which the SCM runs on its own thread, so this
/// builds a runtime the way the stop path next to it does.
#[tokio::main(flavor = "current_thread")]
pub(super) async fn wake_ipc_loop(postfix: &str) {
    ipc::connect(300, postfix).await.ok();
}

pub fn get_current_session_id(share_rdp: bool) -> DWORD {
    unsafe { get_current_session(if share_rdp { TRUE } else { FALSE }) }
}

#[inline]
pub(super) fn resolve_expected_active_session_id_for_service(session_id: u32) -> Option<u32> {
    let share_rdp_enabled = is_share_rdp();
    if get_available_sessions(false)
        .iter()
        .any(|e| e.sid == session_id)
    {
        return Some(session_id);
    }
    let current_active_session =
        unsafe { get_current_session(if share_rdp_enabled { TRUE } else { FALSE }) };
    if current_active_session == u32::MAX {
        None
    } else {
        Some(current_active_session)
    }
}

#[inline]
pub(super) fn authorize_service_scoped_ipc_connection(
    stream: &ipc::Connection,
    expected_active_session_id: Option<u32>,
) -> bool {
    let (authorized, peer_pid, peer_session_id, peer_is_system) =
        stream.service_authorization_status_for_session(expected_active_session_id);
    if !authorized {
        ipc::log_rejected_windows_ipc_connection(
            crate::POSTFIX_SERVICE,
            peer_pid,
            peer_session_id,
            expected_active_session_id,
            peer_is_system,
            None,
        );
        return false;
    }
    if let Err(err) =
        ipc::ensure_peer_executable_matches_current_by_pid_opt(peer_pid, crate::POSTFIX_SERVICE)
    {
        log::warn!(
                "Rejected unauthorized connection on protected service-scoped IPC channel due to executable mismatch: postfix={}, peer_pid={:?}, err={}",
                crate::POSTFIX_SERVICE,
                peer_pid,
                err
            );
        return false;
    }
    true
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn run_service(_arguments: Vec<OsString>) -> ResultType<()> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        log::info!("Got service control event: {:?}", control_event);
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop | ServiceControl::Preshutdown | ServiceControl::Shutdown => {
                send_close(crate::POSTFIX_SERVICE).ok();
                ServiceControlHandlerResult::NoError
            }
            // A user logged on or off, or a session was connected, disconnected, locked or
            // unlocked: whatever the service loop is running for may now be in the wrong
            // session, and this is how it hears about it rather than by looking three times
            // a second for the rest of the machine's uptime.
            ServiceControl::SessionChange(_) => {
                wake_ipc_loop(crate::POSTFIX_SERVICE);
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler
    let status_handle = service_control_handler::register(crate::get_app_name(), event_handler)?;

    let next_status = ServiceStatus {
        // Should match the one from system service registry
        service_type: SERVICE_TYPE,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SESSION_CHANGE,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
        process_id: None,
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let mut session_id = unsafe { get_current_session(share_rdp()) };
    let mut pinned_session = sessions::PinnedSession::new();
    let mut stored_usid = pinned_session.resolve().flatten();
    if let Some(usid) = stored_usid {
        session_id = usid;
    }
    log::info!("session id {}", session_id);
    let mut h_process = launch_server(session_id, true).await.unwrap_or(NULL);
    let mut incoming = ipc::new_listener(crate::POSTFIX_SERVICE).await?;
    loop {
        let sids: Vec<_> = get_available_sessions(false)
            .iter()
            .map(|e| e.sid)
            .collect();
        if !sids.contains(&session_id) || !is_share_rdp() {
            let current_active_session = unsafe { get_current_session(share_rdp()) };
            if session_id != current_active_session {
                if stored_usid.is_some() {
                    log::warn!(
                        "pinned session {} is gone, following the active session",
                        session_id
                    );
                    stored_usid = None;
                }
                session_id = current_active_session;
                // https://github.com/rustdesk/rustdesk/discussions/10039
                let count = ipc::get_port_forward_session_count(1000).await.unwrap_or(0);
                if count == 0 {
                    h_process = launch_server(session_id, true).await.unwrap_or(NULL);
                }
            }
        }
        let res = timeout(SESSION_POLL, incoming.next()).await;
        match res {
            Ok(res) => match res {
                Some(Ok(stream)) => {
                    let mut stream = ipc::Connection::new(stream);
                    // Keep IPC authorization consistent with the session we are currently serving.
                    // Recompute expected session right before authorization to avoid using a stale
                    // session_id after awaiting incoming.next().
                    let expected_active_session_id =
                        resolve_expected_active_session_id_for_service(session_id);
                    if !authorize_service_scoped_ipc_connection(&stream, expected_active_session_id)
                    {
                        continue;
                    }
                    if let Ok(Some(data)) = stream.next_timeout(1000).await {
                        match data {
                            ipc::Data::Close => {
                                log::info!("close received");
                                break;
                            }
                            ipc::Data::SAS => {
                                send_sas();
                            }
                            ipc::Data::UserSid(usid) => {
                                if let Some(usid) = usid {
                                    if session_id != usid {
                                        log::info!(
                                            "session changed from {} to {}",
                                            session_id,
                                            usid
                                        );
                                        session_id = usid;
                                        stored_usid = Some(session_id);
                                        h_process =
                                            launch_server(session_id, true).await.unwrap_or(NULL);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Err(_) => {
                // timeout
                if let Some(target) = pinned_session.resolve() {
                    if target != stored_usid {
                        if let Some(usid) = target {
                            log::info!("session pinned from {} to {}", session_id, usid);
                            session_id = usid;
                            h_process = launch_server(session_id, true).await.unwrap_or(NULL);
                        } else {
                            log::info!("pinned session released");
                        }
                        stored_usid = target;
                    }
                }
                unsafe {
                    let tmp = get_current_session(share_rdp());
                    if tmp == 0xFFFFFFFF {
                        continue;
                    }
                    let mut close_sent = false;
                    if tmp != session_id && stored_usid != Some(session_id) {
                        log::info!("session changed from {} to {}", session_id, tmp);
                        session_id = tmp;
                        let count = ipc::get_port_forward_session_count(1000).await.unwrap_or(0);
                        if count == 0 {
                            send_close_async("").await.ok();
                            close_sent = true;
                        }
                    }
                    let mut exit_code: DWORD = 0;
                    if h_process.is_null()
                        || (GetExitCodeProcess(h_process, &mut exit_code) == TRUE
                            && exit_code != STILL_ACTIVE
                            && CloseHandle(h_process) == TRUE)
                    {
                        match launch_server(session_id, !close_sent).await {
                            Ok(ptr) => {
                                h_process = ptr;
                            }
                            Err(err) => {
                                log::error!("Failed to launch server: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }

    if !h_process.is_null() {
        send_close_async("").await.ok();
        unsafe { CloseHandle(h_process) };
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

pub(super) async fn launch_server(session_id: DWORD, close_first: bool) -> ResultType<HANDLE> {
    if close_first {
        // in case started some elsewhere
        send_close_async("").await.ok();
    }
    let cmd = format!(
        "\"{}\" --server",
        std::env::current_exe()?.to_str().unwrap_or("")
    );
    launch_privileged_process(session_id, &cmd)
}
