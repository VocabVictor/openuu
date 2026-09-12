use super::*;

impl<T: InvokeUiSession> Remote<T> {
    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    pub(super) async fn handle_local_clipboard_msg(
        &self,
        peer: &mut Stream,
        msg: Option<clipboard::ClipboardFile>,
    ) {
        match msg {
            Some(clip) => match clip {
                clipboard::ClipboardFile::NotifyCallback {
                    r#type,
                    title,
                    text,
                } => {
                    self.handler.msgbox(&r#type, &title, &text, "");
                }
                _ => {
                    let is_stopping_allowed = clip.is_stopping_allowed();
                    let server_file_transfer_enabled =
                        *self.handler.server_file_transfer_enabled.read().unwrap();
                    let file_transfer_enabled =
                        self.handler.lc.read().unwrap().enable_file_copy_paste.v;
                    let view_only = self.handler.lc.read().unwrap().get_toggle_option("view-only");
                    let stop = is_stopping_allowed
                        && (view_only
                            || !self.is_connected
                            || !(server_file_transfer_enabled && file_transfer_enabled));
                    log::debug!(
                        "Process clipboard message from system, view_only: {}, stop: {}, is_stopping_allowed: {}, server_file_transfer_enabled: {}, file_transfer_enabled: {}",
                        view_only, stop, is_stopping_allowed, server_file_transfer_enabled, file_transfer_enabled
                    );
                    if stop {
                        #[cfg(target_os = "windows")]
                        {
                            ContextSend::set_is_stopped();
                        }
                    } else {
                        #[cfg(target_os = "windows")]
                        if let Err(e) = ContextSend::make_sure_enabled() {
                            log::error!("failed to restart clipboard context: {}", e);
                            // to-do: Show msgbox with "Don't show again" option
                        };
                        log::debug!("Send system clipboard message to remote");
                        let msg = crate::clipboard_file::clip_2_msg(clip);
                        allow_err!(peer.send(&msg).await);
                    }
                }
            },
            None => {
                // unreachable!()
            }
        }
    }
}

impl<T: InvokeUiSession> Remote<T> {

    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    pub(super) async fn handle_cliprdr_msg(&mut self, clip: base::message_proto::Cliprdr, _peer: &mut Stream) {
        log::debug!("handling cliprdr msg from server peer");
        #[cfg(feature = "flutter")]
        if let Some(base::message_proto::cliprdr::Union::FormatList(_)) = &clip.union {
            if self.client_conn_id
                != clipboard::get_client_conn_id(&crate::flutter::get_cur_peer_id()).unwrap_or(0)
            {
                return;
            }
        }

        let Some(clip) = crate::clipboard_file::msg_2_clip(clip) else {
            log::warn!("failed to decode cliprdr msg from server peer");
            return;
        };

        let is_stopping_allowed = clip.is_beginning_message();
        let file_transfer_enabled = self.handler.is_file_clipboard_required();
        let stop = is_stopping_allowed && !file_transfer_enabled;
        log::debug!(
                "Process clipboard message from server peer, stop: {}, is_stopping_allowed: {}, file_transfer_enabled: {}",
                stop, is_stopping_allowed, file_transfer_enabled);
        if !stop {
            #[cfg(any(
                target_os = "windows",
                all(target_os = "macos", feature = "unix-file-copy-paste")
            ))]
            if let Err(e) = ContextSend::make_sure_enabled() {
                log::error!("failed to restart clipboard context: {}", e);
            };
            #[cfg(target_os = "windows")]
            {
                let _ = ContextSend::proc(|context| -> ResultType<()> {
                    context
                        .server_clip_file(self.client_conn_id, clip)
                        .map_err(|e| e.into())
                });
            }
            #[cfg(feature = "unix-file-copy-paste")]
            if crate::is_support_file_copy_paste_num(self.handler.lc.read().unwrap().version) {
                let mut out_msgs = vec![];

                #[cfg(target_os = "macos")]
                if clipboard::platform::unix::macos::should_handle_msg(&clip) {
                    if let Err(e) = ContextSend::proc(|context| -> ResultType<()> {
                        context
                            .server_clip_file(self.client_conn_id, clip)
                            .map_err(|e| e.into())
                    }) {
                        log::error!("failed to handle cliprdr msg: {}", e);
                    }
                } else {
                    out_msgs = unix_file_clip::serve_clip_messages(
                        ClipboardSide::Client,
                        clip,
                        self.client_conn_id,
                    );
                }

                #[cfg(not(target_os = "macos"))]
                {
                    out_msgs = unix_file_clip::serve_clip_messages(
                        ClipboardSide::Client,
                        clip,
                        self.client_conn_id,
                    );
                }

                for msg in out_msgs.into_iter() {
                    allow_err!(_peer.send(&msg).await);
                }
            }
        }
    }
}
