use super::*;

impl<T: InvokeUiSession> Session<T> {
    pub fn restart_remote_device(&self) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        let mut lc = self.lc.write().unwrap();
        lc.mark_restarting_remote_device();
        let msg = lc.restart_remote_device();
        self.send(Data::Message(msg));
    }

    pub fn get_audit_server(&self, typ: String) -> String {
        if LocalConfig::get_option("access_token").is_empty() {
            return "".to_owned();
        }
        crate::get_audit_server(
            Config::get_option("api-server"),
            Config::get_option("custom-rendezvous-server"),
            typ,
        )
    }

    pub fn send_note(&self, note: String) {
        let url = self.get_audit_server("conn".to_string());
        let id = self.get_id();
        let session_id = self.lc.read().unwrap().session_id;
        *self.last_audit_note.lock().unwrap() = note.clone();
        std::thread::spawn(move || {
            send_note(url, id, session_id, note);
        });
    }

    pub fn remove_port_forward(&self, port: i32) {
        let mut config = self.load_config();
        config.port_forwards = config
            .port_forwards
            .drain(..)
            .filter(|x| x.0 != port)
            .collect();
        self.save_config(config);
        self.send(Data::RemovePortForward(port));
    }

    pub fn add_port_forward(&self, port: i32, remote_host: String, remote_port: i32) {
        let mut config = self.load_config();
        if config
            .port_forwards
            .iter()
            .filter(|x| x.0 == port)
            .next()
            .is_some()
        {
            return;
        }
        let pf = (port, remote_host, remote_port);
        config.port_forwards.push(pf.clone());
        self.save_config(config);
        self.send(Data::AddPortForward(pf));
    }

    pub fn get_option(&self, k: String) -> String {
        if k.eq("remote_dir") {
            return self.lc.read().unwrap().get_remote_dir();
        }
        self.lc.read().unwrap().get_option(&k)
    }

    pub fn set_option(&self, k: String, mut v: String) {
        let mut lc = self.lc.write().unwrap();
        if k.eq("remote_dir") {
            v = lc.get_all_remote_dir(v);
        }
        lc.set_option(k, v);
    }

    #[inline]
    pub fn load_config(&self) -> PeerConfig {
        self.lc.read().unwrap().load_config()
    }

    #[inline]
    pub(crate) fn save_config(&self, config: PeerConfig) {
        self.lc.write().unwrap().save_config(config);
    }

    pub fn is_restarting_remote_device(&self) -> bool {
        self.lc.read().unwrap().is_restarting_remote_device()
    }

    #[inline]
    pub fn peer_platform(&self) -> String {
        self.lc.read().unwrap().info.platform.clone()
    }

    pub fn get_platform(&self, is_remote: bool) -> String {
        if is_remote {
            self.peer_platform()
        } else {
            whoami::platform().to_string()
        }
    }

    pub fn get_path_sep(&self, is_remote: bool) -> &'static str {
        let p = self.get_platform(is_remote);
        if &p == crate::PLATFORM_WINDOWS {
            return "\\";
        } else {
            return "/";
        }
    }

    pub fn input_os_password(&self, pass: String, activate: bool) {
        if self.lc.read().map(|lc| lc.view_only_session).unwrap_or(true) {
            return;
        }
        input_os_password(pass, activate, self.clone());
    }

}
