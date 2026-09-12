use super::*;

/// Interface for client to send data and commands.
#[async_trait]
pub trait Interface: Send + Clone + 'static + Sized {
    /// Send message data to remote peer.
    fn send(&self, data: Data);
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str);
    fn handle_login_error(&self, err: &str) -> bool;
    fn handle_peer_info(&self, pi: PeerInfo);
    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>);
    fn on_error(&self, err: &str) {
        self.msgbox("error", "Error", err, "");
    }
    async fn handle_hash(&self, pass: &str, hash: Hash, peer: &mut Stream) -> bool;
    async fn handle_login_from_ui(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
        peer: &mut Stream,
    );
    async fn handle_test_delay(&self, t: TestDelay, peer: &mut Stream);

    fn get_lch(&self) -> Arc<RwLock<LoginConfigHandler>>;

    fn get_id(&self) -> String {
        self.get_lch().read().unwrap().id.clone()
    }

    fn is_force_relay(&self) -> bool {
        self.get_lch().read().unwrap().force_relay
    }

    fn is_policy_relay(&self) -> bool {
        self.get_lch().read().unwrap().policy_relay
    }

    fn get_switch_code(&self) -> String {
        match self.get_lch().read().unwrap().switch_uuid.clone() {
            Some(u) if !u.is_empty() => {
                use hbb_common::sodiumoxide::crypto::hash::sha256;
                crate::encode64(sha256::hash(u.as_bytes()).0)
            }
            _ => String::new(),
        }
    }

    fn swap_modifier_mouse(&self, _msg: &mut base::protos::message::MouseEvent) {}

    fn update_direct(&self, direct: Option<bool>) {
        self.get_lch().write().unwrap().direct = direct;
    }

    fn update_received(&self, received: bool) {
        self.get_lch().write().unwrap().received = received;
    }

    fn on_establish_connection_error(&self, err: String) {
        let title = "Connection Error";
        let text = err.to_string();
        let lch = self.get_lch();
        let (is_restarting, direct, received) = {
            let lc = lch.read().unwrap();
            (lc.is_restarting_remote_device(), lc.direct, lc.received)
        };
        if is_restarting {
            log::info!("Restart remote device, suppress connection error: {err}");
            // Flutter treats this as a reconnect control event. The text is kept
            // for legacy UI and existing translation reuse.
            self.msgbox("restarting", "Restarting remote device", "Connection in progress. Please wait.", "");
            return;
        }

        let mut relay_hint = false;
        let mut relay_hint_type = "relay-hint";
        // force relay
        let errno = errno::errno().0;
        log::error!("Connection closed: {err}({errno})");
        if direct == Some(true)
            && ((cfg!(windows) && (errno == 10054 || err.contains("10054")))
                || (!cfg!(windows) && (errno == 104 || err.contains("104")))
                || (!err.contains("Failed") && err.contains("deadline")))
        // deadline: https://github.com/rustdesk/rustdesk-server-pro/discussions/325, most likely comes from secure tcp timeout
        {
            relay_hint = true;
            if !received {
                relay_hint_type = "relay-hint2"
            }
        }

        // relay-hint
        if cfg!(feature = "flutter") && relay_hint {
            self.msgbox(relay_hint_type, title, &text, "");
        } else {
            self.msgbox("error", title, &text, "");
        }
    }
}

/// Data used by the client interface.
#[derive(Clone)]
pub enum Data {
    Close,
    RejectInsecureConnection,
    Login((String, String, String, bool)),
    Message(Message),
    SendFiles((i32, JobType, String, String, i32, bool, bool)),
    RemoveDirAll((i32, String, bool, bool)),
    ConfirmDeleteFiles((i32, i32)),
    SetNoConfirm(i32),
    RemoveDir((i32, String)),
    RemoveFile((i32, String, i32, bool)),
    CreateDir((i32, String, bool)),
    CancelJob(i32),
    PauseJob((i32, bool)),
    RemovePortForward(i32),
    AddPortForward((i32, String, i32)),
    #[cfg(all(target_os = "windows", not(feature = "flutter")))]
    ToggleClipboardFile,
    NewRDP,
    SetConfirmOverrideFile((i32, i32, bool, bool, bool)),
    AddJob((i32, JobType, String, String, i32, bool, bool)),
    ResumeJob((i32, bool)),
    RecordScreen(bool),
    ElevateDirect,
    ElevateWithLogon(String, String),
    NewVoiceCall,
    CloseVoiceCall,
    ContinueInsecureConnection,
    ResetDecoder(Option<usize>),
    RenameFile((i32, String, String, bool)),
    TakeScreenshot((i32, String)),
}

pub async fn confirm_insecure_connection(
    interface: &impl Interface,
    receiver: &mut UnboundedReceiver<Data>,
) -> bool {
    interface.msgbox(
        "insecure-connection-nocancel-hasclose",
        "Insecure Connection",
        "conn-e2ee-unavailable-tip",
        "",
    );
    while let Some(data) = receiver.recv().await {
        match data {
            Data::ContinueInsecureConnection => return true,
            Data::RejectInsecureConnection => return false,
            Data::Close => return false,
            _ => {}
        }
    }
    false
}
