//! Behaviour of `Remote::handle_msg_from_peer`, one message family per file.

use super::test_support::*;
use super::*;

mod misc_arms;
mod session;

/// Feed one message to the remote and return whether the loop keeps running.
async fn feed(parts: &mut RemoteTestParts, msg: &Message) -> bool {
    parts
        .remote
        .handle_msg_from_peer(&bytes_of(msg), &mut parts.peer)
        .await
}
