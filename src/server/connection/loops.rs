use super::*;

impl Connection {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn handle_input(receiver: std_mpsc::Receiver<MessageInput>, tx: Sender) {
        let mut block_input_mode = false;
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        {
            rdev::set_mouse_extra_info(enigo::ENIGO_INPUT_EXTRA_VALUE);
            rdev::set_keyboard_extra_info(enigo::ENIGO_INPUT_EXTRA_VALUE);
        }
        #[cfg(target_os = "macos")]
        reset_input_ondisconn();
        loop {
            match receiver.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(v) => match v {
                    MessageInput::Mouse(mouse_input) => {
                        handle_mouse(
                            &mouse_input.msg,
                            mouse_input.conn_id,
                            mouse_input.username,
                            mouse_input.argb,
                            mouse_input.simulate,
                            mouse_input.show_cursor,
                        );
                    }
                    MessageInput::Key((mut msg, press)) => {
                        // Set the press state to false, use `down` only in `handle_key()`.
                        msg.press = false;
                        if press {
                            msg.down = true;
                        }
                        handle_key(&msg);
                        if press {
                            msg.down = false;
                            handle_key(&msg);
                        }
                    }
                    MessageInput::Pointer((msg, id)) => {
                        handle_pointer(&msg, id);
                    }
                    MessageInput::BlockOn => {
                        let (ok, msg) = crate::platform::block_input(true);
                        if ok {
                            block_input_mode = true;
                        } else {
                            Self::send_block_input_error(
                                &tx,
                                back_notification::BlockInputState::BlkOnFailed,
                                msg,
                            );
                        }
                    }
                    MessageInput::BlockOff => {
                        let (ok, msg) = crate::platform::block_input(false);
                        if ok {
                            block_input_mode = false;
                        } else {
                            Self::send_block_input_error(
                                &tx,
                                back_notification::BlockInputState::BlkOffFailed,
                                msg,
                            );
                        }
                    }
                },
                Err(err) => {
                    if block_input_mode {
                        let _ = crate::platform::block_input(true);
                    }
                    if std_mpsc::RecvTimeoutError::Disconnected == err {
                        break;
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        clear_remapped_keycode();
        log::debug!("Input thread exited");
    }

    pub(super) async fn post_seq_loop(mut rx: mpsc::UnboundedReceiver<(String, Value)>) {
        while let Some((url, v)) = rx.recv().await {
            allow_err!(Self::post_audit_async(url, v).await);
        }
        log::debug!("post_seq_loop exited");
    }

    pub(super) async fn try_port_forward_loop(
        &mut self,
        rx_from_cm: &mut mpsc::UnboundedReceiver<Data>,
    ) -> ResultType<()> {
        let mut last_recv_time = Instant::now();
        if let Some(mut forward) = self.port_forward_socket.take() {
            log::info!("Running port forwarding loop");
            self.stream.set_raw();
            let mut hbbs_rx = crate::hbbs_http::sync::signal_receiver();
            loop {
                tokio::select! {
                    Some(data) = rx_from_cm.recv() => {
                        match data {
                            ipc::Data::Close => {
                                bail!("Close requested from connection manager");
                            }
                            // Same end as above: a tunnel must not outlive the window either.
                            // Only the reason differs, and a port forward carries none - the
                            // peer sees the tunnel drop and decides for itself.
                            #[cfg(target_os = "linux")]
                            ipc::Data::CmWindowClosed => {
                                bail!("Connection manager window closed");
                            }
                            ipc::Data::CmErr(e) => {
                                log::error!("Connection manager error: {e}");
                                bail!("{e}");
                            }
                            _ => {}
                        }
                    }
                    res = forward.next() => {
                        if let Some(res) = res {
                            last_recv_time = Instant::now();
                            self.stream.send_bytes(res?.into()).await?;
                        } else {
                            bail!("Forward reset by the peer");
                        }
                    },
                    res = self.stream.next() => {
                        if let Some(res) = res {
                            last_recv_time = Instant::now();
                            timeout(SEND_TIMEOUT_OTHER, forward.send(res?)).await??;
                        } else {
                            bail!("Stream reset by the peer");
                        }
                    },
                    _ = self.timer.tick() => {
                        if last_recv_time.elapsed() >= H1 {
                            bail!("Timeout");
                        }
                    }
                    Ok(conns) = hbbs_rx.recv() => {
                        if conns.contains(&self.inner.id) {
                            // todo: check reconnect
                            bail!("Closed manually by the web console");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
