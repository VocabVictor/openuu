use super::*;

pub(super) fn start_wakelock_thread() -> std::sync::mpsc::Sender<(usize, usize)> {
    // Check if we should keep awake during incoming sessions
    use crate::platform::{get_wakelock, WakeLock};
    let (tx, rx) = std::sync::mpsc::channel::<(usize, usize)>();
    std::thread::spawn(move || {
        let mut wakelock: Option<WakeLock> = None;
        let mut last_display = false;
        loop {
            match rx.recv() {
                Ok((conn_count, remote_count)) => {
                    let keep_awake = config::Config::get_bool_option(
                        keys::OPTION_KEEP_AWAKE_DURING_INCOMING_SESSIONS,
                    );
                    *WAKELOCK_KEEP_AWAKE_OPTION.lock().unwrap() = Some(keep_awake);
                    if conn_count == 0 || !keep_awake {
                        if wakelock.is_some() {
                            wakelock = None;
                            log::info!("drop wakelock");
                        }
                    } else {
                        let mut display = remote_count > 0;
                        if let Some(_w) = wakelock.as_mut() {
                            if display != last_display {
                                #[cfg(any(target_os = "windows", target_os = "macos"))]
                                {
                                    log::info!("set wakelock display to {display}");
                                    if let Err(e) = _w.set_display(display) {
                                        log::error!(
                                            "failed to set wakelock display to {display}: {e:?}"
                                        );
                                    }
                                }
                            }
                        } else {
                            if cfg!(target_os = "linux") {
                                display = true;
                            }
                            wakelock = Some(get_wakelock(display));
                        }
                        last_display = display;
                    }
                }
                Err(e) => {
                    log::error!("wakelock receive error: {e:?}");
                    break;
                }
            }
        }
    });
    tx
}

#[cfg(windows)]
pub struct PortableState {
    pub last_uac: bool,
    pub last_foreground_window_elevated: bool,
    pub last_running: Option<bool>,
    pub is_installed: bool,
}

#[cfg(windows)]
impl Default for PortableState {
    fn default() -> Self {
        Self {
            is_installed: crate::platform::is_installed(),
            last_uac: Default::default(),
            last_foreground_window_elevated: Default::default(),
            last_running: Default::default(),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        self.release_pressed_modifiers();

        if let Some(s) = self.terminal_generic_service.as_ref() {
            s.join();
        }

        #[cfg(target_os = "windows")]
        if let Some(TerminalUserToken::CurrentLogonUser(token)) = self.terminal_user_token.take() {
            if token.as_raw() != 0 {
                unsafe {
                    hbb_common::allow_err!(CloseHandle(HANDLE(token.as_raw() as _)));
                };
            }
        }
    }
}

pub(super) extern "C" fn connection_shutdown_hook() {
    // https://stackoverflow.com/questions/35980148/why-does-an-atexit-handler-panic-when-it-accesses-stdout
    // Please make sure there is no print in the call stack
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        *WALLPAPER_REMOVER.lock().unwrap() = None;
    }
}
