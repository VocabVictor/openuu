use super::*;

#[derive(Clone, Default)]
pub struct ConnInner {
    pub(super) id: i32,
    pub(super) tx: Option<Sender>,
    pub(super) tx_video: Option<Sender>,
}

pub(super) struct InputMouse {
    pub(super) msg: MouseEvent,
    pub(super) conn_id: i32,
    pub(super) username: String,
    pub(super) argb: u32,
    pub(super) simulate: bool,
    pub(super) show_cursor: bool,
}

pub(super) enum MessageInput {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Mouse(InputMouse),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Key((KeyEvent, bool)),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Pointer((PointerDeviceEvent, i32)),
    BlockOn,
    BlockOff,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub struct SessionKey {
    pub(super) peer_id: String,
    pub(super) name: String,
    pub(super) session_id: u64,
}

#[derive(Clone, Debug)]
pub(super) struct Session {
    pub(super) last_recv_time: Arc<Mutex<Instant>>,
    pub(super) random_password: String,
    pub(super) tfa: bool,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) struct StartCmIpcPara {
    pub(super) rx_to_cm: mpsc::UnboundedReceiver<ipc::Data>,
    pub(super) tx_from_cm: mpsc::UnboundedSender<ipc::Data>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum AuthConnType {
    Remote,
    FileTransfer,
    PortForward,
    ViewCamera,
    Terminal,
}

impl AuthConnType {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            AuthConnType::Remote => "remote",
            AuthConnType::FileTransfer => "file_transfer",
            AuthConnType::PortForward => "port_forward",
            AuthConnType::ViewCamera => "view_camera",
            AuthConnType::Terminal => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub(super) enum ConnAuditPrimaryAuth {
    None = 0,
    Click = 1,
    TemporaryPassword = 2,
    PermanentPassword = 3,
    SwitchSides = 4,
}

impl ConnAuditPrimaryAuth {
    pub(super) fn as_i64(self) -> i64 {
        self as i64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i64)]
pub(super) enum ConnAuditTwoFactor {
    None = 0,
    Totp = 1,
    TrustedDevice = 2,
}

impl ConnAuditTwoFactor {
    pub(super) fn as_i64(self) -> i64 {
        self as i64
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[derive(Clone, Debug)]
pub(super) enum TerminalUserToken {
    SelfUser,
    #[cfg(target_os = "windows")]
    CurrentLogonUser(crate::terminal_service::UserToken),
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl TerminalUserToken {
    pub(super) fn to_terminal_service_token(&self) -> Option<crate::terminal_service::UserToken> {
        match self {
            TerminalUserToken::SelfUser => None,
            #[cfg(target_os = "windows")]
            TerminalUserToken::CurrentLogonUser(token) => Some(*token),
        }
    }
}
