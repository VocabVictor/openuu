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
mod launch;
pub use launch::*;
mod helper_main;
pub use helper_main::*;
