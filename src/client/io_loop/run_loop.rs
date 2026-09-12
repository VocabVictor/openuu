use super::*;

impl<T: InvokeUiSession> Remote<T> {
    pub async fn io_loop(&mut self, key: &str, token: &str, round: u32) {
        #[cfg(target_os = "windows")]
        let _file_clip_context_holder = {
            // `is_port_forward()` will not reach here, but we still check it for clarity.
            if self.handler.is_default() {
                // It is ok to call this function multiple times.
                ContextSend::enable(true);
                Some(crate::SimpleCallOnReturn {
                    b: true,
                    f: Box::new(|| {
                        // No need to call `enable(false)` for sciter version, because each client of sciter version is a new process.
                        // It's better to check if the peers are windows(support file copy&paste), but it's not necessary.
                        #[cfg(feature = "flutter")]
                        if !crate::flutter::sessions::has_sessions_running(ConnType::DEFAULT_CONN) {
                            ContextSend::enable(false);
                        };
                    }),
                })
            } else {
                None
            }
        };

        let mut received = false;
        let conn_type = if self.handler.is_file_transfer() {
            ConnType::FILE_TRANSFER
        } else if self.handler.is_view_camera() {
            ConnType::VIEW_CAMERA
        } else if self.handler.is_terminal() {
            ConnType::TERMINAL
        } else {
            ConnType::default()
        };

        match Client::start(
            &self.handler.get_id(),
            key,
            token,
            conn_type,
            self.handler.clone(),
        )
        .await
        {
            Ok(((mut peer, direct, pk, kcp, stream_type), (feedback, rendezvous_server))) => {
                self.handler
                    .connection_round_state
                    .lock()
                    .unwrap()
                    .set_connected();
                let is_secured = peer.is_secured();
                // Only WebRTC needs refining: its label names the transport that won the race,
                // not the family ICE ended up nominating, and it is the one path where the two
                // can disagree with the address the rendezvous observed.
                let stream_type = if peer.webrtc_remote_ipv6().await.unwrap_or(false) {
                    "WebRTC/IPv6"
                } else {
                    stream_type
                };
                self.handler
                    .set_connection_type(is_secured, direct, stream_type); // flutter -> connection_ready
                if !is_secured
                    && !crate::common::is_direct_ip_access(&self.handler.get_id())
                    && !client::confirm_insecure_connection(&self.handler, &mut self.receiver).await
                {
                    self.send_close_reason(&mut peer, "").await;
                    if kcp.is_some() {
                        tokio::time::sleep(KCP_CLOSE_REASON_FLUSH_DELAY).await;
                    }
                    self.handle_disconnected(round);
                    return;
                }
                self.handler.update_direct(Some(direct));
                if conn_type == ConnType::DEFAULT_CONN || conn_type == ConnType::VIEW_CAMERA {
                    self.handler
                        .set_fingerprint(crate::common::pk_to_fingerprint(pk.unwrap_or_default()));
                }

                // just build for now
                #[cfg(not(any(target_os = "windows", feature = "unix-file-copy-paste")))]
                let (_tx_holder, mut rx_clip_client) = mpsc::unbounded_channel::<i32>();

                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                let (_tx_holder, rx) = mpsc::unbounded_channel();
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                let mut rx_clip_client_holder = (Arc::new(TokioMutex::new(rx)), None);
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                {
                    if self.handler.is_default() {
                        (self.client_conn_id, rx_clip_client_holder.0) =
                            clipboard::get_rx_cliprdr_client(&self.handler.get_id());
                        log::debug!("get cliprdr client for conn_id {}", self.client_conn_id);
                        let client_conn_id = self.client_conn_id;
                        rx_clip_client_holder.1 = Some(crate::SimpleCallOnReturn {
                            b: true,
                            f: Box::new(move || {
                                clipboard::remove_channel_by_conn_id(client_conn_id);
                            }),
                        });
                    };
                }
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                let mut rx_clip_client = rx_clip_client_holder.0.lock().await;

                let mut status_timer =
                    crate::rustdesk_interval(time::interval(Duration::new(1, 0)));
                let mut fps_instant = Instant::now();

                let _keep_it = client::hc_connection(feedback, rendezvous_server, token).await;
                let mut last_recv_time = Instant::now();
                let mut webrtc_suspect_since: Option<Instant> = None;
                let mut last_rx_progress = peer.rx_progress();
                let mut peer_gone = false;

                loop {
                    tokio::select! {
                        res = peer.next() => {
                            if let Some(res) = res {
                                match res {
                                    Err(err) => {
                                        self.handler.on_establish_connection_error(err.to_string());
                                        break;
                                    }
                                    Ok(ref bytes) => {
                                        last_recv_time = Instant::now();
                                        if !received {
                                            received = true;
                                            self.handler.update_received(true);
                                        }
                                        self.data_count.fetch_add(bytes.len(), Ordering::Relaxed);
                                        if !self.handle_msg_from_peer(bytes, &mut peer).await {
                                            break
                                        }
                                    }
                                }
                            } else {
                                if self.handler.is_restarting_remote_device() {
                                    log::info!("Restart remote device");
                                    self.handler.msgbox("restarting", "Restarting remote device", "Connection in progress. Please wait.", "");
                                } else {
                                    log::info!("Reset by the peer");
                                    self.handler.msgbox("error", "Connection Error", "Reset by the peer", "");
                                }
                                break;
                            }
                        }
                        d = self.receiver.recv() => {
                            if let Some(d) = d {
                                if !self.handle_msg_from_ui(d, &mut peer).await {
                                    break;
                                }
                            }
                        }
                        _msg = rx_clip_client.recv() => {
                            #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                            self.handle_local_clipboard_msg(&mut peer, _msg).await;
                        }
                        _ = self.timer.tick() => {
                            if last_recv_time.elapsed() >= SEC30 {
                                self.handler.msgbox("error", "Connection Error", "Timeout", "");
                                break;
                            }
                            if !self.read_jobs.is_empty() {
                                if let Err(err) = fs::handle_read_jobs(&mut self.read_jobs, &mut peer).await {
                                    self.handler.msgbox("error", "Connection Error", &err.to_string(), "");
                                    break;
                                }
                                self.update_jobs_status();
                            } else {
                                self.timer = crate::rustdesk_interval(time::interval_at(Instant::now() + SEC30, SEC30));
                            }
                        }
                        _ = status_timer.tick() => {
                            if crate::account::require_login().await.is_err() {
                                self.handler.on_establish_connection_error("OpenUU login expired".to_owned());
                                break;
                            }
                            if self.handler.is_restarting_remote_device()
                                && last_recv_time.elapsed() >= RESTART_REMOTE_DEVICE_NO_DATA_TIMEOUT
                            {
                                self.handler.msgbox("restarting-show", "Restarting remote device", "Connection in progress. Please wait.", "");
                                break;
                            }
                            let rx_progress = peer.rx_progress();
                            // `None` for transports that report none, and it never changes for a
                            // given one, so they are inert here.
                            let progressed = rx_progress != last_rx_progress;
                            last_rx_progress = rx_progress;
                            if peer.webrtc_disconnected() && !progressed {
                                webrtc_suspect_since.get_or_insert_with(Instant::now);
                            } else {
                                webrtc_suspect_since = None;
                            }
                            // Neither limit is a hard upper bound. A send is awaited inline in
                            // this loop, so one in progress delays this tick - bounded on WebRTC
                            // by the timeout the stream was built with, not bounded at all on
                            // KCP. The 30s watchdog above shares the loop and the same delay.
                            peer_gone = webrtc_suspect_since
                                .map_or(false, |since| since.elapsed() >= WEBRTC_SUSPECT_GRACE)
                                || kcp
                                    .as_ref()
                                    .and_then(|k| k.peer_silent_for())
                                    .map_or(false, |silent| silent >= KCP_PEER_SILENCE_LIMIT);
                            if peer_gone {
                                log::info!("Peer stopped answering, reconnecting");
                                #[cfg(feature = "flutter")]
                                self.handler.msgbox("restarting-show", "Connecting...", "Connection in progress. Please wait.", "");
                                // Sciter knows no `restarting-show` and would show a dialog that
                                // waits for a click, where the timeout this arrives ahead of is
                                // retryable and reconnects on its own. Keep that message for it.
                                #[cfg(not(feature = "flutter"))]
                                self.handler.msgbox("error", "Connection Error", "Timeout", "");
                                break;
                            }
                            let elapsed = fps_instant.elapsed().as_millis();
                            if elapsed < 1000 {
                                continue;
                            }
                            fps_instant = Instant::now();
                            let mut speed = self.data_count.swap(0, Ordering::Relaxed);
                            speed = speed * 1000 / elapsed as usize;
                            let speed = format!("{:.2}kB/s", speed as f32 / 1024 as f32);

                            let fps = self.video_threads.iter().map(|(k, v)| {
                                // Correcting the inaccuracy of status_timer
                                (k.clone(), (*v.frame_count.read().unwrap() as i32) * 1000 / elapsed as i32)
                            }).collect::<HashMap<usize, i32>>();
                            self.video_threads.iter().for_each(|(_, v)| {
                                *v.frame_count.write().unwrap() = 0;
                            });
                            self.fps_control(direct, fps.clone());
                            let chroma = self.chroma.read().unwrap().clone();
                            let chroma = match chroma {
                                Some(Chroma::I444) => "4:4:4",
                                Some(Chroma::I420) => "4:2:0",
                                None => "-",
                            };
                            let chroma = Some(chroma.to_string());
                            let codec_format = if self.video_format == CodecFormat::Unknown {
                                None
                            } else {
                                Some(self.video_format.clone())
                            };
                            self.handler.update_quality_status(QualityStatus {
                                speed: Some(speed),
                                fps,
                                chroma,
                                codec_format,
                                ..Default::default()
                            });
                        }
                    }
                }
                log::debug!("Exit io_loop of id={}", self.handler.get_id());
                // Stop client audio server.
                if let Some(s) = self.stop_voice_call_sender.take() {
                    s.send(()).ok();
                }
                if kcp.is_some() {
                    // Attempted rather than skipped even here: if the loss was one-way the peer
                    // does get it, and drops its side instead of waiting out its own timeout.
                    if peer_gone {
                        peer.set_send_timeout(KCP_CLOSE_REASON_GONE_DEADLINE.as_millis() as u64);
                    }
                    // Send the close reason if it hasn't been sent yet, as KCP cannot detect the socket close event.
                    self.send_close_reason(&mut peer, "kcp").await;
                    // KCP does not send messages immediately, so wait to ensure the last message is sent.
                    // 1ms works in my test, but 30ms is more reliable.
                    tokio::time::sleep(KCP_CLOSE_REASON_FLUSH_DELAY).await;
                }
            }
            Err(err) => {
                self.handler.on_establish_connection_error(err.to_string());
            }
        }
        self.handle_disconnected(round);
    }
}
