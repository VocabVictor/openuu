// The DRM/KMS capture half of the `_drm` IPC channel: types, root-service producer, framing.

use super::ipc_auth::active_uid_cached;
use super::*;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

mod channel_types;
pub use channel_types::*;
mod display_cache;
use display_cache::*;
mod wake;
use wake::*;
mod refresh;
use refresh::*;
mod start;
pub use start::*;
mod conn_handler;
use conn_handler::*;
mod capture_worker;
use capture_worker::*;
mod framing;
pub(crate) use framing::*;
mod conn_impl;

#[cfg(test)]
mod drm_conn_tests {
    use super::*;
    use hbb_common::libc;
    use hbb_common::tokio::{self, io::AsyncWriteExt};
    use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

    mod tests_a;
    use tests_a::*;
    mod tests_b;

}
