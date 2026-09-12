#[path = "auth.rs"]
mod ipc_auth;
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "fs.rs"]
mod ipc_fs;
// The DRM/KMS capture producer, the `_drm` channel and its SCM_RIGHTS framing live in their own
// module, declared the same way as the other pieces of this file, so the opt-in feature adds a
// bounded, self-contained surface here instead of ~1800 lines in the middle of the shared IPC.
#[cfg(all(target_os = "linux", feature = "drm"))]
#[path = "drm.rs"]
mod ipc_drm;
mod data_types;
mod data;
mod server;
mod handle;
mod client_connect;
mod connection;
mod config_calls;
mod options_calls;
mod misc_calls;
pub use data_types::*;
pub use data::*;
pub use server::*;
use handle::*;
pub use client_connect::*;
pub use connection::*;
pub use config_calls::*;
pub use options_calls::*;
pub use misc_calls::*;
// Re-exported so the paths callers already use (`crate::ipc::start_drm`, `crate::ipc::connect_drm`,
// `crate::ipc::DrmDisplayInfo`) keep working, and so the `Data` variants can name the two
// payload types.
#[cfg(all(target_os = "linux", feature = "drm"))]
pub use ipc_drm::{start_drm, DmabufDesc, DrmDisplayInfo};
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) use ipc_drm::DrmConn;
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) use ipc_drm::connect_drm;

use crate::{
    common::{is_server, CheckTestNatType},
    privacy_mode,
    privacy_mode::PrivacyModeState,
    rendezvous_mediator::RendezvousMediator,
    ui_interface::{get_local_option, set_local_option},
};
use bytes::Bytes;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use clipboard::ClipboardFile;
#[cfg(target_os = "linux")]
use hbb_common::anyhow;
use hbb_common::{
    allow_err, bail, bytes,
    bytes_codec::BytesCodec,
    config::{self, Config, Config2},
    futures::StreamExt as _,
    futures_util::sink::SinkExt,
    log, password_security as password, timeout,
    tokio::{
        self,
        io::{AsyncRead, AsyncWrite},
    },
    tokio_util::codec::Framed,
    ResultType,
};
use base::config::keys::{self, OPTION_ALLOW_WEBSOCKET};
#[cfg(windows)]
pub(crate) use ipc_auth::authorize_windows_portable_service_ipc_connection;
#[cfg(windows)]
pub(crate) use ipc_auth::ensure_peer_executable_matches_current_by_pid_opt;
#[cfg(windows)]
pub(crate) use ipc_auth::log_rejected_windows_ipc_connection;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use ipc_auth::{active_uid, authorize_service_scoped_ipc_connection};
#[cfg(target_os = "macos")]
use ipc_auth::authorize_user_server_process;
#[cfg(windows)]
use ipc_auth::{
    authorize_windows_main_ipc_connection, portable_service_listener_security_attributes,
    should_allow_everyone_create_on_windows,
};
#[cfg(target_os = "linux")]
pub(crate) use ipc_auth::{
    ensure_peer_executable_matches_current_by_fd, is_allowed_service_peer_uid,
    log_rejected_uinput_connection, peer_uid_from_fd,
};
#[cfg(target_os = "linux")]
use ipc_fs::terminal_count_candidate_uids;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use ipc_fs::{
    check_pid, ensure_secure_ipc_parent_dir, scrub_secure_ipc_parent_dir,
    should_scrub_parent_entries_after_check_pid, write_pid,
};
// Gated with the module that uses it, so a `drm`-less build does not carry an unused import.
#[cfg(all(target_os = "linux", feature = "drm"))]
use ipc_fs::remove_ipc_entry_via_secure_parent_fd;
use parity_tokio_ipc::{
    Connection as Conn, ConnectionClient as ConnClient, Endpoint, Incoming, SecurityAttributes,
};
use serde_derive::{Deserialize, Serialize};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::cell::Cell;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicBool, Ordering},
};

// IPC actions here.
pub const IPC_ACTION_CLOSE: &str = "close";
#[cfg(target_os = "windows")]
const PORTABLE_SERVICE_IPC_HANDSHAKE_TIMEOUT_MS: u64 = 3_000;
#[cfg(target_os = "windows")]
pub(crate) const IPC_TOKEN_LEN: usize = 64;
#[cfg(target_os = "windows")]
const IPC_TOKEN_RANDOM_BYTES: usize = IPC_TOKEN_LEN / 2;
#[cfg(target_os = "windows")]
const _: () = assert!(IPC_TOKEN_LEN % 2 == 0);
pub static EXIT_RECV_CLOSE: AtomicBool = AtomicBool::new(true);

#[cfg(any(target_os = "linux", target_os = "macos"))]
thread_local! {
    static USE_USER_MAIN_IPC: Cell<bool> = Cell::new(false);
}

#[must_use = "bind this guard to a local variable to keep the IPC scope active"]
/// Thread-local guard for routing root main IPC to the active user on Linux/macOS.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) struct UserMainIpcScope {
    previous: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl UserMainIpcScope {
    pub(crate) fn new() -> Self {
        let previous = USE_USER_MAIN_IPC.with(|use_user_main| {
            let previous = use_user_main.get();
            use_user_main.set(true);
            previous
        });
        Self { previous }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for UserMainIpcScope {
    fn drop(&mut self) {
        USE_USER_MAIN_IPC.with(|use_user_main| use_user_main.set(self.previous));
    }
}

#[inline]
pub async fn connect_service(ms_timeout: u64) -> ResultType<ConnectionTmpl<ConnClient>> {
    connect(ms_timeout, crate::POSTFIX_SERVICE).await
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_ffi_enum_data_size() {
        println!("{}", std::mem::size_of::<Data>());
        assert!(std::mem::size_of::<Data>() <= 120);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_service_ipc_path_is_shared_across_uids() {
        assert_eq!(
            Config::ipc_path_for_uid(0, crate::POSTFIX_SERVICE),
            Config::ipc_path_for_uid(501, crate::POSTFIX_SERVICE)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_ipc_path_differs_by_uid_for_cm() {
        let effective_uid = unsafe { hbb_common::libc::geteuid() as u32 };
        let other_uid = effective_uid.saturating_add(1);
        let postfix = "_cm";

        // Default connect path targets the current effective uid.
        assert_eq!(
            Config::ipc_path(postfix),
            Config::ipc_path_for_uid(effective_uid, postfix)
        );
        // A different uid yields a different socket path - this is the root cause of the
        // cross-user regression when root spawns a user process but still connects as uid 0.
        assert_ne!(
            Config::ipc_path(postfix),
            Config::ipc_path_for_uid(other_uid, postfix)
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_uses_active_uid_when_no_server_found() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[], Some(501), false).unwrap(),
            501
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_uses_single_server_uid() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[501], None, false).unwrap(),
            501
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_prefers_active_uid_with_multiple_servers() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[0, 501], Some(501), false).unwrap(),
            501
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_prefers_root_on_wayland_login_screen() {
        assert_eq!(
            select_server_uid_for_user_main_ipc(&[0, 501], Some(501), true).unwrap(),
            0
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_select_server_uid_fails_when_multiple_servers_are_ambiguous() {
        assert!(select_server_uid_for_user_main_ipc(&[501, 502], None, false).is_err());
    }
}
