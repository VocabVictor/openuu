use super::*;

impl Connection {
    pub(super) fn handle_clipboard(&mut self, cb: Clipboard) {
        if self.should_handle_text_clipboard_message() && self.clipboard_enabled() {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            update_clipboard(vec![cb], ClipboardSide::Host);
            // ios as the controlled side is actually not supported for now.
            // The following code is only used to preserve the logic of handling text clipboard on mobile.
            #[cfg(target_os = "ios")]
            {
                let content = if cb.compress {
                    hbb_common::compress::decompress(&cb.content)
                } else {
                    cb.content.into()
                };
                if let Ok(content) = String::from_utf8(content) {
                    let data =
                        HashMap::from([("name", "clipboard"), ("content", &content)]);
                    if let Ok(data) = serde_json::to_string(&data) {
                        let _ = crate::flutter::push_global_event(
                            crate::flutter::APP_TYPE_MAIN,
                            data,
                        );
                    }
                }
            }
            #[cfg(target_os = "android")]
            crate::clipboard::handle_msg_clipboard(cb);
        }
    }

    pub(super) fn handle_multi_clipboards(&mut self, _mcb: MultiClipboards) {
        if self.should_handle_text_clipboard_message() && self.clipboard_enabled() {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            update_clipboard(_mcb.clipboards, ClipboardSide::Host);
            #[cfg(target_os = "android")]
            crate::clipboard::handle_msg_multi_clipboards(_mcb);
        }
    }

    #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
    pub(super) async fn handle_cliprdr(&mut self, clip: Cliprdr) {
        if let Some(cliprdr::Union::Files(files)) = &clip.union {
            self.post_file_audit(
                FileAuditType::RemoteReceive,
                "",
                files
                    .files
                    .iter()
                    .map(|f| (f.name.clone(), f.size as i64))
                    .collect::<Vec<(String, i64)>>(),
                json!({}),
            );
        } else if let Some(clip) = msg_2_clip(clip) {
            #[cfg(target_os = "windows")]
            {
                self.send_to_cm(ipc::Data::ClipboardFile(clip));
            }
            #[cfg(feature = "unix-file-copy-paste")]
            if crate::is_support_file_copy_paste(&self.lr.version) {
                let mut out_msgs = vec![];

                #[cfg(target_os = "macos")]
                if clipboard::platform::unix::macos::should_handle_msg(&clip) {
                    if let Err(e) = clipboard::ContextSend::make_sure_enabled() {
                        log::error!("failed to restart clipboard context: {}", e);
                    } else {
                        let _ =
                            clipboard::ContextSend::proc(|context| -> ResultType<()> {
                                context
                                    .server_clip_file(self.inner.id(), clip)
                                    .map_err(|e| e.into())
                            });
                    }
                } else {
                    out_msgs = unix_file_clip::serve_clip_messages(
                        ClipboardSide::Host,
                        clip,
                        self.inner.id(),
                    );
                }

                #[cfg(not(target_os = "macos"))]
                {
                    out_msgs = unix_file_clip::serve_clip_messages(
                        ClipboardSide::Host,
                        clip,
                        self.inner.id(),
                    );
                }

                for msg in out_msgs.into_iter() {
                    if let Some(message::Union::Cliprdr(cliprdr)) = msg.union.as_ref() {
                        if let Some(cliprdr::Union::Files(files)) =
                            cliprdr.union.as_ref()
                        {
                            self.post_file_audit(
                                FileAuditType::RemoteSend,
                                "",
                                files
                                    .files
                                    .iter()
                                    .map(|f| (f.name.clone(), f.size as i64))
                                    .collect::<Vec<(String, i64)>>(),
                                json!({}),
                            );
                            continue;
                        }
                    }
                    self.send(msg).await;
                }
            }
        }
    }
}
