use std::{
    collections::HashMap,
    future::Future,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
    task::Poll,
};

use serde_json::{json, Map, Value};

use base::{config::keys, message_proto::*};
#[cfg(not(target_os = "ios"))]
use hbb_common::whoami;
use hbb_common::{
    allow_err,
    anyhow::{anyhow, Context},
    async_recursion::async_recursion,
    bail, base64,
    bytes::Bytes,
    config::{self, use_ws, Config, LocalConfig, CONNECT_TIMEOUT, READ_TIMEOUT, RENDEZVOUS_PORT},
    futures::future::join_all,
    futures_util::future::poll_fn,
    get_version_number, log,
    protobuf::{Enum, Message as _},
    rendezvous_proto::*,
    socket_client,
    sodiumoxide::crypto::{box_, secretbox, sign},
    timeout,
    tls::{get_cached_tls_accept_invalid_cert, get_cached_tls_type, upsert_tls_cache, TlsType},
    tokio::{
        self,
        net::UdpSocket,
        time::{Duration, Instant, Interval},
    },
    ResultType, Stream,
};

use crate::{
    hbbs_http::{create_http_client_async, get_url_for_tls},
    ui_interface::{get_api_server as ui_get_api_server, get_option, is_installed, set_option},
};

mod process;
mod version;
mod key_utils;
mod audio;
mod audio_rechannel;
mod nat;
mod server_url;
mod app;
mod tcp_proxy;
mod post;
mod http_request;
mod messages;
mod crypto;
mod interval;
mod custom_client;
mod ipv6;
mod udp_punch;
pub use {process::*, version::*, key_utils::*, audio::*, audio_rechannel::*, nat::*, server_url::*, app::*, post::*, http_request::*, messages::*, crypto::*, interval::*, custom_client::*, ipv6::*, udp_punch::*};

#[derive(Debug, Eq, PartialEq)]
pub enum GrabState {
    Ready,
    Run,
    Wait,
    Exit,
}

pub type NotifyMessageBox = fn(String, String, String, String) -> dyn Future<Output = ()>;

// the executable name of the portable version
pub const PORTABLE_APPNAME_RUNTIME_ENV_KEY: &str = "RUSTDESK_APPNAME";

pub const PLATFORM_WINDOWS: &str = "Windows";
pub const PLATFORM_LINUX: &str = "Linux";
pub const PLATFORM_MACOS: &str = "Mac OS";
pub const PLATFORM_ANDROID: &str = "Android";

pub const TIMER_OUT: Duration = Duration::from_secs(1);
pub const DEFAULT_KEEP_ALIVE: i32 = 60_000;

const MIN_VER_MULTI_UI_SESSION: &str = "1.2.4";

pub mod input {
    pub const MOUSE_TYPE_MOVE: i32 = 0;
    pub const MOUSE_TYPE_DOWN: i32 = 1;
    pub const MOUSE_TYPE_UP: i32 = 2;
    pub const MOUSE_TYPE_WHEEL: i32 = 3;
    pub const MOUSE_TYPE_TRACKPAD: i32 = 4;
    /// Relative mouse movement type for gaming/3D applications.
    /// This type sends delta (dx, dy) values instead of absolute coordinates.
    /// NOTE: This is only supported by the Flutter client. The Sciter client (deprecated)
    /// does not support relative mouse mode due to:
    /// 1. Fixed send_mouse() function signature that doesn't allow type differentiation
    /// 2. Lack of pointer lock API in Sciter/TIS
    /// 3. No OS cursor control (hide/show/clip) FFI bindings in Sciter UI
    pub const MOUSE_TYPE_MOVE_RELATIVE: i32 = 5;

    /// Mask to extract the mouse event type from the mask field.
    /// The lower 3 bits contain the event type (MOUSE_TYPE_*), giving a valid range of 0-7.
    /// Currently defined types use values 0-5; values 6 and 7 are reserved for future use.
    pub const MOUSE_TYPE_MASK: i32 = 0x7;

    pub const MOUSE_BUTTON_LEFT: i32 = 0x01;
    pub const MOUSE_BUTTON_RIGHT: i32 = 0x02;
    pub const MOUSE_BUTTON_WHEEL: i32 = 0x04;
    pub const MOUSE_BUTTON_BACK: i32 = 0x08;
    pub const MOUSE_BUTTON_FORWARD: i32 = 0x10;
}

lazy_static::lazy_static! {
    pub static ref SOFTWARE_UPDATE_URL: Arc<Mutex<String>> = Default::default();
    pub static ref DEVICE_ID: Arc<Mutex<String>> = Default::default();
    pub static ref DEVICE_NAME: Arc<Mutex<String>> = Default::default();
    static ref PUBLIC_IPV6_ADDR: Arc<Mutex<(Option<SocketAddr>, Option<Instant>)>> = Default::default();
}

lazy_static::lazy_static! {
    // Is server process, with "--server" args
    static ref IS_SERVER: bool = std::env::args().nth(1) == Some("--server".to_owned());
    // Is server logic running. The server code can invoked to run by the main process if --server is not running.
    static ref SERVER_RUNNING: Arc<RwLock<bool>> = Default::default();
    static ref IS_MAIN: bool = std::env::args().nth(1).map_or(true, |arg| !arg.starts_with("--"));
    static ref IS_CM: bool = std::env::args().nth(1) == Some("--cm".to_owned());
}

pub struct SimpleCallOnReturn {
    pub b: bool,
    pub f: Box<dyn Fn() + Send + 'static>,
}

impl Drop for SimpleCallOnReturn {
    fn drop(&mut self) {
        if self.b {
            (self.f)();
        }
    }
}

pub fn global_init() -> bool {
    #[cfg(all(target_os = "linux", feature = "drm"))]
    crate::platform::linux::dispatch_wayland_display_probe();
    #[cfg(target_os = "linux")]
    {
        if !crate::platform::linux::is_x11() {
            crate::server::wayland::init();
        }
    }
    true
}

pub fn global_clean() {}
