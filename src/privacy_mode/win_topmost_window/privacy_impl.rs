use super::*;

impl PrivacyModeImpl {
    pub fn new(impl_key: &str) -> Self {
        Self {
            impl_key: impl_key.to_owned(),
            conn_id: INVALID_PRIVACY_MODE_CONN_ID,
            handlers: WindowHandlers {
                hthread: 0,
                hprocess: 0,
            },
            hwnd: 0,
        }
    }

    #[inline]
    pub fn get_hwnd(&self) -> u64 {
        self.hwnd
    }

    pub fn start(&mut self) -> ResultType<()> {
        if self.handlers.hprocess != 0 {
            return Ok(());
        }

        log::info!("Start privacy mode window broker, check_update_broker_process");
        if let Err(e) = crate::platform::windows::check_update_broker_process() {
            log::warn!(
                "Failed to check update broker process. Privacy mode may not work properly. {}",
                e
            );
        }

        let exe_file = std::env::current_exe()?;
        let Some(cur_dir) = exe_file.parent() else {
            bail!("Cannot get parent of current exe file");
        };

        let dll_file = cur_dir.join("WindowInjection.dll");
        if !dll_file.exists() {
            bail!(
                "Failed to find required file {}",
                dll_file.to_string_lossy().as_ref()
            );
        }

        if wait_find_privacy_hwnds(PRIVACY_WINDOW_WAIT_MILLIS).is_ok() {
            log::info!("Privacy window is ready");
            return Ok(());
        }

        // let cmdline = cur_dir.join("MiniBroker.exe").to_string_lossy().to_string();
        let cmdline = cur_dir
            .join(INJECTED_PROCESS_EXE)
            .to_string_lossy()
            .to_string();

        unsafe {
            let cmd_utf16: Vec<u16> = cmdline.encode_utf16().chain(Some(0).into_iter()).collect();

            let mut start_info = STARTUPINFOW {
                cb: 0,
                lpReserved: NULL as _,
                lpDesktop: NULL as _,
                lpTitle: NULL as _,
                dwX: 0,
                dwY: 0,
                dwXSize: 0,
                dwYSize: 0,
                dwXCountChars: 0,
                dwYCountChars: 0,
                dwFillAttribute: 0,
                dwFlags: 0,
                wShowWindow: 0,
                cbReserved2: 0,
                lpReserved2: NULL as _,
                hStdInput: NULL as _,
                hStdOutput: NULL as _,
                hStdError: NULL as _,
            };
            let mut proc_info = PROCESS_INFORMATION {
                hProcess: NULL as _,
                hThread: NULL as _,
                dwProcessId: 0,
                dwThreadId: 0,
            };

            let session_id = WTSGetActiveConsoleSessionId();
            let token = get_user_token(session_id, true);
            if token.is_null() {
                bail!("Failed to get token of current user");
            }

            let create_res = CreateProcessAsUserW(
                token,
                NULL as _,
                cmd_utf16.as_ptr() as _,
                NULL as _,
                NULL as _,
                FALSE,
                CREATE_SUSPENDED | DETACHED_PROCESS,
                NULL,
                NULL as _,
                &mut start_info,
                &mut proc_info,
            );
            CloseHandle(token);
            if 0 == create_res {
                bail!(
                    "Failed to create privacy window process {}, error {}",
                    cmdline,
                    Error::last_os_error()
                );
            };

            if let Err(e) = inject_dll(
                proc_info.hProcess,
                proc_info.hThread,
                dll_file.to_string_lossy().as_ref(),
            ) {
                TerminateProcess(proc_info.hProcess, 0);
                CloseHandle(proc_info.hThread);
                CloseHandle(proc_info.hProcess);
                return Err(e);
            }

            if 0xffffffff == ResumeThread(proc_info.hThread) {
                TerminateProcess(proc_info.hProcess, 0);
                CloseHandle(proc_info.hThread);
                CloseHandle(proc_info.hProcess);

                bail!(
                    "Failed to create privacy window process, error {}",
                    Error::last_os_error()
                );
            }

            self.handlers.hthread = proc_info.hThread as _;
            self.handlers.hprocess = proc_info.hProcess as _;

            if let Err(e) = wait_find_privacy_hwnds(PRIVACY_WINDOW_WAIT_MILLIS) {
                self.handlers.reset();
                return Err(e);
            }
        }

        Ok(())
    }

    #[inline]
    pub fn stop(&mut self) {
        self.handlers.reset();
    }

    pub(super) fn show_privacy_windows(&mut self, conn_id: i32, hook_input: bool) -> ResultType<()> {
        let hwnds = wait_find_privacy_hwnds(PRIVACY_WINDOW_WAIT_MILLIS)?;
        if hwnds.is_empty() {
            bail!("No privacy window created");
        }

        if hook_input {
            crate::privacy_mode::win_input::hook()?;
        }
        match set_privacy_windows_visible(&hwnds, true) {
            Ok(_) => {
                let visible_hwnds =
                    match wait_find_visible_privacy_hwnds(PRIVACY_WINDOW_WAIT_MILLIS) {
                        Ok(hwnds) => hwnds,
                        Err(e) => {
                            allow_err!(set_privacy_windows_visible(&hwnds, false));
                            if hook_input {
                                allow_err!(crate::privacy_mode::win_input::unhook());
                            }
                            return Err(e);
                        }
                    };
                let Some(hwnd) = visible_hwnds.first() else {
                    allow_err!(set_privacy_windows_visible(&hwnds, false));
                    if hook_input {
                        allow_err!(crate::privacy_mode::win_input::unhook());
                    }
                    bail!("No visible privacy window created");
                };
                self.conn_id = conn_id;
                self.hwnd = *hwnd as _;
                Ok(())
            }
            Err(e) => {
                allow_err!(set_privacy_windows_visible(&hwnds, false));
                if hook_input {
                    allow_err!(crate::privacy_mode::win_input::unhook());
                }
                Err(e)
            }
        }
    }
}

impl Drop for PrivacyModeImpl {
    fn drop(&mut self) {
        if self.conn_id != INVALID_PRIVACY_MODE_CONN_ID {
            allow_err!(self.turn_off_privacy(self.conn_id, None));
        }
    }
}
