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
mod login_scope;
mod on_message;
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

    #[test]
    fn session_scope_allows_only_messages_for_authenticated_session_type() {
        let cases = [
            (
                AuthConnType::FileTransfer,
                vec![
                    (msg(|m| m.set_file_action(FileAction::new())), None),
                    (msg(|m| m.set_file_response(FileResponse::new())), None),
                    (msg(|m| m.set_login_request(LoginRequest::new())), None),
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        Some("screenshot_request"),
                    ),
                    (
                        misc_msg(|m| m.set_capture_displays(CaptureDisplays::new())),
                        Some("misc.capture_displays"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                    (msg(|m| m.set_clipboard(Clipboard::new())), None),
                    (
                        msg(|m| m.set_multi_clipboards(MultiClipboards::new())),
                        None,
                    ),
                    (misc_msg(|m| m.set_refresh_video(true)), None),
                    (misc_msg(|m| m.set_refresh_video_display(0)), None),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default())
                        }),
                        None,
                    ),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default());
                            o.disable_audio = BoolOption::Yes.into();
                        }),
                        Some("misc.option"),
                    ),
                    (
                        msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                        Some("port_forward_channel"),
                    ),
                ],
            ),
            (
                AuthConnType::Terminal,
                vec![
                    (msg(|m| m.set_terminal_action(TerminalAction::new())), None),
                    (
                        option_msg(|o| o.terminal_persistent = BoolOption::Yes.into()),
                        None,
                    ),
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        Some("screenshot_request"),
                    ),
                    (
                        msg(|m| m.set_file_action(FileAction::new())),
                        Some("file_action"),
                    ),
                    (
                        misc_msg(|m| m.set_toggle_privacy_mode(TogglePrivacyMode::new())),
                        Some("misc.toggle_privacy_mode"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                    (misc_msg(|m| m.set_chat_message(ChatMessage::new())), None),
                    (msg(|m| m.set_clipboard(Clipboard::new())), None),
                    (
                        msg(|m| m.set_multi_clipboards(MultiClipboards::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_toggle_virtual_display(ToggleVirtualDisplay::new())),
                        Some("misc.toggle_virtual_display"),
                    ),
                    (
                        misc_msg(|m| m.set_change_resolution(Resolution::new())),
                        Some("misc.change_resolution"),
                    ),
                    (
                        misc_msg(|m| m.set_change_display_resolution(DisplayResolution::new())),
                        Some("misc.change_display_resolution"),
                    ),
                    (misc_msg(|m| m.set_refresh_video(true)), None),
                    (misc_msg(|m| m.set_refresh_video_display(0)), None),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default())
                        }),
                        None,
                    ),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default());
                            o.disable_audio = BoolOption::Yes.into();
                        }),
                        Some("misc.option"),
                    ),
                    (
                        msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                        Some("port_forward_channel"),
                    ),
                ],
            ),
            (
                AuthConnType::ViewCamera,
                vec![
                    (
                        misc_msg(|m| m.set_switch_display(SwitchDisplay::new())),
                        None,
                    ),
                    (misc_msg(|m| m.set_chat_message(ChatMessage::new())), None),
                    (
                        msg(|m| m.set_voice_call_request(VoiceCallRequest::new())),
                        None,
                    ),
                    (msg(|m| m.set_audio_frame(AudioFrame::new())), None),
                    (
                        option_msg(|o| o.image_quality = ImageQuality::Balanced.into()),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_toggle_privacy_mode(TogglePrivacyMode::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_toggle_virtual_display(ToggleVirtualDisplay::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_change_resolution(Resolution::new())),
                        None,
                    ),
                    (
                        misc_msg(|m| m.set_change_display_resolution(DisplayResolution::new())),
                        None,
                    ),
                    (msg(|m| m.set_mouse_event(MouseEvent::new())), None),
                    (
                        msg(|m| m.set_pointer_device_event(PointerDeviceEvent::new())),
                        None,
                    ),
                    (msg(|m| m.set_key_event(KeyEvent::new())), None),
                    (misc_msg(|m| m.set_client_record_status(true)), None),
                    (
                        msg(|m| m.set_file_response(FileResponse::new())),
                        Some("file_response"),
                    ),
                    (
                        msg(|m| m.set_terminal_action(TerminalAction::new())),
                        Some("terminal_action"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                ],
            ),
            (
                AuthConnType::Remote,
                vec![
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        None,
                    ),
                    (msg(|m| m.set_terminal_action(TerminalAction::new())), None),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        None,
                    ),
                ],
            ),
            (
                AuthConnType::PortForward,
                vec![
                    (msg(|m| m.set_test_delay(TestDelay::new())), None),
                    (misc_msg(|m| m.set_close_reason("closed".to_owned())), None),
                    (
                        msg(|m| m.set_file_action(FileAction::new())),
                        Some("file_action"),
                    ),
                    (
                        msg(|m| m.set_terminal_action(TerminalAction::new())),
                        Some("terminal_action"),
                    ),
                    (
                        msg(|m| m.set_screenshot_request(ScreenshotRequest::new())),
                        Some("screenshot_request"),
                    ),
                    (
                        misc_msg(|m| m.set_switch_sides_request(SwitchSidesRequest::new())),
                        Some("misc.switch_sides_request"),
                    ),
                    (misc_msg(|m| m.set_refresh_video(true)), None),
                    (misc_msg(|m| m.set_refresh_video_display(0)), None),
                    (
                        option_msg(|o| {
                            o.supported_decoding =
                                hbb_common::protobuf::MessageField::some(Default::default())
                        }),
                        None,
                    ),
                    (
                        msg(|m| m.set_port_forward_channel(PortForwardChannel::new())),
                        None,
                    ),
                ],
            ),
        ];

        for (conn_type, messages) in cases {
            assert_scopes(conn_type, messages);
        }
    }

    #[test]
    fn session_scope_login_options_are_limited_to_authenticated_session_type() {
        let mut option = OptionMessage::new();
        option.image_quality = ImageQuality::Balanced.into();
        option.disable_audio = BoolOption::Yes.into();
        option.block_input = BoolOption::Yes.into();
        option.privacy_mode = BoolOption::Yes.into();

        let (scoped, violation) =
            Connection::scoped_login_option(AuthConnType::ViewCamera, &option);
        let scoped = scoped.unwrap();
        assert_eq!(violation, Some("login.option"));
        assert_eq!(
            scoped.image_quality.enum_value(),
            Ok(ImageQuality::Balanced)
        );
        assert_eq!(scoped.disable_audio.enum_value(), Ok(BoolOption::Yes));
        assert_eq!(scoped.block_input.enum_value(), Ok(BoolOption::NotSet));
        assert_eq!(scoped.privacy_mode.enum_value(), Ok(BoolOption::NotSet));

        let (scoped, violation) =
            Connection::scoped_login_option(AuthConnType::FileTransfer, &option);
        assert!(scoped.is_none());
        assert_eq!(violation, Some("login.option"));
    }

    #[test]
    fn session_scope_limited_render_noop_options_reject_mixed_fields() {
        for conn_type in [
            AuthConnType::FileTransfer,
            AuthConnType::Terminal,
            AuthConnType::PortForward,
        ] {
            let supported_decoding_only = option_msg(set_supported_decoding);
            assert_eq!(
                Connection::authorized_message_scope_violation(conn_type, &supported_decoding_only),
                None
            );

            let mixed_option = option_msg(|o| {
                set_supported_decoding(o);
                o.disable_audio = BoolOption::Yes.into();
            });
            assert_eq!(
                Connection::authorized_message_scope_violation(conn_type, &mixed_option),
                Some("misc.option")
            );
        }
    }

    #[test]
    fn session_scope_view_camera_options_keep_only_camera_fields() {
        let mut option = OptionMessage::new();
        option.image_quality = ImageQuality::Balanced.into();
        option.custom_image_quality = 80;
        option.custom_fps = 24;
        set_supported_decoding(&mut option);
        option.disable_audio = BoolOption::Yes.into();
        option.block_input = BoolOption::Yes.into();
        option.disable_clipboard = BoolOption::Yes.into();
        option.enable_file_transfer = BoolOption::Yes.into();
        option.terminal_persistent = BoolOption::Yes.into();

        let (scoped, violation) =
            Connection::scoped_login_option(AuthConnType::ViewCamera, &option);
        let scoped = scoped.unwrap();
        assert_eq!(violation, Some("login.option"));
        assert_eq!(
            scoped.image_quality.enum_value(),
            Ok(ImageQuality::Balanced)
        );
        assert_eq!(scoped.custom_image_quality, 80);
        assert_eq!(scoped.custom_fps, 24);
        assert!(scoped.supported_decoding.is_some());
        assert_eq!(scoped.disable_audio.enum_value(), Ok(BoolOption::Yes));
        assert_eq!(scoped.block_input.enum_value(), Ok(BoolOption::NotSet));
        assert_eq!(
            scoped.disable_clipboard.enum_value(),
            Ok(BoolOption::NotSet)
        );
        assert_eq!(
            scoped.enable_file_transfer.enum_value(),
            Ok(BoolOption::NotSet)
        );
        assert_eq!(
            scoped.terminal_persistent.enum_value(),
            Ok(BoolOption::NotSet)
        );
    }
    #[test]
    fn only_a_newer_remote_control_of_the_same_session_keeps_the_screen_unlocked() {
        let replaced_by = super::raii::AuthedConnID::is_newer_session_remote;

        let key = |session_id, peer: &str| SessionKey {
            peer_id: peer.to_owned(),
            name: "".to_owned(),
            session_id,
        };
        let conn = |conn_id, conn_type, session_key| AuthedConn {
            conn_id,
            conn_type,
            session_key,
            sender: mpsc::unbounded_channel().0,
        };
        let mine = key(7, "peer");
        let remote = AuthConnType::Remote;

        assert!(replaced_by(&conn(3, remote, mine.clone()), 2, &mine));
        // An older one, and itself: of connections ending at once only the last still locks.
        assert!(!replaced_by(&conn(1, remote, mine.clone()), 2, &mine));
        assert!(!replaced_by(&conn(2, remote, mine.clone()), 2, &mine));
        // A kind that keeps no screen in use.
        assert!(!replaced_by(
            &conn(3, AuthConnType::Terminal, mine.clone()),
            2,
            &mine
        ));
        // Another session of this peer, and another peer on the same session id: `SessionKey`
        // is all three fields, and either of those is someone else's screen to lock.
        assert!(!replaced_by(&conn(3, remote, key(8, "peer")), 2, &mine));
        assert!(!replaced_by(&conn(3, remote, key(7, "other")), 2, &mine));
    }
}
