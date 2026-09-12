//! Terminal Helper Process
//!
//! This module implements a helper process that runs as the logged-in user and creates
//! the ConPTY + Shell. This is necessary because ConPTY has compatibility issues with
//! CreateProcessAsUserW when the ConPTY is created by a different user (SYSTEM service).
//!
//! Architecture:
//! ```
//! SYSTEM Service (terminal_service.rs)
//!     |
//!     +-- CreateProcessAsUserW --> Terminal Helper (this module, runs as user)
//!     |                                |
//!     |                                +-- CreateProcessW + ConPTY --> Shell
//!     |                                |
//!     +-- Named Pipes <----------------+
//! ```
//!
//! This module also contains Windows-specific utility functions used by terminal_service.rs:
//! - Named pipe creation and connection
//! - User token and SID handling
//! - Helper process launching

use hbb_common::{
    anyhow::{anyhow, Context, Result},
    log,
};
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::{
    ffi::{c_void, OsStr},
    fs::File,
    io::{Read, Write},
    os::windows::{ffi::OsStrExt, io::FromRawHandle, raw::HANDLE as RawHandle},
    ptr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            CloseHandle, LocalFree, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE, HLOCAL,
            INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
        },
        Security::{
            Authorization::{
                SetEntriesInAclW, EXPLICIT_ACCESS_W, SET_ACCESS, TRUSTEE_IS_SID, TRUSTEE_IS_USER,
                TRUSTEE_W,
            },
            CreateWellKnownSid, GetLengthSid, GetTokenInformation, InitializeSecurityDescriptor,
            SetSecurityDescriptorDacl, TokenUser, WinLocalSystemSid, ACE_FLAGS, ACL,
            PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, TOKEN_USER,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ALL_ACCESS, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED,
            FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        },
        System::{
            Environment::{CreateEnvironmentBlock, DestroyEnvironmentBlock},
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT,
            },
            Threading::{
                CreateEventW, CreateProcessAsUserW, WaitForSingleObject, CREATE_NO_WINDOW,
                CREATE_UNICODE_ENVIRONMENT, PROCESS_CREATION_FLAGS, PROCESS_INFORMATION,
                STARTUPINFOW,
            },
            IO::{GetOverlappedResult, OVERLAPPED},
        },
    },
};

// Re-export types needed by terminal_service.rs
pub use windows::Win32::{
    Foundation::{
        CloseHandle as WinCloseHandle, HANDLE as WinHANDLE, WAIT_OBJECT_0 as WIN_WAIT_OBJECT_0,
    },
    System::Threading::{
        GetExitCodeProcess as WinGetExitCodeProcess, TerminateProcess as WinTerminateProcess,
        WaitForSingleObject as WinWaitForSingleObject,
    },
};

/// User token wrapper for cross-module use.
///
/// Using newtype pattern for type safety. The inner value is `usize` to match
/// platform pointer size (32-bit on x86, 64-bit on x64).
/// Windows HANDLE is defined as `*mut c_void`, which has the same size as `usize`.
///
/// # Design Note
/// This type is defined here (terminal_helper.rs) for Windows and in
/// terminal_service.rs for non-Windows platforms. This avoids circular
/// dependencies while keeping the API consistent across platforms.
/// Both definitions MUST have identical public API (new, as_raw methods).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserToken(pub usize);

impl UserToken {
    /// Create a new UserToken from a raw handle value.
    pub fn new(handle: usize) -> Self {
        Self(handle)
    }

    /// Get the raw handle value.
    pub fn as_raw(&self) -> usize {
        self.0
    }
}

// Windows pipe access mode constants (not exported by windows crate)
const PIPE_ACCESS_INBOUND: u32 = 0x00000001;
const PIPE_ACCESS_OUTBOUND: u32 = 0x00000002;

// Named pipe configuration constants
const PIPE_BUFFER_SIZE: u32 = 65536; // 64KB for better throughput with large terminal output
const PIPE_DEFAULT_TIMEOUT_MS: u32 = 5000;
/// Timeout for waiting for helper process to connect to pipes
pub const PIPE_CONNECTION_TIMEOUT_MS: u32 = 10000;

/// Message type constants for helper protocol.
/// Used to distinguish between terminal data and control commands.
/// Note: Using non-zero values to make debugging easier (0x00 could indicate uninitialized memory).
pub const MSG_TYPE_DATA: u8 = 0x01;
pub const MSG_TYPE_RESIZE: u8 = 0x02;

/// Message header size: 1 byte type + 4 bytes length
pub const MSG_HEADER_SIZE: usize = 5;

/// Maximum payload size to prevent denial of service from malicious messages.
/// 16MB should be more than enough for any legitimate terminal data.
const MAX_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;

/// Timeout in milliseconds to wait for helper process to exit gracefully before force termination.
/// Using 500ms to allow helper process enough time to clean up, especially under high system load.
pub const HELPER_GRACEFUL_EXIT_TIMEOUT_MS: u64 = 500;

mod handles;
pub use handles::*;
mod messages;
pub use messages::*;
mod shell;
pub use shell::*;
mod security;
pub use security::*;
mod pipe;
pub use pipe::*;

/// Launch terminal helper process as the logged-in user using the provided token.
/// The helper process creates ConPTY and shell, communicating via named pipes.
/// This uses CreateProcessAsUserW directly with the user token, which works because
/// the helper process itself doesn't need ConPTY - it creates ConPTY internally.
///
/// Returns HelperProcessInfo containing the process handle and PID.

/// RAII guard for environment block cleanup.
/// Ensures DestroyEnvironmentBlock is called even if an error occurs.
struct EnvironmentBlockGuard {
    ptr: *mut c_void,
}

impl Drop for EnvironmentBlockGuard {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                // Ignore result: DestroyEnvironmentBlock failure is non-critical during cleanup
                let _ = DestroyEnvironmentBlock(self.ptr);
            }
        }
    }
}

pub fn launch_terminal_helper_with_token(
    user_token: UserToken,
    input_pipe_name: &str,
    output_pipe_name: &str,
    terminal_id: i32,
    rows: u16,
    cols: u16,
) -> Result<HelperProcessInfo> {
    let exe_path =
        std::env::current_exe().map_err(|e| anyhow!("Failed to get current exe path: {}", e))?;

    // Build command line arguments (without exe path to avoid escaping issues)
    // lpApplicationName will contain the exe path separately
    let cmd_args = format!(
        "--terminal-helper {} {} {} {} {}",
        input_pipe_name, output_pipe_name, rows, cols, terminal_id
    );

    log::debug!("Launching terminal helper for terminal {}", terminal_id);

    // Convert exe path to wide string for lpApplicationName
    let exe_path_wide: Vec<u16> = OsStr::new(exe_path.as_os_str())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Command line must include exe name as first argument per Windows convention
    let cmd_line = format!("\"{}\" {}", exe_path.display(), cmd_args);
    let mut cmd_wide: Vec<u16> = OsStr::new(&cmd_line)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut si: STARTUPINFOW = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

    let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };

    // Create environment block for the user with RAII cleanup
    let mut environment: *mut c_void = ptr::null_mut();
    let env_ok = unsafe {
        CreateEnvironmentBlock(
            &mut environment,
            Some(HANDLE(user_token.as_raw() as _)),
            true,
        )
    }
    .is_ok();

    // Use RAII guard to ensure cleanup even on error paths
    let _env_guard = if env_ok && !environment.is_null() {
        Some(EnvironmentBlockGuard { ptr: environment })
    } else {
        if !env_ok {
            log::warn!("Failed to create environment block, using default");
        }
        None
    };

    let creation_flags = CREATE_NO_WINDOW
        | if env_ok {
            CREATE_UNICODE_ENVIRONMENT
        } else {
            PROCESS_CREATION_FLAGS(0)
        };

    // Use lpApplicationName to pass exe path separately from command line
    // This avoids potential issues with special characters in the exe path
    let result = unsafe {
        CreateProcessAsUserW(
            Some(HANDLE(user_token.as_raw() as _)),
            PCWSTR::from_raw(exe_path_wide.as_ptr()), // lpApplicationName: exe path
            Some(PWSTR::from_raw(cmd_wide.as_mut_ptr())), // lpCommandLine: full command
            None,
            None,
            false, // Don't inherit handles
            creation_flags,
            if env_ok { Some(environment) } else { None },
            PCWSTR::null(), // Use default current directory
            &si,
            &mut pi,
        )
    };

    // Environment block cleanup is handled by _env_guard's Drop

    if let Err(e) = result {
        log::error!("CreateProcessAsUserW failed: {}", e);
        return Err(anyhow!("Failed to launch terminal helper: {}", e));
    }

    // Close thread handle - we only need the process handle for tracking
    // Ignore result: CloseHandle failure here is non-critical since process is already launched
    unsafe {
        let _ = CloseHandle(pi.hThread);
    }

    log::info!("Terminal helper launched with PID {}", pi.dwProcessId);
    // Return process info for tracking
    Ok(HelperProcessInfo {
        handle: pi.hProcess,
        pid: pi.dwProcessId,
    })
}

/// Check if a helper process is still running.
/// Returns true if the process is running, false if it has exited.
pub fn is_helper_process_running(handle: HANDLE) -> bool {
    let wait_result = unsafe { WaitForSingleObject(handle, 0) };
    // WAIT_TIMEOUT (258) means process is still running
    // WAIT_OBJECT_0 (0) means process has exited
    wait_result != WAIT_OBJECT_0
}

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
    let mut input_pipe = open_pipe(input_pipe_name, true)?;
    let mut output_pipe = open_pipe(output_pipe_name, false)?;

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
fn read_exact_or_eof<R: Read>(reader: &mut R, buf: &mut [u8]) -> std::io::Result<bool> {
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
