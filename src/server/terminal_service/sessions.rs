use super::*;

pub struct TerminalSession {
    pub created_at: Instant,
    pub(super) last_activity: Instant,
    pub(super) pty_pair: Option<portable_pty::PtyPair>,
    pub(super) child: Option<Box<dyn Child + std::marker::Send + Sync>>,
    // Channel for sending input to the writer thread
    pub(super) input_tx: Option<SyncSender<Vec<u8>>>,
    // Channel for receiving output from the reader thread
    pub(super) output_rx: Option<Receiver<Vec<u8>>>,
    pub(super) exiting: Arc<AtomicBool>,
    // Thread handles
    pub(super) reader_thread: Option<thread::JoinHandle<()>>,
    pub(super) writer_thread: Option<thread::JoinHandle<()>>,
    pub(super) output_buffer: OutputBuffer,
    pub(super) title: String,
    pub(super) pid: u32,
    pub(super) rows: u16,
    pub(super) cols: u16,
    // Track if we've already sent the closed message
    pub(super) closed_message_sent: bool,
    // Session state machine for reconnection handling
    pub(super) state: SessionState,
    // Helper mode: PTY is managed by helper process, communication via message protocol
    #[cfg(target_os = "windows")]
    pub(super) is_helper_mode: bool,
    // Handle to helper process for termination when session closes
    #[cfg(target_os = "windows")]
    pub(super) helper_process_handle: Option<SendableHandle>,
}

impl TerminalSession {
    pub(super) fn new(terminal_id: i32, rows: u16, cols: u16) -> Self {
        Self {
            created_at: Instant::now(),
            last_activity: Instant::now(),
            pty_pair: None,
            child: None,
            input_tx: None,
            output_rx: None,
            exiting: Arc::new(AtomicBool::new(false)),
            reader_thread: None,
            writer_thread: None,
            output_buffer: OutputBuffer::new(),
            title: format!("Terminal {}", terminal_id),
            pid: 0,
            rows,
            cols,
            closed_message_sent: false,
            state: SessionState::Closed,
            #[cfg(target_os = "windows")]
            is_helper_mode: false,
            #[cfg(target_os = "windows")]
            helper_process_handle: None,
        }
    }

    pub(super) fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    // This helper function is to ensure that the threads are joined before the child process is dropped.
    // Though this is not strictly necessary on macOS.
    pub(super) fn stop(&mut self) {
        self.state = SessionState::Closed;
        self.exiting.store(true, Ordering::SeqCst);

        // Drop the input channel to signal writer thread to exit
        if let Some(input_tx) = self.input_tx.take() {
            // Send a final newline to ensure the reader can read some data, and then exit.
            // This is required on Windows and Linux.
            // Although `self.pty_pair = None;` is called below, we can still send a final newline here.
            #[cfg(target_os = "windows")]
            let final_msg = if self.is_helper_mode {
                encode_helper_message(MSG_TYPE_DATA, b"\r\n")
            } else {
                b"\r\n".to_vec()
            };
            #[cfg(not(target_os = "windows"))]
            let final_msg = b"\r\n".to_vec();

            if let Err(e) = input_tx.send(final_msg) {
                log::warn!("Failed to send final newline to the terminal: {}", e);
            }
            drop(input_tx);
        }
        self.output_rx = None;

        // CRITICAL: In helper mode, we must terminate the helper process BEFORE joining threads!
        // The reader thread is blocking on output_pipe.read(), which only returns EOF when
        // the helper process exits. If we try to join the reader thread first, we deadlock.
        //
        // Sequence for helper mode:
        // 1. Signal exiting and close input channel (done above)
        // 2. Terminate helper process (causes output pipe EOF)
        // 3. Join reader thread (now unblocked due to EOF)
        // 4. Join writer thread
        #[cfg(target_os = "windows")]
        if self.is_helper_mode {
            if let Some(helper_handle) = self.helper_process_handle.take() {
                let handle = helper_handle.as_raw();
                log::debug!("Helper mode: terminating helper process before joining threads...");

                // Give helper a very short time to exit gracefully (it should detect pipe close)
                // But don't wait too long - we need to unblock the reader thread
                let wait_result = unsafe { WinWaitForSingleObject(handle, 100) };

                if wait_result == WIN_WAIT_OBJECT_0 {
                    log::debug!("Helper process exited gracefully");
                } else {
                    // Force terminate to unblock reader thread
                    log::debug!("Force terminating helper process to unblock reader thread");
                    unsafe {
                        let _ = WinTerminateProcess(handle, 0);
                    }
                }

                unsafe {
                    let _ = WinCloseHandle(handle);
                }
            }
        }

        // 1. Windows (non-helper mode)
        //    `pty_pair` uses pipe. https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/win/conpty.rs#L16
        //     `read()` may stuck at https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/filedescriptor/src/windows.rs#L345
        //     We can close the pipe to signal the reader thread to exit.
        //     After https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/win/psuedocon.rs#L86, the reader reads `[27, 91, 63, 57, 48, 48, 49, 108, 27, 91, 63, 49, 48, 48, 52, 108]` in my tests.
        // 2. Linux
        //    `pty_pair` uses `libc::openpty`. https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/unix.rs#L32
        //    We can also call the drop method first. https://github.com/rustdesk-org/wezterm/blob/80174f8009f41565f0fa8c66dab90d4f9211ae16/pty/src/unix.rs#L352
        //    The reader will get [13, 10] after dropping the `pty_pair`.
        // 3. macOS
        //    No stuck cases have been found so far, more testing is needed.
        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            self.pty_pair = None;
        }

        // Wait for threads to finish
        // The reader thread should join before the writer thread on Windows.
        if let Some(reader_thread) = self.reader_thread.take() {
            let _ = reader_thread.join();
        }

        // The read can read the last "\r\n" after the writer thread (not the child process) exits
        // on Linux in my tests.
        // But we still send "\r\n" to the writer thread and let the reader thread exit first for safety.
        if let Some(writer_thread) = self.writer_thread.take() {
            let _ = writer_thread.join();
        }

        if let Some(mut child) = self.child.take() {
            // Kill the process
            let _ = child.kill();
            add_to_reaper(child);
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        // Ensure child process is properly handled when session is dropped
        self.stop();
    }
}

/// Persistent terminal service that can survive connection drops
pub struct PersistentTerminalService {
    pub(super) service_id: String,
    pub(super) sessions: HashMap<i32, Arc<Mutex<TerminalSession>>>,
    pub created_at: Instant,
    pub(super) last_activity: Instant,
    pub is_persistent: bool,
    pub(super) needs_session_sync: bool,
    pub(super) is_specified_user: bool,
}

impl PersistentTerminalService {
    pub fn new(service_id: String, is_persistent: bool, is_specified_user: bool) -> Self {
        Self {
            service_id,
            sessions: HashMap::new(),
            created_at: Instant::now(),
            last_activity: Instant::now(),
            is_persistent,
            needs_session_sync: false,
            is_specified_user,
        }
    }

    pub(super) fn update_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Get list of terminal metadata
    pub fn list_terminals(&self) -> Vec<(i32, String, u32, Instant)> {
        self.sessions
            .iter()
            .map(|(id, session)| {
                let s = session.lock().unwrap();
                (*id, s.title.clone(), s.pid, s.created_at)
            })
            .collect()
    }

    /// Get buffered output for a terminal
    pub fn get_terminal_buffer(&self, terminal_id: i32, max_bytes: usize) -> Option<Vec<u8>> {
        self.sessions.get(&terminal_id).map(|session| {
            let session = session.lock().unwrap();
            session.output_buffer.get_recent(max_bytes)
        })
    }

    /// Get terminal info for recovery
    pub fn get_terminal_info(&self, terminal_id: i32) -> Option<(u16, u16, Vec<u8>)> {
        self.sessions.get(&terminal_id).map(|session| {
            let session = session.lock().unwrap();
            (
                session.rows,
                session.cols,
                session
                    .output_buffer
                    .get_recent(DEFAULT_RECONNECT_BUFFER_BYTES),
            )
        })
    }

    /// Check if service has active terminals
    pub fn has_active_terminals(&self) -> bool {
        !self.sessions.is_empty()
    }

    pub(super) fn reset_status(&mut self, is_persistent: bool) {
        self.is_persistent = is_persistent;
        self.needs_session_sync = true;
        for session in self.sessions.values() {
            let mut session = session.lock().unwrap();
            session.state = SessionState::Closed;
        }
    }
}
