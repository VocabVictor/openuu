#[cfg(target_os = "windows")]
use super::login_failure_check::try_acquire_os_credential_login_gate;
use super::login_failure_check::{
    evaluate_os_credential_policy, record_os_credential_failure, FailureScope,
};
use super::{input_service::*, *};
#[cfg(feature = "unix-file-copy-paste")]
use crate::clipboard::try_empty_clipboard_files;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::clipboard::{update_clipboard, ClipboardSide};
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use crate::clipboard_file::*;
#[cfg(target_os = "android")]
use crate::keyboard::client::map_key_to_control_key;
#[cfg(any(target_os = "windows", target_os = "linux"))]
use crate::platform::WallPaperRemover;
#[cfg(windows)]
use crate::portable_service::client as portable_client;
use crate::{
    client::{
        new_voice_call_request, new_voice_call_response, start_audio_thread, MediaData, MediaSender,
    },
    display_service, ipc, privacy_mode, video_service, VERSION,
};
#[cfg(any(target_os = "android", target_os = "ios"))]
use crate::{common::DEVICE_NAME, flutter::connection_manager::start_channel};
use cidr_utils::cidr::IpCidr;
#[cfg(target_os = "android")]
use hbb_common::protobuf::EnumOrUnknown;
use hbb_common::{
    config::{
        self, decode_permanent_password_h1_from_storage, decode_preset_password_h1_from_storage,
        local_permanent_password_storage_is_usable_for_auth,
        preset_permanent_password_storage_is_usable_for_auth, Config, TrustedDevice,
    },
    futures::{SinkExt, StreamExt},
    get_time, get_version_number,
    password_security::{self as password, ApproveMode},
    sha2::{Digest, Sha256},
    sleep, timeout,
    tokio::{
        net::TcpStream,
        sync::mpsc,
        time::{self, Duration, Instant},
    },
    tokio_util::codec::{BytesCodec, Framed},
};
use base::{
    config::keys,
    fs::{self, can_enable_overwrite_detection, JobType},
    message_proto::{option_message::BoolOption, permission_info::Permission},
};
#[cfg(any(target_os = "android", target_os = "ios"))]
use scrap::android::{call_main_service_key_event, call_main_service_pointer_input};
use scrap::camera;
use serde_derive::Serialize;
use serde_json::{json, value::Value};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::sync::atomic::Ordering;
use std::{
    collections::HashSet,
    net::Ipv6Addr,
    num::NonZeroI64,
    path::PathBuf,
    str::FromStr,
    sync::{atomic::AtomicI64, mpsc as std_mpsc},
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use system_shutdown;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{CloseHandle, HANDLE};

#[cfg(windows)]
use crate::virtual_display_manager;
pub type Sender = mpsc::UnboundedSender<(Instant, Arc<Message>)>;

const FAILURE_IDX_ID_WHITELIST: usize = 2;
// How long a rejection counts, so also how long a blocked address stays blocked. Longer
// throttles enumeration harder; shorter limits collateral on whitelisted neighbours.
const ID_WHITELIST_FAILURE_DECAY_MINUTES: i32 = 10;

lazy_static::lazy_static! {
    // [0] password, [1] 2FA, [2] ID whitelist.
    // Bucket 2 is separate so its rejections do not touch the password / 2FA budgets. It is
    // decayed in `check_id_whitelist` and cleared on auth, never on a bare id match.
    static ref LOGIN_FAILURES: [Arc::<Mutex<HashMap<String, (i32, i32, i32)>>>; 3] = Default::default();
    static ref SESSIONS: Arc::<Mutex<HashMap<SessionKey, Session>>> = Default::default();
    static ref ALIVE_CONNS: Arc::<Mutex<Vec<i32>>> = Default::default();
    pub static ref AUTHED_CONNS: Arc::<Mutex<Vec<AuthedConn>>> = Default::default();
    pub static ref CONTROL_PERMISSIONS_ARRAY: Arc::<Mutex<Vec<(i32, ControlPermissions)>>> = Default::default();
    static ref WAKELOCK_SENDER: Arc::<Mutex<std::sync::mpsc::Sender<(usize, usize)>>> = Arc::new(Mutex::new(start_wakelock_thread()));
    static ref WAKELOCK_KEEP_AWAKE_OPTION: Arc::<Mutex<Option<bool>>> = Default::default();
}

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const SWITCH_SIDES_UUID_TTL: Duration = Duration::from_secs(10);

#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
lazy_static::lazy_static! {
    static ref SWITCH_SIDES_UUID: Arc::<Mutex<HashMap<String, (Instant, uuid::Uuid)>>> = Default::default();
    static ref PENDING_SWITCH_SIDES_UUID: Arc::<Mutex<HashMap<String, (Instant, uuid::Uuid, bool)>>> = Default::default();
}

#[cfg(target_os = "windows")]
const TERMINAL_OS_LOGIN_FAILED_MSG: &str = "Incorrect username or password.";

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    // Avoid data-dependent early exits.
    let mut x: u8 = 0;
    for i in 0..a.len() {
        x |= a[i] ^ b[i];
    }
    x == 0
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn should_use_terminal_os_login_scope(is_terminal: bool, os_login_username: &str) -> bool {
    cfg!(target_os = "windows") && is_terminal && !os_login_username.trim().is_empty()
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
lazy_static::lazy_static! {
    static ref WALLPAPER_REMOVER: Arc<Mutex<Option<WallPaperRemover>>> = Default::default();
}
pub static CLICK_TIME: AtomicI64 = AtomicI64::new(0);
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub static MOUSE_MOVE_TIME: AtomicI64 = AtomicI64::new(0);

mod types;
pub use types::*;
mod conn_struct;
pub use conn_struct::*;
mod conn_inner;
mod consts;
use consts::*;
mod start;
mod loops;
mod access_checks;
mod audit;
mod port_forward;
mod logon_response;
mod subscriptions;
mod cm_and_input;
mod password_check;
#[cfg(test)]
mod test_support;
mod login_scope;
mod on_message;
mod login_msg;
mod delay_switch_msg;
mod file_msg;
mod file_action_msg;
mod clipboard_msg;
mod media_msg;
mod misc_msg;
mod input_msg;
mod terminal_login;
mod failures;
mod display;
mod voice_options;
mod privacy_close;
mod file_transfer;
mod housekeeping;
mod scope;
mod scope_rules;
mod clipboard_terminal;
mod switch_sides;
pub use switch_sides::*;
mod ipc_start;
use ipc_start::*;
mod audit_types;
pub use audit_types::*;
mod drop_and_state;
#[cfg_attr(not(windows), allow(unused_imports))]
pub use drop_and_state::*;
mod retina_perm;
pub use retina_perm::*;
mod raii;
mod whitelist_match;
use whitelist_match::*;

impl Connection {
}

#[cfg(test)]
mod test {
    #[allow(unused)]
    use super::*;

    mod tests_a;
    mod tests_b;
    use tests_b::*;
    mod scope_tests;
    mod scope_option_tests;
    mod login_tests;
    mod pre_auth_tests;
    mod input_tests;
    mod misc_tests;
    mod media_tests;
    mod clipboard_tests;
    mod file_tests;

}
