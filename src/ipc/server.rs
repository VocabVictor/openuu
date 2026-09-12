use super::*;

#[tokio::main(flavor = "current_thread")]
pub async fn start(postfix: &str) -> ResultType<()> {
    let mut incoming = new_listener(postfix).await?;
    loop {
        if let Some(result) = incoming.next().await {
            match result {
                Ok(stream) => {
                    let mut stream = Connection::new(stream);
                    let postfix = postfix.to_owned();
                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                    if config::is_service_ipc_postfix(&postfix) {
                        if !authorize_service_scoped_ipc_connection(&stream, &postfix) {
                            continue;
                        }
                    }
                    #[cfg(windows)]
                    if postfix.is_empty() {
                        // Windows main IPC (`postfix == ""`) is authorized here.
                        // Other security-sensitive channels use dedicated authorization paths:
                        // - `_portable_service`: portable-service listener + handshake policy
                        // - service-scoped postfixes: service-specific listener/authorization
                        if !authorize_windows_main_ipc_connection(&stream, &postfix) {
                            continue;
                        }
                    }
                    tokio::spawn(async move {
                        loop {
                            match stream.next().await {
                                Err(err) => {
                                    log::trace!("ipc '{}' connection closed: {}", postfix, err);
                                    break;
                                }
                                Ok(Some(data)) => {
                                    // On Linux/macOS, the protected `_service` channel is used only for
                                    // syncing config between root service and the active user process.
                                    //
                                    // NOTE: `is_service_ipc_postfix()` also includes `_uinput_*`, but those
                                    // channels are handled by the dedicated uinput listener/protocol in
                                    // `src/server/uinput.rs` and therefore do not share this Data enum
                                    // allowlist. The SyncConfig allowlist here is intentionally scoped to the
                                    // `_service` channel only.
                                    //
                                    // Keep this explicit branch to avoid policy drift between `_service` and
                                    // uinput IPC paths while still minimizing exposed message surface here.
                                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                                    if postfix == crate::POSTFIX_SERVICE {
                                        if matches!(&data, Data::SyncConfig(_)) {
                                            handle(data, &mut stream).await;
                                        } else {
                                            log::warn!(
                                                "Rejected non-sync data on protected _service IPC channel: postfix={}, data_kind={:?}, peer_uid={:?}",
                                                postfix,
                                                std::mem::discriminant(&data),
                                                stream.peer_uid()
                                            );
                                            // Close the connection to avoid keeping a protected channel
                                            // alive while repeatedly receiving invalid traffic.
                                            break;
                                        }
                                        continue;
                                    }
                                    handle(data, &mut stream).await;
                                }
                                Ok(None) => {
                                    // `Ok(None)` means a complete frame arrived but did not
                                    // deserialize into `Data`. Peer close/reset is returned as
                                    // `Err` by `ConnectionTmpl::next()`. Keep the historical
                                    // ignore behavior except on the protected `_service` channel.
                                    #[cfg(any(target_os = "linux", target_os = "macos"))]
                                    {
                                        if postfix == crate::POSTFIX_SERVICE {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                Err(err) => {
                    log::error!("Couldn't get client: {:?}", err);
                }
            }
        }
    }
}

pub async fn new_listener(postfix: &str) -> ResultType<Incoming> {
    let path = Config::ipc_path(postfix);
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let should_scrub_parent_entries = ensure_secure_ipc_parent_dir(&path, postfix)?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let existing_listener_alive = check_pid(postfix).await;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if should_scrub_parent_entries_after_check_pid(
        should_scrub_parent_entries,
        existing_listener_alive,
    ) {
        scrub_secure_ipc_parent_dir(&path, postfix)?;
    }
    let mut endpoint = Endpoint::new(path.clone());
    let security_attrs = {
        #[cfg(windows)]
        {
            if postfix == "_portable_service" {
                portable_service_listener_security_attributes()
            } else if should_allow_everyone_create_on_windows(postfix) {
                SecurityAttributes::allow_everyone_create()
            } else {
                Ok(SecurityAttributes::empty())
            }
        }
        #[cfg(not(windows))]
        {
            SecurityAttributes::allow_everyone_create()
        }
    };
    match security_attrs {
        Ok(attr) => endpoint.set_security_attributes(attr),
        Err(err) => {
            log::error!("Failed to set ipc{} security: {}", postfix, err);
            #[cfg(windows)]
            if postfix == "_portable_service" {
                // Fail closed for `_portable_service` when SDDL construction fails.
                // This endpoint is security-critical and must not start with default ACLs.
                return Err(err.into());
            }
        }
    };
    match endpoint.incoming() {
        Ok(incoming) => {
            if postfix == crate::POSTFIX_SERVICE {
                log::info!("Started protected ipc service server: postfix={}", postfix);
            } else {
                log::info!("Started ipc{} server at path: {}", postfix, &path);
            }
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            {
                // NOTE: On Linux/macOS, some IPC sockets are intentionally world-connectable
                // (0666) so the active (non-root) user process can connect. Authorization is
                // enforced at accept-time for these channels, and the protected `_service`
                // channel is further restricted by an explicit message allowlist (SyncConfig
                // only).
                let socket_mode = if config::is_service_ipc_postfix(postfix) {
                    0o0666
                } else {
                    0o0600
                };
                if let Err(err) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(socket_mode))
                {
                    log::error!(
                        "Failed to set permissions on ipc{} socket at path {}: {}",
                        postfix,
                        &path,
                        err
                    );
                    std::fs::remove_file(&path).ok();
                    return Err(err.into());
                }
                write_pid(postfix);
            }
            Ok(incoming)
        }
        Err(err) => {
            log::error!(
                "Failed to start ipc{} server at path {}: {}",
                postfix,
                path,
                err
            );
            Err(err.into())
        }
    }
}

pub struct CheckIfRestart {
    stop_service: String,
    rendezvous_servers: Vec<String>,
    audio_input: String,
    voice_call_input: String,
    ws: String,
    disable_udp: String,
    allow_insecure_tls_fallback: String,
    api_server: String,
}

impl CheckIfRestart {
    pub fn new() -> CheckIfRestart {
        CheckIfRestart {
            stop_service: Config::get_option("stop-service"),
            rendezvous_servers: Config::get_rendezvous_servers(),
            audio_input: Config::get_option("audio-input"),
            voice_call_input: Config::get_option("voice-call-input"),
            ws: Config::get_option(OPTION_ALLOW_WEBSOCKET),
            disable_udp: Config::get_option(keys::OPTION_DISABLE_UDP),
            allow_insecure_tls_fallback: Config::get_option(
                keys::OPTION_ALLOW_INSECURE_TLS_FALLBACK,
            ),
            api_server: Config::get_option("api-server"),
        }
    }
}

impl Drop for CheckIfRestart {
    fn drop(&mut self) {
        // If https proxy is used, we need to restart rendezvous mediator.
        // No need to check if https proxy is used, because this option does not change frequently
        // and restarting mediator is safe even https proxy is not used.
        let allow_insecure_tls_fallback_changed = self.allow_insecure_tls_fallback
            != Config::get_option(keys::OPTION_ALLOW_INSECURE_TLS_FALLBACK);
        if allow_insecure_tls_fallback_changed
            || self.stop_service != Config::get_option("stop-service")
            || self.rendezvous_servers != Config::get_rendezvous_servers()
            || self.ws != Config::get_option(OPTION_ALLOW_WEBSOCKET)
            || self.disable_udp != Config::get_option(keys::OPTION_DISABLE_UDP)
            || self.api_server != Config::get_option("api-server")
        {
            if allow_insecure_tls_fallback_changed {
                hbb_common::tls::reset_tls_cache();
            }
            RendezvousMediator::restart();
        }
        if self.audio_input != Config::get_option("audio-input") {
            crate::audio_service::restart();
        }
        if self.voice_call_input != Config::get_option("voice-call-input") {
            crate::audio_service::set_voice_call_input_device(
                Some(Config::get_option("voice-call-input")),
                true,
            )
        }
    }
}
