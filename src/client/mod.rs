#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::clipboard::clipboard_listener;
use async_trait::async_trait;
use bytes::Bytes;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use clipboard_master::CallbackResult;
#[cfg(not(target_os = "linux"))]
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, StreamConfig,
};
use crossbeam_queue::ArrayQueue;
use magnum_opus::{Channels::*, Decoder as AudioDecoder};
#[cfg(not(target_os = "linux"))]
use ringbuf::{ring_buffer::RbBase, Rb};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::c_void,
    net::SocketAddr,
    ops::Deref,
    str::FromStr,
    sync::{
        mpsc::{self, RecvTimeoutError},
        Arc, Mutex, RwLock,
    },
};
use uuid::Uuid;

use crate::{
    check_port,
    common::input::{MOUSE_BUTTON_LEFT, MOUSE_BUTTON_RIGHT, MOUSE_TYPE_DOWN, MOUSE_TYPE_UP},
    create_symmetric_key_msg, decode_id_pk, decode_id_pk_dtls, get_rs_pk, is_keyboard_mode_supported,
    kcp_stream::KcpStream,
    secure_tcp,
    ui_interface::{get_builtin_option, resolve_avatar_url, use_texture_render},
    ui_session_interface::{InvokeUiSession, Session},
};
#[cfg(feature = "unix-file-copy-paste")]
use crate::{clipboard::check_clipboard_files, clipboard_file::unix_file_clip};
pub use file_trait::FileManager;
use hbb_common::{
    allow_err,
    anyhow::{anyhow, Context},
    bail,
    config::{
        self, use_ws, Config, LocalConfig, PeerConfig, PeerInfoSerde, Resolution,
        CONNECT_TIMEOUT, READ_TIMEOUT, RELAY_PORT, RENDEZVOUS_PORT, RENDEZVOUS_SERVERS,
    },
    futures::future::{select_ok, BoxFuture, FutureExt},
    get_version_number, log,
    protobuf::{Message as _, MessageField},
    rand,
    rendezvous_proto::*,
    sha2::{Digest, Sha256},
    socket_client::{connect_tcp, connect_tcp_local, ipv4_to_ipv6, new_direct_udp_for},
    sodiumoxide::{base64, crypto::sign},
    timeout,
    tokio::{
        self,
        net::UdpSocket,
        sync::{
            mpsc::{error::TryRecvError, unbounded_channel, UnboundedReceiver},
            oneshot,
        },
        time::{interval, Duration, Instant},
    },
    webrtc::WebRTCStream,
    AddrMangle, ResultType, Stream,
};
use base::{
    config::keys,
    fs::JobType,
    message_proto::{option_message::BoolOption, *},
};
pub use helper::*;
use scrap::{
    codec::Decoder,
    record::{Recorder, RecorderContext},
    CodecFormat, ImageFormat, ImageRgb, ImageTexture,
};

#[cfg(not(target_os = "ios"))]
use crate::clipboard::CLIPBOARD_INTERVAL;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::clipboard::{check_clipboard, ClipboardSide};

pub use super::lang::*;

#[cfg(not(target_os = "linux"))]
mod audio_playback;
#[cfg(all(test, not(target_os = "linux")))]
mod audio_state_tests;
pub mod file_trait;
pub mod helper;
pub mod io_loop;
pub mod screenshot;

pub const MILLI1: Duration = Duration::from_millis(1);
pub const SEC30: Duration = Duration::from_secs(30);
// Empirical restart reconnect grace window.
const RESTART_REMOTE_DEVICE_GRACE: Duration = Duration::from_secs(5 * 60);
pub const VIDEO_QUEUE_SIZE: usize = 120;
const MAX_DECODE_FAIL_COUNTER: usize = 3;

pub const LOGIN_MSG_PASSWORD_EMPTY: &str = "Empty Password";
pub const LOGIN_MSG_PASSWORD_WRONG: &str = "Wrong Password";
pub const LOGIN_MSG_2FA_WRONG: &str = "Wrong 2FA Code";
pub const REQUIRE_2FA: &'static str = "2FA Required";
pub const LOGIN_MSG_NO_PASSWORD_ACCESS: &str = "No Password Access";
pub const LOGIN_MSG_OFFLINE: &str = "Offline";
pub const LOGIN_SCREEN_WAYLAND: &str = "Wayland login screen is not supported";
#[cfg(target_os = "linux")]
pub const SCRAP_UBUNTU_HIGHER_REQUIRED: &str = "ubuntu-21-04-required";
#[cfg(target_os = "linux")]
pub const SCRAP_OTHER_VERSION_OR_X11_REQUIRED: &str =
    "wayland-requires-higher-linux-version";
#[cfg(target_os = "linux")]
pub const SCRAP_XDP_PORTAL_UNAVAILABLE: &str =
    "xdp-portal-unavailable";
pub const SCRAP_X11_REQUIRED: &str = "x11 expected";
pub const SCRAP_X11_REF_URL: &str = "https://rustdesk.com/docs/en/manual/linux/#x11-required";

#[cfg(not(target_os = "linux"))]
pub const AUDIO_BUFFER_MS: usize = 3000;

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) struct ClientClipboardContext;

/// Client of the remote desktop.
pub struct Client;

#[cfg(not(target_os = "ios"))]
struct ClipboardState {
    #[cfg(feature = "flutter")]
    is_text_required: bool,
    #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
    is_file_required: bool,
    running: bool,
}

#[cfg(not(target_os = "linux"))]
lazy_static::lazy_static! {
    static ref AUDIO_HOST: Host = cpal::default_host();
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
lazy_static::lazy_static! {
    static ref ENIGO: Arc<Mutex<enigo::Enigo>> = Arc::new(Mutex::new(enigo::Enigo::new()));
}

#[cfg(not(target_os = "ios"))]
lazy_static::lazy_static! {
    static ref CLIPBOARD_STATE: Arc<Mutex<ClipboardState>> = Arc::new(Mutex::new(ClipboardState::new()));
}

const PUBLIC_SERVER: &str = "public";

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_key_state(key: enigo::Key) -> bool {
    use enigo::KeyboardControllable;
    #[cfg(target_os = "macos")]
    if key == enigo::Key::NumLock {
        return true;
    }
    ENIGO.lock().unwrap().get_key_state(key)
}

mod transport;
use transport::*;
mod start;
mod webrtc_bridge;
mod start_inner;
mod connect;
mod secure;
mod clipboard_sync;
mod audio_handler;
pub use audio_handler::*;
mod audio_buffer;
use audio_buffer::*;
mod video;
pub use video::*;
mod login_config;
pub use login_config::*;
mod login_options;
mod login_display;
mod login_peer;
mod login_msg;
mod media;
pub use media::*;
mod input;
pub use input::*;
mod login_error;
pub use login_error::*;
mod login_hash;
pub use login_hash::*;
mod interface;
pub use interface::*;
mod key_map;
pub use key_map::*;
mod retry;
pub use retry::*;
mod hc_connection;
pub use hc_connection::*;
pub mod peer_online;
mod udp_nat;
use udp_nat::*;
#[cfg(test)]
mod webrtc_race_tests;
#[cfg(test)]
mod view_only_session_tests;

