use super::*;

impl TerminalServiceProxy {
    pub fn read_outputs(&self) -> Vec<TerminalResponse> {
        let service = match get_service(&self.service_id) {
            Some(s) => s,
            None => {
                return vec![];
            }
        };

        // Get session references with minimal service lock time
        let sessions: Vec<(i32, Arc<Mutex<TerminalSession>>)> = {
            let service = service.lock().unwrap();
            service
                .sessions
                .iter()
                .map(|(id, session)| (*id, session.clone()))
                .collect()
        };

        let mut responses = Vec::new();
        let mut closed_terminals = Vec::new();

        // Process each session with its own lock
        for (terminal_id, session_arc) in sessions {
            if let Ok(mut session) = session_arc.try_lock() {
                // Check if the session has ended (reader thread finished or child exited).
                // On Linux, the PTY reader thread may not return EOF when the shell exits
                // (the cloned master fd keeps the read side open), so we also poll the child
                // process via try_wait() as a fallback detection mechanism.
                let mut should_send_closed = false;
                if !session.closed_message_sent {
                    if let Some(thread) = &session.reader_thread {
                        if thread.is_finished() {
                            should_send_closed = true;
                        }
                    }
                    if !should_send_closed {
                        if let Some(child) = &mut session.child {
                            match child.try_wait() {
                                Ok(Some(_)) => {
                                    should_send_closed = true;
                                }
                                Ok(None) => {} // still running
                                Err(e) => {
                                    log::warn!("Terminal {} child wait error: {}", terminal_id, e);
                                }
                            }
                        }
                    }
                    if should_send_closed {
                        session.closed_message_sent = true;
                    }
                }
                // It's Ok to put the closed message here.
                // Because the `reader_thread` is joined in `stop()`,
                // and `stop()` is called before the session is dropped.
                if should_send_closed {
                    closed_terminals.push(terminal_id);
                }

                // Always drain the output channel regardless of session state.
                // When Active: data is sent to client. When Closed (within the same
                // connection): data is buffered in output_buffer for reconnection replay.
                // Note: during actual disconnect, the run loop exits and read_outputs()
                // is not called, so channel data produced after disconnect may be lost.
                let mut has_activity = false;
                let mut received_data = Vec::new();
                if let Some(output_rx) = &session.output_rx {
                    // Try to read all available data
                    while let Ok(data) = output_rx.try_recv() {
                        has_activity = true;
                        received_data.push(data);
                    }
                }

                // Update buffer (always buffer for reconnection support)
                for data in &received_data {
                    session.output_buffer.append(data);
                }

                // Skip sending responses if session is not Active.
                // Data is already buffered above and will be sent on next reconnection.
                // Use a scoped block to limit the mutable borrow of session.state,
                // so we can immutably borrow other session fields afterwards.
                let (replay_buffer, sigwinch_action) = {
                    let (pending_buffer, sigwinch) = match &mut session.state {
                        SessionState::Active {
                            pending_buffer,
                            sigwinch,
                        } => (pending_buffer, sigwinch),
                        _ => continue,
                    };

                    let replay_buffer = pending_buffer.take();

                    // Two-phase SIGWINCH: see SigwinchPhase doc comments for rationale.
                    // Each phase is a single PTY resize, spaced ~30ms apart by the polling
                    // interval, ensuring the TUI app sees a real size change on each signal.
                    let sigwinch_action = match sigwinch {
                        SigwinchPhase::TempResize { retries } => {
                            if *retries == 0 {
                                log::warn!(
                                    "Terminal {} SIGWINCH phase 1 (temp resize) failed after {} attempts, giving up",
                                    terminal_id, MAX_SIGWINCH_PHASE_ATTEMPTS
                                );
                                *sigwinch = SigwinchPhase::Idle;
                                None
                            } else {
                                *retries -= 1;
                                Some(SigwinchAction::TempResize)
                            }
                        }
                        SigwinchPhase::Restore { retries } => {
                            if *retries == 0 {
                                log::warn!(
                                    "Terminal {} SIGWINCH phase 2 (restore) failed after {} attempts, giving up",
                                    terminal_id, MAX_SIGWINCH_PHASE_ATTEMPTS
                                );
                                *sigwinch = SigwinchPhase::Idle;
                                None
                            } else {
                                *retries -= 1;
                                Some(SigwinchAction::Restore)
                            }
                        }
                        SigwinchPhase::Idle => None,
                    };
                    (replay_buffer, sigwinch_action)
                };

                if let Some(buffer) = replay_buffer {
                    if !buffer.is_empty() {
                        responses.push(Self::create_terminal_data_response(terminal_id, buffer));
                    }
                }

                if has_activity {
                    session.update_activity();
                }

                // Execute SIGWINCH resize outside the mutable borrow scope of session.state.
                if let Some(action) = sigwinch_action {
                    #[cfg(target_os = "windows")]
                    let is_helper = session.is_helper_mode;
                    #[cfg(not(target_os = "windows"))]
                    let is_helper = false;
                    let resize_ok = Self::do_sigwinch_resize(
                        terminal_id,
                        session.rows,
                        session.cols,
                        &session.pty_pair,
                        &session.input_tx,
                        is_helper,
                        &action,
                    );
                    if let SessionState::Active { sigwinch, .. } = &mut session.state {
                        match action {
                            SigwinchAction::TempResize => {
                                if resize_ok {
                                    // Phase 1 succeeded — advance to phase 2 (restore).
                                    *sigwinch = SigwinchPhase::Restore {
                                        retries: MAX_SIGWINCH_PHASE_ATTEMPTS,
                                    };
                                }
                                // If failed, retries already decremented; will retry phase 1.
                            }
                            SigwinchAction::Restore => {
                                if resize_ok {
                                    // Phase 2 succeeded — SIGWINCH sequence complete.
                                    *sigwinch = SigwinchPhase::Idle;
                                }
                                // If failed, retries already decremented; will retry phase 2.
                            }
                        }
                    }
                }

                // Send real-time data after historical buffer
                for data in received_data {
                    responses.push(Self::create_terminal_data_response(terminal_id, data));
                }
            }
        }

        // Clean up closed terminals (requires service lock briefly)
        if !closed_terminals.is_empty() {
            let mut sessions = service.lock().unwrap().sessions.clone();
            for terminal_id in closed_terminals {
                let mut exit_code = 0;

                if !self.is_persistent {
                    if let Some(session_arc) = sessions.remove(&terminal_id) {
                        service.lock().unwrap().sessions.remove(&terminal_id);
                        let mut session = session_arc.lock().unwrap();
                        // Take the child and add to zombie reaper
                        if let Some(mut child) = session.child.take() {
                            // Try to get exit code if available
                            if let Ok(Some(status)) = child.try_wait() {
                                exit_code = status.exit_code() as i32;
                            }
                            add_to_reaper(child);
                        }
                    }
                } else {
                    // For persistent sessions, clear the child reference and remove the session
                    // if the closed message has been sent (shell has exited).
                    if let Some(session_arc) = sessions.get(&terminal_id) {
                        let mut session = session_arc.lock().unwrap();
                        if let Some(mut child) = session.child.take() {
                            // Try to get exit code if available
                            if let Ok(Some(status)) = child.try_wait() {
                                exit_code = status.exit_code() as i32;
                            }
                            add_to_reaper(child);
                        }
                        if session.closed_message_sent {
                            // Shell has exited, remove the dead session
                            drop(session);
                            sessions.remove(&terminal_id);
                            service.lock().unwrap().sessions.remove(&terminal_id);
                        }
                    }
                }

                let mut response = TerminalResponse::new();
                let mut closed = TerminalClosed::new();
                closed.terminal_id = terminal_id;
                closed.exit_code = exit_code;
                response.set_closed(closed);
                responses.push(response);
            }
        }

        responses
    }

    /// Cleanup when connection drops
    pub fn on_disconnect(&self) {
        if !self.is_persistent {
            // Remove non-persistent service
            remove_service(&self.service_id);
        }
    }
}
