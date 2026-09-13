use crate::ipc::{self, new_listener, Connection, Data, DataKeyboard, DataMouse};
use enigo::{Key, KeyboardControllable, MouseButton, MouseControllable};
use evdev::{
    uinput::{VirtualDevice, VirtualDeviceBuilder},
    AttributeSet, EventType, InputEvent,
};
use hbb_common::{
    allow_err, bail, log,
    tokio::{self, runtime::Runtime},
    ResultType,
};

static IPC_CONN_TIMEOUT: u64 = 1000;
static IPC_REQUEST_TIMEOUT: u64 = 1000;
static IPC_POSTFIX_KEYBOARD: &str = "_uinput_keyboard";
static IPC_POSTFIX_MOUSE: &str = "_uinput_mouse";
static IPC_POSTFIX_CONTROL: &str = "_uinput_control";

pub mod client;

pub mod service {
    use super::*;
    use hbb_common::lazy_static;
    #[cfg(target_os = "linux")]
    use parity_tokio_ipc::Connection as RawIpcConnection;
    use scrap::wayland::{
        pipewire::RDP_SESSION_INFO, remote_desktop_portal::OrgFreedesktopPortalRemoteDesktop,
    };
    #[cfg(target_os = "linux")]
    use std::os::unix::io::AsRawFd;
    use std::{collections::HashMap, sync::Mutex};

    mod key_map;
    use key_map::*;
    mod text_input;
    pub(crate) use text_input::*;
    mod keyboard;
    pub use keyboard::*;
    mod input_handlers;
    use input_handlers::*;
    mod start;
    pub use start::*;

}

// https://github.com/emrebicer/mouce
mod mouce {
    use std::{
        fs::File,
        io::{Error, ErrorKind, Result},
        mem::size_of,
        os::{
            raw::{c_char, c_int, c_long, c_uint, c_ulong, c_ushort},
            unix::{fs::OpenOptionsExt, io::AsRawFd},
        },
        thread,
        time::Duration,
    };

    mod ffi_types;
    pub use ffi_types::*;
    mod manager;
    pub use manager::*;

}
