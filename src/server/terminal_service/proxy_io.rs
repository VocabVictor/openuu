use super::*;

impl TerminalServiceProxy {
    pub(super) fn handle_resize(
        &self,
        session: Option<Arc<Mutex<TerminalSession>>>,
        resize: &ResizeTerminal,
    ) -> Result<Option<TerminalResponse>> {
        if let Some(session_arc) = session {
            let mut session = session_arc.lock().unwrap();
            session.update_activity();
            session.rows = resize.rows as u16;
            session.cols = resize.cols as u16;

            // Note: we do NOT clear the sigwinch phase here. The server-side two-phase
            // SIGWINCH mechanism in read_outputs() is self-contained (temp resize → restore
            // across two polling cycles), so client resize is purely a dimension sync and
            // doesn't affect it.

            // Windows: handle helper mode vs direct PTY mode
            #[cfg(target_os = "windows")]
            {
                if session.is_helper_mode {
                    // Helper mode: send resize command via message protocol
                    if let Some(input_tx) = &session.input_tx {
                        let msg = encode_resize_message(resize.rows as u16, resize.cols as u16);
                        if let Err(e) = input_tx.send(msg) {
                            log::error!("Failed to send resize to helper: {}", e);
                        }
                    } else {
                        log::warn!(
                            "Terminal {} is in helper mode but input_tx is None, cannot send resize",
                            resize.terminal_id
                        );
                    }
                } else {
                    // Direct PTY mode
                    Self::resize_pty(&session, resize)?;
                }
            }

            // Non-Windows: always direct PTY mode
            #[cfg(not(target_os = "windows"))]
            {
                Self::resize_pty(&session, resize)?;
            }
        }
        Ok(None)
    }

    /// Resize PTY directly (used for non-helper mode)
    pub(super) fn resize_pty(session: &TerminalSession, resize: &ResizeTerminal) -> Result<()> {
        if let Some(pty_pair) = &session.pty_pair {
            pty_pair.master.resize(PtySize {
                rows: resize.rows as u16,
                cols: resize.cols as u16,
                pixel_width: 0,
                pixel_height: 0,
            })?;
        }
        Ok(())
    }

    pub(super) fn handle_data(
        &self,
        session: Option<Arc<Mutex<TerminalSession>>>,
        data: &TerminalData,
    ) -> Result<Option<TerminalResponse>> {
        if let Some(session_arc) = session {
            let input = {
                let mut session = session_arc.lock().unwrap();
                session.update_activity();
                if let Some(input_tx) = session.input_tx.clone() {
                    // Encode data for helper mode or send raw for direct PTY mode
                    #[cfg(target_os = "windows")]
                    let msg = if session.is_helper_mode {
                        encode_helper_message(MSG_TYPE_DATA, &data.data)
                    } else {
                        data.data.to_vec()
                    };
                    #[cfg(not(target_os = "windows"))]
                    let msg = data.data.to_vec();

                    Some((input_tx, msg))
                } else {
                    None
                }
            };

            if let Some((input_tx, msg)) = input {
                // Send outside the session lock; SyncSender::send can block when full.
                if let Err(e) = input_tx.send(msg) {
                    log::error!(
                        "Failed to send data to terminal {}: {}",
                        data.terminal_id,
                        e
                    );
                }
            }
        }

        Ok(None)
    }

    pub(super) fn handle_close(
        &self,
        service: &mut PersistentTerminalService,
        close: &CloseTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        // Always close and remove the terminal
        if let Some(session_arc) = service.sessions.remove(&close.terminal_id) {
            let mut session = session_arc.lock().unwrap();
            let exit_code = if let Some(mut child) = session.child.take() {
                child.kill()?;
                add_to_reaper(child);
                -1 // -1 indicates forced termination
            } else {
                0
            };

            let mut closed = TerminalClosed::new();
            closed.terminal_id = close.terminal_id;
            closed.exit_code = exit_code;
            response.set_closed(closed);
            Ok(Some(response))
        } else {
            Ok(None)
        }
    }

    /// Perform a single PTY resize as part of the two-phase SIGWINCH sequence.
    /// Returns true if the resize succeeded.
    ///
    /// Takes individual field references to avoid borrowing the entire TerminalSession,
    /// which would conflict with the mutable borrow of session.state in read_outputs().
    pub(super) fn do_sigwinch_resize(
        terminal_id: i32,
        rows: u16,
        cols: u16,
        pty_pair: &Option<portable_pty::PtyPair>,
        input_tx: &Option<SyncSender<Vec<u8>>>,
        _is_helper_mode: bool,
        action: &SigwinchAction,
    ) -> bool {
        // Skip if dimensions are not initialized (shouldn't happen on reconnect,
        // but guard against it to avoid resizing to nonsensical values).
        if rows == 0 || cols == 0 {
            return false;
        }

        let target_rows = match action {
            SigwinchAction::TempResize => {
                // For very small terminals (≤2 rows), subtracting 1 would result in an unusable
                // size (0 or 1 row), so we add 1 instead. Either direction triggers SIGWINCH.
                if rows > 2 {
                    rows.saturating_sub(1)
                } else {
                    rows.saturating_add(1)
                }
            }
            SigwinchAction::Restore => rows,
        };

        let phase_name = match action {
            SigwinchAction::TempResize => "temp resize",
            SigwinchAction::Restore => "restore",
        };

        #[cfg(target_os = "windows")]
        let use_helper = _is_helper_mode;
        #[cfg(not(target_os = "windows"))]
        let use_helper = false;

        if use_helper {
            #[cfg(target_os = "windows")]
            {
                let input_tx = match input_tx {
                    Some(tx) => tx,
                    None => return false,
                };
                let msg = encode_resize_message(target_rows, cols);
                if let Err(e) = input_tx.try_send(msg) {
                    log::warn!(
                        "Terminal {} SIGWINCH {} via helper failed: {}",
                        terminal_id,
                        phase_name,
                        e
                    );
                    return false;
                }
                true
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = (input_tx, phase_name);
                false
            }
        } else if let Some(pty_pair) = pty_pair {
            if let Err(e) = pty_pair.master.resize(PtySize {
                rows: target_rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                log::warn!(
                    "Terminal {} SIGWINCH {} failed: {}",
                    terminal_id,
                    phase_name,
                    e
                );
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Helper to create a TerminalResponse with optional compression.
    pub(super) fn create_terminal_data_response(terminal_id: i32, data: Vec<u8>) -> TerminalResponse {
        let mut response = TerminalResponse::new();
        let mut terminal_data = TerminalData::new();
        terminal_data.terminal_id = terminal_id;

        if data.len() > COMPRESS_THRESHOLD {
            let compressed = compress::compress(&data);
            if compressed.len() < data.len() {
                terminal_data.data = bytes::Bytes::from(compressed);
                terminal_data.compressed = true;
            } else {
                terminal_data.data = bytes::Bytes::from(data);
            }
        } else {
            terminal_data.data = bytes::Bytes::from(data);
        }

        response.set_data(terminal_data);
        response
    }
}
