use super::*;

/// Run terminal helper process
/// Args: --terminal-helper <input_pipe_name> <output_pipe_name> <rows> <cols> <terminal_id>
pub fn run_terminal_helper(args: &[String]) -> Result<()> {
    if args.len() < 5 {
        return Err(anyhow!(
            "Usage: --terminal-helper <input_pipe> <output_pipe> <rows> <cols> <terminal_id>"
        ));
    }

    let input_pipe_name = &args[0];
    let output_pipe_name = &args[1];
    let rows: u16 = args[2]
        .parse()
        .map_err(|e| anyhow!("Failed to parse rows '{}': {}", args[2], e))?;
    let cols: u16 = args[3]
        .parse()
        .map_err(|e| anyhow!("Failed to parse cols '{}': {}", args[3], e))?;
    let terminal_id: i32 = args[4]
        .parse()
        .map_err(|e| anyhow!("Failed to parse terminal_id '{}': {}", args[4], e))?;

    log::debug!(
        "Terminal helper starting: terminal_id={}, size={}x{}",
        terminal_id,
        cols,
        rows
    );

    // Open named pipes (created by the service)
    let input_pipe = open_pipe(input_pipe_name, true)?;
    let output_pipe = open_pipe(output_pipe_name, false)?;

    // Create ConPTY and shell
    let pty_size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };

    let pty_system = portable_pty::native_pty_system();
    let pty_pair = pty_system.openpty(pty_size).context("Failed to open PTY")?;

    let shell = get_default_shell();
    log::debug!("Using shell: {}", shell);

    let mut cmd = CommandBuilder::new(&shell);
    configure_utf8_shell_command(&shell, &mut cmd);
    let mut child = pty_pair
        .slave
        .spawn_command(cmd)
        .context("Failed to spawn shell")?;

    // Explicitly drop slave after spawning to release resources
    drop(pty_pair.slave);

    let pid = child.process_id().unwrap_or(0);
    log::debug!("Shell started with PID: {}", pid);

    let mut pty_writer = pty_pair
        .master
        .take_writer()
        .context("Failed to get PTY writer")?;

    let mut pty_reader = pty_pair
        .master
        .try_clone_reader()
        .context("Failed to get PTY reader")?;

    // Wrap pty_pair.master in Arc<Mutex> for sharing with input thread (for resize).
    let pty_master: Arc<Mutex<Box<dyn MasterPty + Send>>> = Arc::new(Mutex::new(pty_pair.master));

    let exiting = Arc::new(AtomicBool::new(false));

    // Thread: Read from input pipe, parse messages, write data to PTY or handle control commands
    let exiting_clone = exiting.clone();
    let pty_master_clone = pty_master.clone();
    let input_thread = thread::spawn(move || {
        let mut input_pipe = input_pipe;
        let mut header_buf = [0u8; MSG_HEADER_SIZE];
        let mut payload_buf = vec![0u8; 4096];

        loop {
            if exiting_clone.load(Ordering::SeqCst) {
                break;
            }

            // Read message header
            match read_exact_or_eof(&mut input_pipe, &mut header_buf) {
                Ok(false) => {
                    log::debug!("Input pipe EOF");
                    break;
                }
                Ok(true) => {}
                Err(e) => {
                    log::error!("Input pipe header read error: {}", e);
                    break;
                }
            }

            let msg_type = header_buf[0];
            let payload_len =
                u32::from_le_bytes([header_buf[1], header_buf[2], header_buf[3], header_buf[4]])
                    as usize;

            // Validate payload length to prevent denial of service
            if payload_len > MAX_PAYLOAD_SIZE {
                log::error!(
                    "Payload too large: {} bytes (max {})",
                    payload_len,
                    MAX_PAYLOAD_SIZE
                );
                break;
            }

            // Ensure payload buffer is large enough
            if payload_buf.len() < payload_len {
                payload_buf.resize(payload_len, 0);
            }

            // Read payload
            if payload_len > 0 {
                match read_exact_or_eof(&mut input_pipe, &mut payload_buf[..payload_len]) {
                    Ok(false) => {
                        log::debug!("Input pipe EOF during payload read");
                        break;
                    }
                    Ok(true) => {}
                    Err(e) => {
                        log::error!("Input pipe payload read error: {}", e);
                        break;
                    }
                }
            }

            match msg_type {
                MSG_TYPE_DATA => {
                    // Write terminal data to PTY
                    if let Err(e) = pty_writer.write_all(&payload_buf[..payload_len]) {
                        log::error!("PTY write error: {}", e);
                        break;
                    }
                    if let Err(e) = pty_writer.flush() {
                        log::error!("PTY flush error: {}", e);
                        break;
                    }
                }
                MSG_TYPE_RESIZE => {
                    if payload_len >= 4 {
                        let rows = u16::from_le_bytes([payload_buf[0], payload_buf[1]]);
                        let cols = u16::from_le_bytes([payload_buf[2], payload_buf[3]]);
                        log::debug!("Resize: {}x{}", cols, rows);
                        if let Ok(master) = pty_master_clone.lock() {
                            let _ = master.resize(PtySize {
                                rows,
                                cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            });
                        }
                    }
                }
                _ => {
                    // Unknown type may indicate data corruption - stop to avoid parse errors
                    log::error!("Unknown message type: {}, terminating", msg_type);
                    break;
                }
            }
        }
        log::debug!("Input thread exiting");
    });

    // Thread: Read from PTY, write to output pipe
    let exiting_clone = exiting.clone();
    let output_thread = thread::spawn(move || {
        let mut output_pipe = output_pipe;
        let mut buf = vec![0u8; 4096];
        loop {
            if exiting_clone.load(Ordering::SeqCst) {
                break;
            }
            match pty_reader.read(&mut buf) {
                Ok(0) => {
                    log::debug!("PTY EOF");
                    break;
                }
                Ok(n) => {
                    if let Err(e) = output_pipe.write_all(&buf[..n]) {
                        log::error!("Output pipe write error: {}", e);
                        break;
                    }
                    if let Err(e) = output_pipe.flush() {
                        log::error!("Output pipe flush error: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        log::error!("PTY read error: {}", e);
                        break;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
            }
        }
        log::debug!("Output thread exiting");
    });

    // Wait for child process to exit
    let exit_status = child.wait();
    log::info!("Shell exited: {:?}", exit_status);

    exiting.store(true, Ordering::SeqCst);

    // Wait for threads
    let _ = input_thread.join();
    let _ = output_thread.join();

    // pty_master will be dropped here, releasing PTY resources
    drop(pty_master);

    log::info!("Terminal helper exiting");
    Ok(())
}

/// Read exactly `buf.len()` bytes from reader.
/// Returns Ok(true) if successful, Ok(false) on EOF, Err on error.
pub(super) fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<bool> {
    let mut pos = 0;
    while pos < buf.len() {
        match reader.read(&mut buf[pos..]) {
            Ok(0) => return Ok(false), // EOF
            Ok(n) => pos += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(true)
}
