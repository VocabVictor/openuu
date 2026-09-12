use super::*;

impl Client {
    #[inline]
    #[cfg(feature = "flutter")]
    #[cfg(not(target_os = "ios"))]
    pub fn set_is_text_clipboard_required(b: bool) {
        CLIPBOARD_STATE.lock().unwrap().is_text_required = b;
    }

    #[inline]
    #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
    pub fn set_is_file_clipboard_required(b: bool) {
        CLIPBOARD_STATE.lock().unwrap().is_file_required = b;
    }

    #[cfg(not(target_os = "ios"))]
    pub(super) fn try_stop_clipboard() {
        // Disconnected Flutter sessions may keep UI handlers alive, so only connected sessions
        // should block clipboard cleanup.
        #[cfg(feature = "flutter")]
        if crate::flutter::sessions::has_connected_sessions_running(ConnType::DEFAULT_CONN) {
            return;
        }
        #[cfg(not(target_os = "android"))]
        clipboard_listener::unsubscribe(Self::CLIENT_CLIPBOARD_NAME);
        CLIPBOARD_STATE.lock().unwrap().running = false;
        #[cfg(all(feature = "unix-file-copy-paste", target_os = "linux"))]
        if let Err(e) = crate::clipboard::try_empty_clipboard_files_sync(
            crate::clipboard::ClipboardSide::Client,
            0,
        ) {
            log::error!("Failed to empty client clipboard files: {}", e);
        }
        #[cfg(all(feature = "unix-file-copy-paste", target_os = "linux"))]
        clipboard::platform::unix::fuse::uninit_fuse_context(true);
    }

    // `try_start_clipboard` is called by all session when connection is established. (When handling peer info).
    // This function only create one thread with a loop, the loop is shared by all sessions.
    // After all sessions are end, the loop exists.
    //
    // If clipboard update is detected, the text will be sent to all sessions by `send_clipboard_msg`.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub(super) fn try_start_clipboard(
        _client_clip_ctx: Option<ClientClipboardContext>,
    ) -> Option<UnboundedReceiver<()>> {
        let mut clipboard_lock = CLIPBOARD_STATE.lock().unwrap();
        if clipboard_lock.running {
            return None;
        }

        let (tx_cb_result, rx_cb_result) = mpsc::channel();
        if let Err(e) =
            clipboard_listener::subscribe(Self::CLIENT_CLIPBOARD_NAME.to_owned(), tx_cb_result)
        {
            log::error!("Failed to subscribe clipboard listener: {}", e);
            return None;
        }

        clipboard_lock.running = true;
        let (tx_started, rx_started) = unbounded_channel();

        log::info!("Start client clipboard loop");
        std::thread::spawn(move || {
            let mut handler = ClientClipboardHandler {
                ctx: None,
                #[cfg(not(feature = "flutter"))]
                client_clip_ctx: _client_clip_ctx,
            };

            tx_started.send(()).ok();
            loop {
                if !CLIPBOARD_STATE.lock().unwrap().running {
                    break;
                }
                match rx_cb_result.recv_timeout(Duration::from_millis(CLIPBOARD_INTERVAL)) {
                    Ok(CallbackResult::Next) => {
                        handler.check_clipboard();
                    }
                    Ok(CallbackResult::Stop) => {
                        log::debug!("Clipboard listener stopped");
                        break;
                    }
                    Ok(CallbackResult::StopWithError(err)) => {
                        log::error!("Clipboard listener stopped with error: {}", err);
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        log::error!("Clipboard listener disconnected");
                        break;
                    }
                }
            }
            log::info!("Stop client clipboard loop");
            CLIPBOARD_STATE.lock().unwrap().running = false;
        });

        Some(rx_started)
    }

    #[cfg(target_os = "android")]
    pub(super) fn try_start_clipboard(_p: Option<()>) -> Option<UnboundedReceiver<()>> {
        let mut clipboard_lock = CLIPBOARD_STATE.lock().unwrap();
        if clipboard_lock.running {
            return None;
        }
        clipboard_lock.running = true;

        log::info!("Start client clipboard loop");
        std::thread::spawn(move || {
            loop {
                if !CLIPBOARD_STATE.lock().unwrap().running {
                    break;
                }
                if !CLIPBOARD_STATE.lock().unwrap().is_text_required {
                    std::thread::sleep(Duration::from_millis(CLIPBOARD_INTERVAL));
                    continue;
                }

                if let Some(msg) = crate::clipboard::get_clipboards_msg(true) {
                    crate::flutter::send_clipboard_msg(msg, false);
                }

                std::thread::sleep(Duration::from_millis(CLIPBOARD_INTERVAL));
            }
            log::info!("Stop client clipboard loop");
            CLIPBOARD_STATE.lock().unwrap().running = false;
        });

        None
    }
}

#[cfg(not(target_os = "ios"))]
impl ClipboardState {
    pub(super) fn new() -> Self {
        Self {
            #[cfg(feature = "flutter")]
            is_text_required: true,
            #[cfg(all(feature = "flutter", feature = "unix-file-copy-paste"))]
            is_file_required: true,
            running: false,
        }
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(super) struct ClientClipboardHandler {
    ctx: Option<crate::clipboard::ClipboardContext>,
    #[cfg(not(feature = "flutter"))]
    client_clip_ctx: Option<ClientClipboardContext>,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl ClientClipboardHandler {
    pub(super) fn is_text_required(&self) -> bool {
        #[cfg(feature = "flutter")]
        {
            CLIPBOARD_STATE.lock().unwrap().is_text_required
        }
        #[cfg(not(feature = "flutter"))]
        {
            self.client_clip_ctx
                .as_ref()
                .map(|ctx| ctx.cfg.is_text_clipboard_required())
                .unwrap_or(false)
        }
    }

    #[cfg(feature = "unix-file-copy-paste")]
    pub(super) fn is_file_required(&self) -> bool {
        #[cfg(feature = "flutter")]
        {
            CLIPBOARD_STATE.lock().unwrap().is_file_required
        }
        #[cfg(not(feature = "flutter"))]
        {
            self.client_clip_ctx
                .as_ref()
                .map(|ctx| ctx.cfg.is_file_clipboard_required())
                .unwrap_or(false)
        }
    }

    pub(super) fn check_clipboard(&mut self) {
        if CLIPBOARD_STATE.lock().unwrap().running {
            #[cfg(feature = "unix-file-copy-paste")]
            if let Some(urls) = check_clipboard_files(&mut self.ctx, ClipboardSide::Client, false) {
                if !urls.is_empty() {
                    #[cfg(target_os = "macos")]
                    if crate::clipboard::is_file_url_set_by_rustdesk(&urls) {
                        return;
                    }
                    if self.is_file_required() {
                        match clipboard::platform::unix::serv_files::sync_files(&urls) {
                            Ok(()) => {
                                let msg = crate::clipboard_file::clip_2_msg(
                                    unix_file_clip::get_format_list(),
                                );
                                self.send_msg(msg, true);
                            }
                            Err(e) => {
                                log::error!("Failed to sync clipboard files: {}", e);
                            }
                        }
                        return;
                    }
                }
            }

            if let Some(msg) = check_clipboard(&mut self.ctx, ClipboardSide::Client, false) {
                if self.is_text_required() {
                    self.send_msg(msg, false);
                }
            }
        }
    }

    #[inline]
    #[cfg(feature = "flutter")]
    pub(super) fn send_msg(&self, msg: Message, _is_file: bool) {
        crate::flutter::send_clipboard_msg(msg, _is_file);
    }

    #[cfg(not(feature = "flutter"))]
    pub(super) fn send_msg(&self, msg: Message, _is_file: bool) {
        if let Some(ctx) = &self.client_clip_ctx {
            #[cfg(feature = "unix-file-copy-paste")]
            if _is_file {
                if ctx.is_file_supported {
                    let _ = ctx.tx.send(Data::Message(msg));
                }
                return;
            }

            let pi = ctx.cfg.lc.read().unwrap().peer_info.clone();
            if let Some(pi) = pi.as_ref() {
                if let Some(message::Union::MultiClipboards(multi_clipboards)) = &msg.union {
                    if let Some(msg_out) = crate::clipboard::get_msg_if_not_support_multi_clip(
                        &pi.version,
                        &pi.platform,
                        multi_clipboards,
                    ) {
                        let _ = ctx.tx.send(Data::Message(msg_out));
                        return;
                    }
                }
            }
            let _ = ctx.tx.send(Data::Message(msg));
        }
    }
}
