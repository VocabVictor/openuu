use super::*;

impl<T: InvokeUiSession> Remote<T> {
    pub(super) fn handle_clipboard(&mut self, cb: Clipboard) {
        let clipboard_allowed = {
            let lc = self.handler.lc.read().unwrap();
            !lc.disable_clipboard.v && !lc.get_toggle_option("view-only")
        };
        if clipboard_allowed {
            #[cfg(all(
                feature = "flutter",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            if self.handler.is_text_clipboard_required()
                && crate::clipboard::is_sync_clipboard_between_sessions_enabled()
            {
                let mut msg = Message::new();
                msg.set_clipboard(cb.clone());
                let session_id = self.handler.lc.read().unwrap().session_id;
                crate::flutter::send_clipboard_msg_to_other_sessions(msg, session_id);
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            update_clipboard(vec![cb], ClipboardSide::Client);
            #[cfg(target_os = "ios")]
            {
                let content = if cb.compress {
                    hbb_common::compress::decompress(&cb.content)
                } else {
                    cb.content.into()
                };
                if let Ok(content) = String::from_utf8(content) {
                    self.handler.clipboard(content);
                }
            }
            #[cfg(target_os = "android")]
            crate::clipboard::handle_msg_clipboard(cb);
        }
    }

    pub(super) fn handle_multi_clipboards(&mut self, _mcb: MultiClipboards) {
        let clipboard_allowed = {
            let lc = self.handler.lc.read().unwrap();
            !lc.disable_clipboard.v && !lc.get_toggle_option("view-only")
        };
        if clipboard_allowed {
            #[cfg(all(
                feature = "flutter",
                not(any(target_os = "android", target_os = "ios"))
            ))]
            if self.handler.is_text_clipboard_required()
                && crate::clipboard::is_sync_clipboard_between_sessions_enabled()
            {
                let mut msg = Message::new();
                msg.set_multi_clipboards(_mcb.clone());
                let session_id = self.handler.lc.read().unwrap().session_id;
                crate::flutter::send_clipboard_msg_to_other_sessions(msg, session_id);
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            update_clipboard(_mcb.clipboards, ClipboardSide::Client);
            #[cfg(target_os = "ios")]
            {
                if let Some(cb) = _mcb
                    .clipboards
                    .iter()
                    .find(|c| c.format.enum_value() == Ok(ClipboardFormat::Text))
                {
                    let content = if cb.compress {
                        hbb_common::compress::decompress(&cb.content)
                    } else {
                        cb.content.to_vec()
                    };
                    if let Ok(content) = String::from_utf8(content) {
                        self.handler.clipboard(content);
                    }
                }
            }
            #[cfg(target_os = "android")]
            crate::clipboard::handle_msg_multi_clipboards(_mcb);
        }
    }
}
