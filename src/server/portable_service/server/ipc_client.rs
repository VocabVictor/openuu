use super::*;

#[tokio::main(flavor = "current_thread")]
pub(super) async fn run_ipc_client(ipc_token: String) {
    use DataPortableService::*;

    let postfix = IPC_SUFFIX;

    match ipc::connect(1000, postfix).await {
        Ok(mut stream) => {
            if let Err(err) =
                ipc::portable_service_ipc_handshake_as_client(&mut stream, &ipc_token).await
            {
                log::error!("portable service ipc handshake failed: {}", err);
                *EXIT.lock().unwrap() = true;
                return;
            }
            let mut timer =
                crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
            let mut nack = 0;
            loop {
                if *EXIT.lock().unwrap() {
                    log::info!("Portable service EXIT signaled, closing ipc client loop");
                    stream
                        .send(&Data::DataPortableService(WillClose))
                        .await
                        .ok();
                    break;
                }

                tokio::select! {
                    res = stream.next() => {
                        match res {
                            Err(err) => {
                                log::error!(
                                    "ipc{} connection closed: {}",
                                    postfix,
                                    err
                                );
                                break;
                            }
                            Ok(Some(Data::DataPortableService(data))) => match data {
                                Ping => {
                                    allow_err!(
                                        stream
                                            .send(&Data::DataPortableService(Pong))
                                            .await
                                    );
                                }
                                Pong => {
                                    nack = 0;
                                }
                                ConnCount(Some(n)) => {
                                    if n == 0 {
                                        log::info!("Connection count equals 0, exit");
                                        stream.send(&Data::DataPortableService(WillClose)).await.ok();
                                        break;
                                    }
                                }
                                Mouse((v, conn, username, argb, simulate, show_cursor)) => {
                                    if let Ok(evt) = MouseEvent::parse_from_bytes(&v) {
                                        crate::input_service::handle_mouse_(&evt, conn, username, argb, simulate, show_cursor);
                                    }
                                }
                                Pointer((v, conn)) => {
                                    if let Ok(evt) = PointerDeviceEvent::parse_from_bytes(&v) {
                                        crate::input_service::handle_pointer_(&evt, conn);
                                    }
                                }
                                Key(v) => {
                                    if let Ok(evt) = KeyEvent::parse_from_bytes(&v) {
                                        crate::input_service::handle_key_(&evt);
                                    }
                                }
                                _ => {}
                            },
                            _ => {}
                        }
                    }
                    _ = timer.tick() => {
                        nack+=1;
                        if nack > MAX_NACK {
                            log::info!("max ping nack, exit");
                            break;
                        }
                        stream.send(&Data::DataPortableService(Ping)).await.ok();
                        stream.send(&Data::DataPortableService(ConnCount(None))).await.ok();
                    }
                }
            }
        }
        Err(e) => {
            log::error!("Failed to connect portable service ipc: {:?}", e);
        }
    }

    *EXIT.lock().unwrap() = true;
}
