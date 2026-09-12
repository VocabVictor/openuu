use super::*;

pub(super) fn start_ipc_server() -> mpsc::UnboundedSender<Data> {
    let (tx, rx) = mpsc::unbounded_channel::<Data>();
    std::thread::spawn(move || start_ipc_server_async(rx));
    tx
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn start_ipc_server_async(rx: mpsc::UnboundedReceiver<Data>) {
    use DataPortableService::*;
    let rx = Arc::new(tokio::sync::Mutex::new(rx));
    let postfix = IPC_SUFFIX;
    let quick_support = QUICK_SUPPORT.lock().unwrap().clone();

    match new_listener(postfix).await {
        Ok(mut incoming) => loop {
            {
                tokio::select! {
                    Some(result) = incoming.next() => {
                        match result {
                            Ok(stream) => {
                                let mut stream = Connection::new(stream);
                                if !ipc::authorize_windows_portable_service_ipc_connection(
                                    &stream, postfix,
                                ) {
                                    continue;
                                }
                                let mut consumed_token: Option<String> = None;
                                let mut consumed_token_shmem_name: Option<String> = None;
                                let handshake_result =
                                    ipc::portable_service_ipc_handshake_as_server(
                                        &mut stream,
                                        |token| {
                                            let (matched, matched_shmem_name) =
                                                consume_runtime_ipc_token_if_match(token);
                                            if matched {
                                                consumed_token = Some(token.to_owned());
                                                consumed_token_shmem_name = matched_shmem_name;
                                                true
                                            } else {
                                                false
                                            }
                                        },
                                    )
                                    .await;
                                if let Err(err) = handshake_result {
                                    if let Some(token) = consumed_token.as_deref() {
                                        restore_runtime_ipc_token_after_failed_handshake(
                                            token,
                                            consumed_token_shmem_name.as_deref(),
                                        );
                                        *STARTING.lock().unwrap() = false;
                                    }
                                    log::warn!(
                                        "Rejected portable service ipc connection due to token handshake failure: postfix={}, err={}",
                                        postfix,
                                        err
                                    );
                                    continue;
                                }
                                log::info!("Got portable service ipc connection");
                                let rx_clone = rx.clone();
                                tokio::spawn(async move {
                                    let mut stream = stream;
                                    let postfix = postfix.to_owned();
                                    let mut timer = crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
                                    let mut nack = 0;
                                    let mut rx = rx_clone.lock().await;
                                    loop {
                                        tokio::select! {
                                            res = stream.next() => {
                                                match res {
                                                    Err(err) => {
                                                        log::info!(
                                                            "ipc{} connection closed: {}",
                                                            postfix,
                                                            err
                                                        );
                                                        break;
                                                    }
                                                    Ok(Some(Data::DataPortableService(data))) => match data {
                                                        Ping => {
                                                            stream.send(&Data::DataPortableService(Pong)).await.ok();
                                                        }
                                                        Pong => {
                                                            nack = 0;
                                                            *RUNNING.lock().unwrap() = true;
                                                            *STARTING.lock().unwrap() = false;
                                                        },
                                                        ConnCount(None) => {
                                                            if !quick_support {
                                                                let remote_count = crate::server::AUTHED_CONNS
                                                                    .lock()
                                                                    .unwrap()
                                                                    .iter()
                                                                    .filter(|c| c.conn_type == crate::server::AuthConnType::Remote)
                                                                    .count();
                                                                stream.send(&Data::DataPortableService(ConnCount(Some(remote_count)))).await.ok();
                                                            }
                                                        },
                                                        WillClose => {
                                                            log::info!("portable service will close");
                                                            break;
                                                        }
                                                        _=>{}
                                                    }
                                                    _=>{}
                                                }
                                            }
                                            _ = timer.tick() => {
                                                nack+=1;
                                                if nack > MAX_NACK {
                                                    // In fact, this will not happen, ipc will be closed before max nack.
                                                    log::error!("max ipc nack");
                                                    break;
                                                }
                                                stream.send(&Data::DataPortableService(Ping)).await.ok();
                                            }
                                            Some(data) = rx.recv() => {
                                                allow_err!(stream.send(&data).await);
                                            }
                                        }
                                    }
                                    *RUNNING.lock().unwrap() = false;
                                    *STARTING.lock().unwrap() = false;
                                });
                            }
                            Err(err) => {
                                log::error!("Couldn't get portable client: {:?}", err);
                            }
                        }
                    }
                }
            }
        },
        Err(err) => {
            log::error!("Failed to start portable service ipc server: {}", err);
        }
    }
}

pub(super) fn ipc_send(data: Data) -> ResultType<()> {
    let sender = SENDER.lock().unwrap();
    sender
        .send(data)
        .map_err(|e| anyhow!("ipc send error:{:?}", e))
}
