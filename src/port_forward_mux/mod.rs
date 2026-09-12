use hbb_common::{
    bytes::Bytes,
    log,
    tokio::{
        self,
        io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
        sync::{mpsc, watch, Notify},
        time::Instant,
    },
    ResultType,
};
use base::message_proto::*;
use std::sync::{Arc, Mutex};

mod channel;
pub use channel::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod tunnel;
#[cfg(test)]
mod tests;

/// On the wire and fixed forever: what the controller may have in flight on a
/// channel before `opened` brings the peer's window.
pub const INITIAL_WINDOW: u32 = 64 * 1024;
/// Also on the wire and fixed forever: the window a `data` frame costs at
/// minimum, whatever its length. It bounds the per-frame bookkeeping a peer
/// can make us hold — 1-byte frames would otherwise cost it one byte and us
/// a queue entry.
pub const MIN_FRAME_CHARGE: u32 = 64;
pub const CHANNEL_WINDOW: u32 = 256 * 1024;
pub const MAX_FRAME: usize = 64 * 1024;
/// Cap on one framed packet once the tunnel is up: a `MAX_FRAME` data frame,
/// its protobuf envelope and the 16-byte MAC fit with room to spare. The codec
/// otherwise takes a header declaring up to 1 GiB, and the channel window is
/// only checked once the whole packet has arrived.
pub const MAX_PACKET: usize = 2 * MAX_FRAME;
pub const UPDATE_THRESHOLD: u32 = CHANNEL_WINDOW / 2;
pub const MAX_CHANNELS: usize = 256;
pub const DATA_QUEUE_FRAMES: usize = 128;
/// Never keep more than our own advertised window in flight, whatever the peer
/// offers. The controlled side's sink is unbounded, so credit is the only bound
/// on how much target data it buffers, and the peer chooses that number.
pub const MAX_SEND_CREDIT: u32 = CHANNEL_WINDOW;

pub fn effective_window(advertised: u32) -> u32 {
    advertised.max(INITIAL_WINDOW)
}

/// For a tunnel's stream once multiplexing is agreed, on both sides. The
/// WebSocket and WebRTC codecs carry caps of their own.
pub fn cap_packet_size(stream: &mut hbb_common::Stream) {
    if let hbb_common::Stream::Tcp(s) = stream {
        s.0.codec_mut().set_max_packet_length(MAX_PACKET);
    }
}

/// What a `data` frame of this length costs its channel's window.
pub fn charge(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX).max(MIN_FRAME_CHARGE)
}

/// Receiver-side accounting: the credit the peer still has, and what we have
/// drained to the local socket since the last `window_update`. Both are
/// bounded — a cumulative counter would wear out on a long transfer.
pub struct RecvWindow {
    remaining: u32,
    drained_since_update: u32,
}

impl RecvWindow {
    pub fn new(granted: u32) -> Self {
        Self {
            remaining: effective_window(granted),
            drained_since_update: 0,
        }
    }

    /// False means the peer overran the window: a protocol violation.
    pub fn accept(&mut self, len: usize) -> bool {
        match self.remaining.checked_sub(charge(len)) {
            Some(left) => {
                self.remaining = left;
                true
            }
            None => false,
        }
    }

    /// Widens the window without advertising: the grant travels in `opened`.
    pub fn grant(&mut self, n: u32) {
        self.remaining = self.remaining.saturating_add(n);
    }

    #[cfg(test)]
    pub fn remaining(&self) -> u32 {
        self.remaining
    }

    /// Returns the amount to advertise in a `window_update` once enough has
    /// been drained; the same amount is credited back.
    pub fn drained(&mut self, n: usize) -> Option<u32> {
        self.drained_since_update = self.drained_since_update.saturating_add(charge(n));
        if self.drained_since_update < UPDATE_THRESHOLD {
            return None;
        }
        let add = self.drained_since_update;
        self.drained_since_update = 0;
        self.remaining = self.remaining.saturating_add(add);
        Some(add)
    }
}

/// Sender-side credit. `take` parks until credit is available; the lock is
/// never held across an await.
pub struct SendCredit {
    credit: Mutex<u32>,
    notify: Notify,
}

impl SendCredit {
    pub fn new(initial: u32) -> Self {
        Self {
            credit: Mutex::new(initial.min(MAX_SEND_CREDIT)),
            notify: Notify::new(),
        }
    }

    /// `max` must be at least `MIN_FRAME_CHARGE` (`MAX_FRAME` is), and the
    /// caller pays `charge(bytes_read)` and refunds the rest — so this parks
    /// until a whole minimum charge is available rather than at zero.
    pub async fn take(&self, max: usize) -> usize {
        debug_assert!(max >= MIN_FRAME_CHARGE as usize);
        loop {
            {
                let mut credit = self.credit.lock().unwrap();
                if *credit >= MIN_FRAME_CHARGE {
                    let n = (*credit as usize).min(max);
                    *credit -= n as u32;
                    return n;
                }
            }
            self.notify.notified().await;
        }
    }

    pub fn add(&self, n: u32) {
        {
            let mut credit = self.credit.lock().unwrap();
            *credit = credit.saturating_add(n).min(MAX_SEND_CREDIT);
        }
        self.notify.notify_one();
    }

    /// The controller starts a channel with `INITIAL_WINDOW` of credit; when
    /// `opened` advertises the peer's real window this re-bases to it.
    pub fn raise_initial(&self, total: u32) {
        let extra = effective_window(total) - INITIAL_WINDOW;
        if extra > 0 {
            self.add(extra);
        }
    }
}

fn channel_msg(union: port_forward_channel::Union) -> Message {
    let mut ch = PortForwardChannel::new();
    ch.union = Some(union);
    let mut msg = Message::new();
    msg.set_port_forward_channel(ch);
    msg
}

pub fn open_msg(id: i32, host: &str, port: i32, window: u32) -> Message {
    channel_msg(port_forward_channel::Union::Open(PortForwardOpen {
        channel_id: id,
        host: host.to_owned(),
        port,
        window,
        ..Default::default()
    }))
}

pub fn opened_msg(id: i32, success: bool, message: &str, window: u32) -> Message {
    channel_msg(port_forward_channel::Union::Opened(PortForwardOpened {
        channel_id: id,
        success,
        message: message.to_owned(),
        window,
        ..Default::default()
    }))
}

pub fn data_msg(id: i32, data: Bytes) -> Message {
    channel_msg(port_forward_channel::Union::Data(PortForwardData {
        channel_id: id,
        data,
        ..Default::default()
    }))
}

pub fn close_msg(id: i32) -> Message {
    channel_msg(port_forward_channel::Union::Close(PortForwardClose {
        channel_id: id,
        ..Default::default()
    }))
}

pub fn window_update_msg(id: i32, add: u32) -> Message {
    channel_msg(port_forward_channel::Union::WindowUpdate(PortForwardWindowUpdate {
        channel_id: id,
        add,
        ..Default::default()
    }))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use tunnel::{Claim, Tunnel};


