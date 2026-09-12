use super::*;

/// Information about a launched helper process.
/// Contains both the process handle and PID for tracking and status checks.
#[derive(Debug)]
pub struct HelperProcessInfo {
    /// Process handle for termination and waiting
    pub handle: HANDLE,
    /// Process ID for logging and status display
    pub pid: u32,
}

/// Wrapper for Windows HANDLE that implements Send.
/// This is safe because Windows HANDLEs are valid across threads.
/// Note: We only implement Send, not Sync. The handle is protected by
/// Mutex in TerminalSession, so concurrent access is controlled there.
///
/// # Ownership and Cleanup
/// This type intentionally does NOT implement Drop. The handle is owned by
/// `TerminalSession` and explicitly closed in `TerminalSession::close_internal()`
/// after graceful shutdown logic (waiting for helper to exit, force termination if needed).
/// Implementing Drop here would interfere with that cleanup sequence.
#[derive(Debug)]
pub struct SendableHandle(HANDLE);

impl SendableHandle {
    /// Create a new SendableHandle from a raw HANDLE.
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    /// Get the raw HANDLE value.
    pub fn as_raw(&self) -> HANDLE {
        self.0
    }
}

unsafe impl Send for SendableHandle {}

/// RAII wrapper for Windows HANDLE that automatically closes the handle on drop.
/// This ensures proper resource cleanup even when errors occur or code paths diverge.
pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    /// Create a new OwnedHandle from a raw HANDLE.
    /// The handle will be closed when this OwnedHandle is dropped.
    pub fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    /// Consume the OwnedHandle and return the raw HANDLE without closing it.
    /// Use this when transferring ownership to another resource (e.g., File).
    pub fn into_raw(self) -> HANDLE {
        let handle = self.0;
        std::mem::forget(self); // Prevent Drop from closing the handle
        handle
    }

    /// Get the raw HANDLE value.
    pub fn as_raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// RAII guard for helper process that terminates the process on drop.
/// This prevents helper process leaks when pipe connection fails or other errors occur.
///
/// Unlike OwnedHandle (which only closes the handle), this guard:
/// 1. Terminates the process using TerminateProcess
/// 2. Then closes the handle
///
/// Use `disarm()` to prevent termination when the helper is successfully handed off
/// to the terminal session for proper lifecycle management.
pub struct HelperProcessGuard {
    pub(super) handle: HANDLE,
    pub(super) pid: u32,
    pub(super) armed: bool,
}

impl HelperProcessGuard {
    /// Create a new guard for a helper process.
    pub fn new(handle: HANDLE, pid: u32) -> Self {
        Self {
            handle,
            pid,
            armed: true,
        }
    }

    /// Get the raw process HANDLE.
    pub fn as_raw(&self) -> HANDLE {
        self.handle
    }

    /// Get the process ID.
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// Disarm the guard and return the raw HANDLE.
    /// After calling this, the guard will NOT terminate the process on drop.
    /// Use this when successfully handing off the helper to session management.
    pub fn disarm(self) -> HANDLE {
        let handle = self.handle;
        std::mem::forget(self); // Prevent Drop from running
        handle
    }
}

impl Drop for HelperProcessGuard {
    fn drop(&mut self) {
        if self.armed && self.handle != INVALID_HANDLE_VALUE && !self.handle.is_invalid() {
            log::warn!(
                "HelperProcessGuard: terminating leaked helper process (PID {})",
                self.pid
            );
            unsafe {
                // Terminate the process first
                let _ = WinTerminateProcess(self.handle, 1);
                // Then close the handle
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
