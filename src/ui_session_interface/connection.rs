use super::*;

impl<T: InvokeUiSession> Session<T> {
    pub fn reconnect(&self, force_relay: bool) {
        // 1. If current session is connecting, do not reconnect.
        // 2. If the connection is established, send `Data::Close`.
        // 3. If the connection is disconnected, do nothing.
        let mut connection_round_state_lock = self.connection_round_state.lock().unwrap();
        if self.thread.lock().unwrap().is_some() {
            match connection_round_state_lock.state {
                ConnectionState::Connecting => return,
                ConnectionState::Connected => self.send(Data::Close),
                ConnectionState::Disconnected => {}
            }
        }
        let round = connection_round_state_lock.new_round();
        drop(connection_round_state_lock);

        let cloned = self.clone();

        // override only if true
        if true == force_relay {
            let mut lc = self.lc.write().unwrap();
            lc.force_relay = true;
            // An explicit retry-via-relay is a decision about this peer, not transport
            // necessity: Relay-only ICE for this round like any force-always-relay session,
            // and it is the one kind of relay that belongs in the peer's saved config.
            lc.policy_relay = true;
            lc.peer_relay = true;
        }
        self.lc.write().unwrap().peer_info = None;
        self.reconnect_count.fetch_add(1, Ordering::SeqCst);
        let mut lock = self.thread.lock().unwrap();
        // No need to join the previous thread, because it will exit automatically.
        // And the previous thread will not change important states.
        *lock = Some(std::thread::spawn(move || {
            io_loop(cloned, round);
        }));
    }

    #[cfg(not(feature = "flutter"))]
    pub fn get_icon_path(&self, file_type: i32, ext: String) -> String {
        let mut path = Config::icon_path();
        if file_type == FileType::DirLink as i32 {
            let new_path = path.join("dir_link");
            if !std::fs::metadata(&new_path).is_ok() {
                #[cfg(windows)]
                allow_err!(std::os::windows::fs::symlink_file(&path, &new_path));
                #[cfg(not(windows))]
                allow_err!(std::os::unix::fs::symlink(&path, &new_path));
            }
            path = new_path;
        } else if file_type == FileType::File as i32 {
            if !ext.is_empty() {
                path = path.join(format!("file.{}", ext));
            } else {
                path = path.join("file");
            }
            if !std::fs::metadata(&path).is_ok() {
                allow_err!(std::fs::File::create(&path));
            }
        } else if file_type == FileType::FileLink as i32 {
            let new_path = path.join("file_link");
            if !std::fs::metadata(&new_path).is_ok() {
                path = path.join("file");
                if !std::fs::metadata(&path).is_ok() {
                    allow_err!(std::fs::File::create(&path));
                }
                #[cfg(windows)]
                allow_err!(std::os::windows::fs::symlink_file(&path, &new_path));
                #[cfg(not(windows))]
                allow_err!(std::os::unix::fs::symlink(&path, &new_path));
            }
            path = new_path;
        } else if file_type == FileType::DirDrive as i32 {
            if cfg!(windows) {
                path = fs::get_path("C:");
            } else if cfg!(target_os = "macos") {
                if let Ok(entries) = fs::get_path("/Volumes/").read_dir() {
                    for entry in entries {
                        if let Ok(entry) = entry {
                            path = entry.path();
                            break;
                        }
                    }
                }
            }
        }
        fs::get_string(&path)
    }

    pub fn login(
        &self,
        os_username: String,
        os_password: String,
        password: String,
        remember: bool,
    ) {
        self.send(Data::Login((os_username, os_password, password, remember)));
    }

    pub fn send2fa(&self, code: String, trust_this_device: bool) {
        let mut msg_out = Message::new();
        let hwid = if trust_this_device {
            crate::get_hwid()
        } else {
            Bytes::new()
        };
        self.lc.write().unwrap().set_option(
            "trust-this-device".to_string(),
            if trust_this_device { "Y" } else { "" }.to_string(),
        );
        msg_out.set_auth_2fa(Auth2FA {
            code,
            hwid,
            ..Default::default()
        });
        self.send(Data::Message(msg_out));
    }

    pub fn get_enable_trusted_devices(&self) -> bool {
        self.lc.read().unwrap().enable_trusted_devices
    }

    pub fn new_rdp(&self) {
        self.send(Data::NewRDP);
    }

    pub fn close(&self) {
        self.send(Data::Close);
    }

    pub fn continue_insecure_connection(&self, continue_insecure: bool) {
        let data = if continue_insecure {
            Data::ContinueInsecureConnection
        } else {
            Data::RejectInsecureConnection
        };
        self.send(data);
    }

    fn try_auto_start_job_str(is_reconnected: bool, job_str: &str) -> Option<String> {
        if is_reconnected {
            let job_str = job_str.trim();
            if let Some(stripped) = job_str.strip_suffix('}') {
                format!(r#"{},"auto_start": true}}"#, stripped).into()
            } else {
                // unreachable in normal cases
                log::warn!(
                    "The last character is not '}}': {}, auto start is ignored on flutter",
                    job_str
                );
                Some(job_str.to_owned())
            }
        } else {
            None
        }
    }

    pub fn load_last_jobs(&self) {
        self.clear_all_jobs();
        let pc = self.load_config();
        if pc.transfer.write_jobs.is_empty() && pc.transfer.read_jobs.is_empty() {
            // no last jobs
            return;
        }
        let reconnect_count_thr = if cfg!(feature = "flutter") { 0 } else { 1 };
        let is_reconnected = self.reconnect_count.load(Ordering::SeqCst) > reconnect_count_thr;
        // TODO: can add a confirm dialog
        let mut cnt = 1;
        for job_str in pc.transfer.read_jobs.iter() {
            if !job_str.is_empty() {
                self.load_last_job(
                    cnt,
                    Self::try_auto_start_job_str(is_reconnected, job_str)
                        .as_deref()
                        .unwrap_or(job_str),
                    is_reconnected,
                );
                cnt += 1;
                log::info!("restore read_job: {:?}", job_str);
            }
        }
        for job_str in pc.transfer.write_jobs.iter() {
            if !job_str.is_empty() {
                self.load_last_job(
                    cnt,
                    Self::try_auto_start_job_str(is_reconnected, job_str)
                        .as_deref()
                        .unwrap_or(job_str),
                    is_reconnected,
                );
                cnt += 1;
                log::info!("restore write_job: {:?}", job_str);
            }
        }
        self.update_transfer_list();
    }

    pub fn elevate_direct(&self) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        self.send(Data::ElevateDirect);
    }

    pub fn elevate_with_logon(&self, username: String, password: String) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        self.send(Data::ElevateWithLogon(username, password));
    }

    #[cfg(any(target_os = "android", target_os = "ios", not(feature = "flutter")))]
    pub fn switch_sides(&self) {}

    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    #[tokio::main(flavor = "current_thread")]
    pub async fn switch_sides(&self) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                if conn
                    .send(&crate::ipc::Data::SwitchSidesRequest(self.get_id()))
                    .await
                    .is_ok()
                {
                    if let Ok(Some(data)) = conn.next_timeout(1000).await {
                        match data {
                            crate::ipc::Data::SwitchSidesRequest(str_uuid) => {
                                if let Ok(uuid) = Uuid::from_str(&str_uuid) {
                                    let mut misc = Misc::new();
                                    misc.set_switch_sides_request(SwitchSidesRequest {
                                        uuid: Bytes::from(uuid.as_bytes().to_vec()),
                                        ..Default::default()
                                    });
                                    let mut msg_out = Message::new();
                                    msg_out.set_misc(misc);
                                    self.send(Data::Message(msg_out));
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Err(err) => {
                log::info!("server not started (will try to start): {}", err);
            }
        }
    }
}
