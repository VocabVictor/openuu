use super::connection::{Connection, Sender};
use crate::port_forward_mux::{
    charge, close_msg, effective_window, opened_msg, run_channel, FrameSink, Inbound, RecvWindow,
    SendCredit, CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS,
};
use hbb_common::{
    bytes::Bytes,
    log,
    timeout,
    tokio::{self, net::TcpStream, sync::{mpsc, watch}},
};
use base::message_proto::*;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

const CONNECT_TIMEOUT_MS: u64 = 3000;

/// Before `opened` the controller may only have used `INITIAL_WINDOW`.
/// `charged` is the running total of `charge(len)`, not of raw lengths.
fn pending_fits(charged: usize, add_len: usize) -> bool {
    charged.saturating_add(charge(add_len) as usize) <= INITIAL_WINDOW as usize
}

struct Entry {
    inbound: mpsc::UnboundedSender<Inbound>,
    credit: Arc<SendCredit>,
    window: Arc<Mutex<RecvWindow>>,
}

/// The controlled side of one multiplexed tunnel. The main loop owns it and
/// forwards every `PortForwardChannel` frame here; each channel is a task.

mod mux;
mod channel;
use channel::*;
pub struct PortForwardMux {
    channels: HashMap<i32, Entry>,
    tx: Sender,
    login_target: String,
    /// Raised once, by `close_all`, for the channels its `clear` cannot reach:
    /// one parked on its target socket is not on the inbound queue.
    teardown: watch::Sender<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::port_forward_mux::{CHANNEL_WINDOW, INITIAL_WINDOW, MAX_CHANNELS, MIN_FRAME_CHARGE};
    use hbb_common::{
        tokio::{
            self,
            io::{AsyncReadExt, AsyncWriteExt},
            net::TcpListener,
            sync::mpsc,
            time::Instant,
        },
    };
    use base::message_proto::{message, port_forward_channel};

    mod helpers;
    use helpers::*;
    mod channel_tests;
    mod cap_tests;

}
