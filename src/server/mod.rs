use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock, Weak},
    time::Duration,
};

use bytes::Bytes;

pub use connection::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::config::Config2;
use hbb_common::tcp::{self, new_listener};
use hbb_common::{
    allow_err,
    anyhow::Context,
    bail,
    config::{Config, CONNECT_TIMEOUT, RELAY_PORT},
    log,
    protobuf::{Enum, Message as _},
    rendezvous_proto::*,
    socket_client,
    sodiumoxide::crypto::{box_, sign},
    timeout, tokio, ResultType, Stream,
};
use base::message_proto::*;
use scrap::camera;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use service::ServiceTmpl;
use service::{EmptyExtraFieldService, GenericService, Service, Subscriber};
use video_service::VideoSource;

use crate::ipc::Data;

mod connect;
pub use connect::*;
mod server_impl;
mod start;
pub use start::*;
mod config_sync;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use config_sync::wait_initial_config_sync;

pub mod audio_service;
#[cfg(target_os = "windows")]
pub mod terminal_helper;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod terminal_service;
cfg_if::cfg_if! {
if #[cfg(not(target_os = "ios"))] {
mod clipboard_service;
#[cfg(target_os = "android")]
pub use clipboard_service::is_clipboard_service_ok;
#[cfg(target_os = "linux")]
pub(crate) mod wayland;
#[cfg(all(target_os = "linux", feature = "drm"))]
pub(crate) mod drm_capturer;
#[cfg(target_os = "linux")]
pub mod uinput;
#[cfg(target_os = "linux")]
pub mod rdp_input;
#[cfg(target_os = "linux")]
pub mod dbus;
#[cfg(not(target_os = "android"))]
pub mod input_service;
} else {
mod clipboard_service {
pub const NAME: &'static str = "";
}
}
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub mod input_service {
    pub const NAME_CURSOR: &'static str = "";
    pub const NAME_POS: &'static str = "";
    pub const NAME_WINDOW_FOCUS: &'static str = "";
}

mod connection;
mod login_failure_check;
pub(crate) mod port_forward_mux;
pub mod display_service;
#[cfg(windows)]
pub mod portable_service;
mod service;
mod video_qos;
pub mod video_service;

pub type Childs = Arc<Mutex<Vec<std::process::Child>>>;
type ConnMap = HashMap<i32, ConnInner>;

#[derive(Clone, Default)]
pub struct ConnectionMeta {
    pub control_permissions: Option<ControlPermissions>,
    pub controlled_context: Option<ControlledContext>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
const CONFIG_SYNC_INTERVAL_SECS: f32 = 0.3;
#[cfg(any(target_os = "macos", target_os = "linux"))]
// 3s is enough for at least one initial sync attempt:
// 0.3s backoff + up to 1s connect timeout + up to 1s response timeout.
const CONFIG_SYNC_INITIAL_WAIT_SECS: u64 = 3;

lazy_static::lazy_static! {
    pub static ref CHILD_PROCESS: Childs = Default::default();
    // A client server used to provide local services(audio, video, clipboard, etc.)
    // for all initiative connections.
    //
    // [Note]
    // ugly
    // Now we use this [`CLIENT_SERVER`] to do following operations:
    // - record local audio, and send to remote
    pub static ref CLIENT_SERVER: ServerPtr = new();
}

pub struct Server {
    connections: ConnMap,
    services: HashMap<String, Box<dyn Service>>,
    id_count: i32,
}

pub type ServerPtr = Arc<RwLock<Server>>;
pub type ServerPtrWeak = Weak<RwLock<Server>>;

pub fn new() -> ServerPtr {
    let mut server = Server {
        connections: HashMap::new(),
        services: HashMap::new(),
        id_count: hbb_common::rand::random::<i32>() % 1000 + 1000, // ensure positive
    };
    server.add_service(Box::new(audio_service::new()));
    #[cfg(not(target_os = "ios"))]
    {
        server.add_service(Box::new(display_service::new()));
        server.add_service(Box::new(clipboard_service::new(
            clipboard_service::NAME.to_owned(),
        )));
        #[cfg(feature = "unix-file-copy-paste")]
        server.add_service(Box::new(clipboard_service::new(
            clipboard_service::FILE_NAME.to_owned(),
        )));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if !display_service::capture_cursor_embedded() {
            server.add_service(Box::new(input_service::new_cursor()));
            server.add_service(Box::new(input_service::new_pos()));
            #[cfg(target_os = "linux")]
            if scrap::is_x11() {
                // wayland does not support multiple displays currently
                server.add_service(Box::new(input_service::new_window_focus()));
            }
            #[cfg(not(target_os = "linux"))]
            server.add_service(Box::new(input_service::new_window_focus()));
        }
    }
    // Terminal service is created per connection, not globally
    Arc::new(RwLock::new(server))
}


