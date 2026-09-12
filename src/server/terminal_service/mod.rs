use super::*;
use hbb_common::{
    anyhow::{anyhow, Context, Result},
    compress,
};
use portable_pty::{Child, CommandBuilder, PtySize};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

// Windows-specific imports from terminal_helper module
#[cfg(target_os = "windows")]
use super::terminal_helper::{
    configure_utf8_shell_command, create_named_pipe_server, encode_helper_message,
    encode_resize_message, is_helper_process_running, launch_terminal_helper_with_token,
    wait_for_pipe_connection, HelperProcessGuard, OwnedHandle, SendableHandle, WinCloseHandle,
    WinTerminateProcess, WinWaitForSingleObject, MSG_TYPE_DATA, PIPE_CONNECTION_TIMEOUT_MS,
    WIN_WAIT_OBJECT_0,
};

const MAX_OUTPUT_BUFFER_SIZE: usize = 1024 * 1024; // 1MB per terminal
const MAX_BUFFER_LINES: usize = 10000;
const MAX_SERVICES: usize = 100; // Maximum number of persistent terminal services
const SERVICE_IDLE_TIMEOUT: Duration = Duration::from_secs(3600); // 1 hour idle timeout
const CHANNEL_BUFFER_SIZE: usize = 500; // Channel buffer size. Max per-message size ~4KB (reader buffer), so worst case ~500*4KB ≈ 2MB/terminal. Increased from 100 to reduce data loss during disconnects.
const COMPRESS_THRESHOLD: usize = 512; // Compress terminal data larger than this
                                       // Default max bytes for reconnection buffer replay.
const DEFAULT_RECONNECT_BUFFER_BYTES: usize = 8 * 1024;
const MAX_SIGWINCH_PHASE_ATTEMPTS: u8 = 3; // Max attempts per SIGWINCH phase before giving up

/// Two-phase SIGWINCH trigger for TUI app redraw on reconnection.
///
/// Why two phases? A single resize-then-restore done back-to-back is too fast:
/// by the time the TUI app handles the asynchronous SIGWINCH signal and calls
/// `ioctl(TIOCGWINSZ)`, the PTY size has already been restored to the original.
/// ncurses sees no size change and skips the full redraw.
///
/// Splitting across two `read_outputs()` calls (~30ms apart) ensures the app
/// sees a real size change on each SIGWINCH, forcing a complete redraw.
#[derive(Debug, Clone)]
enum SigwinchPhase {
    /// No SIGWINCH needed.
    Idle,
    /// Phase 1: Resize PTY to temp dimensions (rows±1). The app handles SIGWINCH
    /// and redraws at the temporary size.
    TempResize { retries: u8 },
    /// Phase 2: Restore PTY to correct dimensions. The app handles SIGWINCH,
    /// detects the size change, and performs a full redraw at the correct size.
    Restore { retries: u8 },
}

/// Which resize to perform in the two-phase SIGWINCH sequence.
enum SigwinchAction {
    /// Phase 1: resize to temp dimensions (rows±1) to trigger SIGWINCH with a visible size change.
    TempResize,
    /// Phase 2: restore to correct dimensions to trigger SIGWINCH and force full redraw.
    Restore,
}

/// Session state machine for terminal streaming.
#[derive(Debug)]
enum SessionState {
    /// Session is closed, not streaming data to client.
    Closed,
    /// Session is active, streaming data to client.
    /// pending_buffer: historical buffer to send before real-time data (set on reconnection).
    /// sigwinch: two-phase SIGWINCH trigger state for TUI app redraw.
    Active {
        pending_buffer: Option<Vec<u8>>,
        sigwinch: SigwinchPhase,
    },
}

lazy_static::lazy_static! {
    // Global registry of persistent terminal services indexed by service_id
    static ref TERMINAL_SERVICES: Arc<Mutex<HashMap<String, Arc<Mutex<PersistentTerminalService>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Cleanup task handle
    static ref CLEANUP_TASK: Arc<Mutex<Option<std::thread::JoinHandle<()>>>> = Arc::new(Mutex::new(None));

    // List of terminal child processes to check for zombies
    static ref TERMINAL_TASKS: Arc<Mutex<Vec<Box<dyn Child + Send + Sync>>>> = Arc::new(Mutex::new(Vec::new()));
}

/// Service metadata that is sent to clients
#[derive(Clone, Debug)]
pub struct ServiceMetadata {
    pub service_id: String,
    pub created_at: Instant,
    pub terminal_count: usize,
    pub is_persistent: bool,
}

mod registry;
pub use registry::*;
mod service_entry;
pub use service_entry::*;
pub use service_entry::new;
mod output_buffer;
use output_buffer::*;
mod sessions;
pub use sessions::*;
mod proxy;
pub use proxy::*;
mod proxy_open;
mod proxy_open_helper;
mod proxy_io;
mod proxy_outputs;
#[cfg(test)]
mod tests;
