use super::*;

impl TerminalServiceProxy {
    /// Windows-only: Open terminal using helper process pattern
    /// This solves the ConPTY + CreateProcessAsUserW incompatibility issue.
    /// The helper process runs as the logged-in user and creates ConPTY + shell,
    /// communicating with this service via named pipes.
    #[cfg(target_os = "windows")]
    pub(super) fn handle_open_with_helper(
        &self,
        service: &mut PersistentTerminalService,
        open: &OpenTerminal,
    ) -> Result<Option<TerminalResponse>> {
        let mut response = TerminalResponse::new();

        log::info!(
            "Creating new terminal {} using helper process for service: {}",
            open.terminal_id,
            service.service_id
        );

        let mut session =
            TerminalSession::new(open.terminal_id, open.rows as u16, open.cols as u16);

        // Generate unique pipe names for this terminal
        let pipe_id = uuid::Uuid::new_v4();
        let input_pipe_name = format!(r"\\.\pipe\rustdesk_term_in_{}", pipe_id);
        let output_pipe_name = format!(r"\\.\pipe\rustdesk_term_out_{}", pipe_id);

        log::debug!(
            "Creating pipes: input={}, output={}",
            input_pipe_name,
            output_pipe_name
        );

        // Get user_token early - needed for both DACL creation and helper launch
        let user_token = self
            .user_token
            .ok_or_else(|| anyhow!("user_token is required for helper mode"))?;

        // Create pipes (server side, don't wait for connection yet)
        // input_pipe: service WRITES to this, helper READS from this
        // output_pipe: service READS from this, helper WRITES to this
        // Using OwnedHandle for RAII - handles are automatically closed on error
        // Pass user_token to create restricted DACL (only SYSTEM + user can access)
        let input_pipe_handle = OwnedHandle::new(create_named_pipe_server(
            &input_pipe_name,
            false,
            user_token,
        )?);
        let output_pipe_handle = OwnedHandle::new(create_named_pipe_server(
            &output_pipe_name,
            true,
            user_token,
        )?);

        let helper_process_info = launch_terminal_helper_with_token(
            user_token,
            &input_pipe_name,
            &output_pipe_name,
            open.terminal_id,
            open.rows as u16,
            open.cols as u16,
        )?;

        // Use HelperProcessGuard for RAII cleanup - terminates process on error
        // Unlike OwnedHandle which only closes the handle, this guard ensures
        // the helper process is terminated if pipe connection fails or other errors occur.
        let helper_process_guard =
            HelperProcessGuard::new(helper_process_info.handle, helper_process_info.pid);
        let helper_pid = helper_process_guard.pid();

        // Wait for helper to connect to pipes
        // If this fails, HelperProcessGuard will terminate the helper process
        let mut input_pipe = wait_for_pipe_connection(
            input_pipe_handle,
            &input_pipe_name,
            PIPE_CONNECTION_TIMEOUT_MS,
        )?;
        let mut output_pipe = wait_for_pipe_connection(
            output_pipe_handle,
            &output_pipe_name,
            PIPE_CONNECTION_TIMEOUT_MS,
        )?;

        // Check if helper process is still running after pipe connection
        // This provides early detection if helper crashed during startup
        if !is_helper_process_running(helper_process_guard.as_raw()) {
            return Err(anyhow!(
                "Helper process (PID {}) exited unexpectedly after pipe connection",
                helper_pid
            ));
        }

        // Disarm the guard and transfer ownership to session
        // From this point, the session is responsible for terminating the helper
        let helper_raw_handle = helper_process_guard.disarm();

        // Use helper process PID for session tracking
        // Note: This is the helper process PID, not the actual shell PID.
        // The real shell runs inside the helper process but its PID is not exposed here.
        // For process management (termination, status), the helper PID is what we need.
        session.pid = helper_pid;

        // Create channels for input/output (same as direct PTY mode)
        let (input_tx, input_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);
        let (output_tx, output_rx) = mpsc::sync_channel::<Vec<u8>>(CHANNEL_BUFFER_SIZE);

        // Spawn writer thread: reads from channel, writes to input pipe
        let terminal_id = open.terminal_id;
        let writer_thread = thread::spawn(move || {
            while let Ok(data) = input_rx.recv() {
                if let Err(e) = input_pipe.write_all(&data) {
                    log::error!("Terminal {} pipe write error: {}", terminal_id, e);
                    break;
                }
                if let Err(e) = input_pipe.flush() {
                    log::error!("Terminal {} pipe flush error: {}", terminal_id, e);
                }
            }
            log::debug!(
                "Terminal {} writer thread (helper mode) exiting",
                terminal_id
            );
        });

        // Spawn reader thread: reads from output pipe, sends to channel
        // Note: The output pipe was created with FILE_FLAG_OVERLAPPED for timeout support
        // during ConnectNamedPipe. However, once converted to a File handle, reads are
        // performed synchronously. The WouldBlock handling below is defensive but may
        // not be triggered in practice since File::read() blocks until data is available.
        let exiting = session.exiting.clone();
        let terminal_id = open.terminal_id;
        let reader_thread = thread::spawn(move || {
            let mut buf = vec![0u8; 4096];
            let mut utf8_chunks = Utf8ChunkAccumulator::default();
            let mut drop_count: u64 = 0;
            // Initialize to > 5s ago so the first drop triggers a warning immediately.
            let mut last_drop_warn = Instant::now() - Duration::from_secs(6);
            loop {
                match output_pipe.read(&mut buf) {
                    Ok(0) => {
                        if let Some(data) = utf8_chunks.finish() {
                            let _ = try_send_output(
                                &output_tx,
                                data,
                                terminal_id,
                                " (helper)",
                                &mut drop_count,
                                &mut last_drop_warn,
                            );
                        }
                        // EOF - helper process exited
                        log::debug!("Terminal {} helper output EOF", terminal_id);
                        break;
                    }
                    Ok(n) => {
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        let Some(data) = utf8_chunks.push_chunk(buf[..n].to_vec()) else {
                            continue;
                        };
                        // Use try_send to avoid blocking the reader thread (same as direct PTY mode)
                        if try_send_output(
                            &output_tx,
                            data,
                            terminal_id,
                            " (helper)",
                            &mut drop_count,
                            &mut last_drop_warn,
                        ) {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        // Defensive: WouldBlock is unlikely with synchronous File::read(),
                        // but handle it gracefully just in case.
                        if exiting.load(Ordering::SeqCst) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(e) => {
                        log::error!("Terminal {} pipe read error: {}", terminal_id, e);
                        break;
                    }
                }
            }
            log::debug!(
                "Terminal {} reader thread (helper mode) exiting",
                terminal_id
            );
        });

        // In helper mode, we don't have pty_pair or child - helper manages those
        session.pty_pair = None;
        session.child = None;
        session.input_tx = Some(input_tx);
        session.output_rx = Some(output_rx);
        session.reader_thread = Some(reader_thread);
        session.writer_thread = Some(writer_thread);
        session.state = SessionState::Active {
            pending_buffer: None,
            sigwinch: SigwinchPhase::Idle,
        };
        session.is_helper_mode = true;
        session.helper_process_handle = Some(SendableHandle::new(helper_raw_handle));

        let mut opened = TerminalOpened::new();
        opened.terminal_id = open.terminal_id;
        opened.success = true;
        opened.message = "Terminal opened (helper mode)".to_string();
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
            "Terminal {} opened successfully using helper process (PID {})",
            open.terminal_id,
            session.pid
        );

        service
            .sessions
            .insert(open.terminal_id, Arc::new(Mutex::new(session)));

        Ok(Some(response))
    }
}
