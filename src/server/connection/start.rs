use super::*;

impl Connection {
    pub async fn start(
        addr: SocketAddr,
        stream: super::super::Stream,
        id: i32,
        server: super::super::ServerPtrWeak,
        meta: super::super::ConnectionMeta,
    ) {
        let super::super::ConnectionMeta {
            control_permissions,
            controlled_context,
        } = meta;
        // Android is not supported yet, so we always set control_permissions to None.
        #[cfg(target_os = "android")]
        let control_permissions = None;
        let _raii_id = raii::ConnectionID::new(id);
        let _raii_control_permissions_id =
            raii::ControlPermissionsID::new(id, &control_permissions);
        let (mut conn, mut ch) =
            Self::build(stream, id, server, control_permissions, controlled_context);
        let mut hbbs_rx = crate::hbbs_http::sync::signal_receiver();
        let addr = hbb_common::try_into_v4(addr);
        if !conn.on_open(addr).await {
            conn.closed = true;
            // sleep to ensure msg got received.
            sleep(1.).await;
            return;
        }
        #[cfg(target_os = "android")]
        start_channel(ch.rx_to_cm, ch.tx_from_cm);
        conn.send_denied_permissions().await;
        let mut test_delay_timer =
            crate::rustdesk_interval(time::interval_at(Instant::now(), TEST_DELAY_TIMEOUT));
        let mut last_recv_time = Instant::now();

        // The connection type is not known until the login request arrives;
        // `on_message` picks the type-specific timeout then.
        conn.stream.set_send_timeout(SEND_TIMEOUT_VIDEO);

        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        std::thread::spawn(move || Self::handle_input(ch.rx_input, ch.tx_cloned));
        let mut second_timer = crate::rustdesk_interval(time::interval(Duration::from_secs(1)));

        #[cfg(feature = "unix-file-copy-paste")]
        let rx_clip_holder;
        let mut rx_clip;
        let _tx_clip: mpsc::UnboundedSender<i32>;
        #[cfg(feature = "unix-file-copy-paste")]
        {
            rx_clip_holder = (
                clipboard::get_rx_cliprdr_server(id),
                crate::SimpleCallOnReturn {
                    b: true,
                    f: Box::new(move || {
                        clipboard::remove_channel_by_conn_id(id);
                    }),
                },
            );
            rx_clip = rx_clip_holder.0.lock().await;
        }
        #[cfg(not(feature = "unix-file-copy-paste"))]
        {
            (_tx_clip, rx_clip) = mpsc::unbounded_channel::<i32>();
        }

        loop {
            tokio::select! {
                // biased; // video has higher priority // causing test_delay_timer failed while transferring big file

                Some(data) = ch.rx_from_cm.recv() => {
                    if !conn.handle_cm_data(data).await {
                        break;
                    }
                },
                res = conn.stream.next() => {
                    if let Some(res) = res {
                        match res {
                            Err(err) => {
                                conn.on_close(&err.to_string(), true).await;
                                break;
                            },
                            Ok(bytes) => {
                                last_recv_time = Instant::now();
                                conn.session_last_recv_time.as_mut().map(|t| *t.lock().unwrap() = Instant::now());
                                if let Ok(msg_in) = Message::parse_from_bytes(&bytes) {
                                    if !conn.on_message(msg_in).await {
                                        break;
                                    }
                                    if conn.port_forward_socket.is_some() && conn.authorized {
                                        log::info!("Port forward, last_test_delay is none: {}", conn.last_test_delay.is_none());
                                        // Avoid TestDelay reply injection into rdp data stream
                                        if conn.last_test_delay.is_none() {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        conn.on_close("Reset by the peer", true).await;
                        break;
                    }
                },
                _ = conn.file_timer.tick() => {
                    if !conn.read_jobs.is_empty() {
                        conn.send_to_cm(ipc::Data::FileTransferLog(("transfer".to_string(), fs::serialize_transfer_jobs(&conn.read_jobs))));
                        match conn.handle_file_read_jobs().await {
                            Ok(log) => {
                                if !log.is_empty() {
                                    conn.send_to_cm(ipc::Data::FileTransferLog(("transfer".to_string(), log)));
                                }
                            }
                            Err(err) =>  {
                                conn.on_close(&err.to_string(), false).await;
                                break;
                            }
                        }
                    } else {
                        conn.file_timer = crate::rustdesk_interval(time::interval_at(Instant::now() + SEC30, SEC30));
                    }
                }
                Ok(conns) = hbbs_rx.recv() => {
                    if conns.contains(&id) {
                        conn.send_close_reason_no_retry("Closed manually by web console").await;
                        conn.on_close("web console", true).await;
                        break;
                    }
                }
                Some((instant, value)) = ch.rx_video.recv() => {
                    if !conn.video_ack_required {
                        if let Some(message::Union::VideoFrame(vf)) = &value.union {
                            video_service::notify_video_frame_fetched(vf.display as usize, id, Some(instant.into()));
                        }
                    }
                    let bits = 8 * value.compute_size();
                    let send_begin = Instant::now();
                    let whole = conn.stream.writer().is_none();
                    if let Err(err) = conn.stream.send_video(instant.into(), value).await {
                        conn.on_close(&err.to_string(), false).await;
                        break;
                    }
                    // Only the un-split form wrote just now; the writer task does its own
                    // accounting and reports it once a second.
                    if whole {
                        conn.note_video_sent(bits, send_begin.elapsed().as_millis() as u32);
                    }
                },
                Some((instant, value)) = ch.rx.recv() => {
                    if !conn.send_queued(instant, value).await {
                        break;
                    }
                },
                Some(data) = ch.rx_from_authed.recv() => {
                    match data {
                        _ => {}
                    }
                }
                _ = second_timer.tick() => {
                    if !conn.on_second_tick(ch.rx_video.len()).await {
                        break;
                    }
                }
                _ = test_delay_timer.tick() => {
                    if !conn.on_test_delay_tick(last_recv_time).await {
                        break;
                    }
                }
                clip_file = rx_clip.recv() => match clip_file {
                    Some(_clip) => {
                        #[cfg(feature = "unix-file-copy-paste")]
                        if crate::is_support_file_copy_paste(&conn.lr.version)
                        {
                            conn.handle_file_clip(_clip).await;
                        }
                    }
                    None => {
                        //
                    }
                },
            }
        }

        conn.finish(&mut ch.rx_from_cm).await;
    }
}
