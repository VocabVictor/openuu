use super::*;

impl TerminalServiceProxy {
    pub(super) fn handle_open(
        &self,
        service: &mut PersistentTerminalService,
        open: &OpenTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        // When the client requests a terminal_id that doesn't exist but there are
        // surviving persistent sessions, remap the lowest-ID session to the requested
        // terminal_id. This handles the case where _nextTerminalId resets to 1 on
        // reconnect but the server-side sessions have non-contiguous IDs (e.g. {2: htop}).
        //
        // The client's requested terminal_id may not match any surviving session ID
        // (e.g. _nextTerminalId incremented beyond the surviving IDs). This remap is a
        // one-time handle reassignment — only the first reconnect triggers it because
        // needs_session_sync is cleared afterward. Remaining sessions are communicated
        // back via `persistent_sessions` with their original server-side IDs.
        if !service.sessions.contains_key(&open.terminal_id)
            && service.needs_session_sync
            && !service.sessions.is_empty()
        {
            if let Some(&lowest_id) = service.sessions.keys().min() {
                log::info!(
                    "Remapping persistent session {} -> {} for reconnection",
                    lowest_id,
                    open.terminal_id
                );
                if let Some(session_arc) = service.sessions.remove(&lowest_id) {
                    service.sessions.insert(open.terminal_id, session_arc);
                }
            }
        }

        // Check if terminal already exists
        if let Some(session_arc) = service.sessions.get(&open.terminal_id) {
            // Reconnect to existing terminal
            let mut session = session_arc.lock().unwrap();
            // Directly enter Active state with pending replay for immediate streaming.
            // The replay combines output_buffer history and the channel backlog that was
            // already pending at reconnect time so the client can suppress stale xterm
            // query answers without requiring a protobuf schema change.
            // During disconnect, read_outputs() is not called; channel data can still be lost
            // if output_rx fills before reconnect drains it.
            let mut buffer = session
                .output_buffer
                .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES);
            let mut reconnect_backlog = Vec::new();
            if let Some(output_rx) = &session.output_rx {
                // Cap reconnect-time drain so a chatty PTY cannot keep OpenTerminal
                // inside this loop indefinitely. Remaining output is drained by read_outputs().
                for _ in 0..CHANNEL_BUFFER_SIZE {
                    let Ok(data) = output_rx.try_recv() else {
                        break;
                    };
                    reconnect_backlog.push(data);
                }
            }
            let has_reconnect_backlog = !reconnect_backlog.is_empty();
            for data in reconnect_backlog {
                session.output_buffer.append(&data);
            }
            if has_reconnect_backlog {
                buffer = session
                    .output_buffer
                    .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES);
            }
            let has_pending = !buffer.is_empty();
            session.state = SessionState::Active {
                pending_buffer: if has_pending { Some(buffer) } else { None },
                // Always trigger two-phase SIGWINCH on reconnect to force TUI app redraw,
                // regardless of whether there's pending buffer data. This avoids edge cases
                // where buffer is empty but a TUI app (top/htop) still needs a full redraw.
                sigwinch: SigwinchPhase::TempResize {
                    retries: MAX_SIGWINCH_PHASE_ATTEMPTS,
                },
            };
            let mut opened = TerminalOpened::new();
            opened.terminal_id = open.terminal_id;
            opened.success = true;
            opened.message = if has_pending {
                "Reconnected to existing terminal with pending output".to_string()
            } else {
                "Reconnected to existing terminal".to_string()
            };
            opened.pid = session.pid;
            opened.service_id = self.service_id.clone();
            opened.replay_terminal_output = has_pending;
            if service.needs_session_sync {
                if service.sessions.len() > 1 {
                    // No need to include the current terminal in the list.
                    // Because the `persistent_sessions` is used to restore the other sessions.
                    opened.persistent_sessions = service
                        .sessions
                        .keys()
                        .filter(|&id| *id != open.terminal_id)
                        .cloned()
                        .collect();
                }
                service.needs_session_sync = false;
            }
            response.set_opened(opened);

            return Ok(Some(response));
        }

        // Windows with user_token: use helper process to run shell as the logged-in user
        // This solves the ConPTY + CreateProcessAsUserW incompatibility issue where
        // vim, Claude Code, and other TUI applications hang when ConPTY is created
        // by SYSTEM service but shell runs as user via CreateProcessAsUserW.
        #[cfg(target_os = "windows")]
        if self.user_token.is_some() {
            return self.handle_open_with_helper(service, open);
        }

        // Create new terminal session
        log::info!(
            "Creating new terminal {} for service {}",
            open.terminal_id,
            service.service_id
        );
        let mut session =
            TerminalSession::new(open.terminal_id, open.rows as u16, open.cols as u16);

        let pty_size = PtySize {
            rows: open.rows as u16,
            cols: open.cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        };

        log::debug!("Opening PTY with size: {}x{}", open.rows, open.cols);
        let pty_system = portable_pty::native_pty_system();
        let pty_pair = pty_system.openpty(pty_size).context("Failed to open PTY")?;

        // Use default shell for the platform
        let shell = get_default_shell();
        log::debug!("Using shell: {}", shell);

        #[allow(unused_mut)]
        let mut cmd = CommandBuilder::new(&shell);

        #[cfg(target_os = "windows")]
        configure_utf8_shell_command(&shell, &mut cmd);

        // macOS-specific terminal configuration
        // 1. Use login shell (-l) to load user's shell profile (~/.zprofile, ~/.bash_profile)
        //    This ensures PATH includes Homebrew paths (/opt/homebrew/bin, /usr/local/bin)
        // 2. Set TERM environment variable for proper terminal behavior
        //    This fixes issues with control sequences (e.g., Delete/Backspace keys)
        //    macOS terminfo uses hex naming: '78' = 'x' for xterm entries
        // Note: For Linux, `TERM` is set in src/platform/linux.rs try_start_server_()
        #[cfg(target_os = "macos")]
        {
            // Start as login shell to load user environment (PATH, etc.)
            cmd.arg("-l");
            log::debug!("Added -l flag for macOS login shell");

            let term = if std::path::Path::new("/usr/share/terminfo/78/xterm-256color").exists() {
                "xterm-256color"
            } else {
                "xterm"
            };
            cmd.env("TERM", term);
            log::debug!("Set TERM={} for macOS PTY", term);

            if should_force_process_utf8_ctype() {
                cmd.env_remove("LC_ALL");
                cmd.env("LC_CTYPE", "en_US.UTF-8");
                log::debug!("Set LC_CTYPE=en_US.UTF-8 for macOS PTY");
            }
        }

        // Note: On Windows with user_token, we use helper mode (handle_open_with_helper)
        // which is dispatched earlier in this function. This code path is only reached
        // when user_token is None (e.g., running directly as user, not as SYSTEM service).

        log::debug!("Spawning shell process...");
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .context("Failed to spawn command")?;

        let writer = pty_pair
            .master
            .take_writer()
            .context("Failed to get writer")?;

        let reader = pty_pair
            .master
            .try_clone_reader()
            .context("Failed to get reader")?;

        session.pid = child.process_id().unwrap_or(0) as u32;

        // Create channels for input/output
        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

        // Spawn writer thread
        let terminal_id = open.terminal_id;
        let writer_thread = thread::spawn(move || {
            let mut writer = writer;
            while let Ok(data) = input_rx.recv() {
                if let Err(e) = writer.write_all(&data) {
                    log::error!("Terminal {} write error: {}", terminal_id, e);
                    break;
                }
                if let Err(e) = writer.flush() {
                    log::error!("Terminal {} flush error: {}", terminal_id, e);
                }
            }
            log::debug!("Terminal {} writer thread exiting", terminal_id);
        });

        let exiting = session.exiting.clone();
        // Spawn reader thread
        let terminal_id = open.terminal_id;
        let reader_thread = thread::spawn(move || {
            let mut reader = reader;
            let mut buf = vec![0u8; 4096];
            let mut utf8_chunks = Utf8ChunkAccumulator::default();
            let mut drop_count: u64 = 0;
            // Initialize to > 5s ago so the first drop triggers a warning immediately.
            let mut last_drop_warn = Instant::now() - Duration::from_secs(6);
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => {
                        // EOF
                        // This branch can be reached when the child process exits on macOS.
                        // But not on Linux and Windows in my tests.
                        if let Some(data) = utf8_chunks.finish() {
                            let _ = try_send_output(
                                &output_tx,
                                data,
                                terminal_id,
                                "",
                                &mut drop_count,
                                &mut last_drop_warn,
                            );
                        }
                        break;
                    }
                    Ok(n) => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        let Some(data) = utf8_chunks.push_chunk(buf[..n].to_vec()) else {
                            continue;
                        };
                        // Use try_send to avoid blocking the reader thread when channel is full.
                        // During disconnect, the run loop (sp.ok()) stops and read_outputs() is
                        // no longer called, so the channel won't be drained. Blocking send would
                        // deadlock the reader thread in that case.
                        // Note: data produced during disconnect may be lost if channel fills up,
                        // since output_buffer is only updated in read_outputs(). The buffer will
                        // contain history from before the disconnect, not data produced after it.
                        if try_send_output(
                            &output_tx,
                            data,
                            terminal_id,
                            "",
                            &mut drop_count,
                            &mut last_drop_warn,
                        ) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // This branch is not reached in my tests, but we still add `exiting` check to ensure we can exit.
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        // For non-blocking I/O, sleep briefly
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        log::error!("Terminal {} read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            log::debug!("Terminal {} reader thread exiting", terminal_id);
        });

        session.pty_pair = Some(pty_pair);
        session.child = Some(child);
        session.input_tx = Some(input_tx);
        session.output_rx = Some(output_rx);
        session.reader_thread = Some(reader_thread);
        session.writer_thread = Some(writer_thread);
        session.state = SessionState::Active {
            pending_buffer: None,
            sigwinch: SigwinchPhase::Idle,
        };

        let mut opened = TerminalOpened::new();
        opened.terminal_id = open.terminal_id;
        opened.success = true;
        opened.message = "Terminal opened".to_string();
        opened.pid = session.pid;
        opened.service_id = service.service_id.clone();
        if service.needs_session_sync {
            if !service.sessions.is_empty() {
                opened.persistent_sessions = service.sessions.keys().cloned().collect();
            }
            service.needs_session_sync = false;
        }
        response.set_opened(opened);

        log::info!(
            "Terminal {} opened successfully with PID {}",
            open.terminal_id,
            session.pid
        );

        // Store the session
        service
            .sessions
            .insert(open.terminal_id, Arc::new(Mutex::new(session)));

        Ok(Some(response))
    }
}
