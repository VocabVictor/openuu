use super::*;

#[cfg(not(target_os = "ios"))]
pub fn update_text_clipboard_required() {
    let is_required = sessions::get_sessions()
        .iter()
        .any(|s| s.is_default() && s.is_text_clipboard_required());
    #[cfg(target_os = "android")]
    let _ = scrap::android::ffi::call_clipboard_manager_enable_client_clipboard(is_required);
    Client::set_is_text_clipboard_required(is_required);
}

#[cfg(feature = "unix-file-copy-paste")]
pub fn update_file_clipboard_required() {
    let is_required = sessions::get_sessions()
        .iter()
        .any(|s| s.is_default() && s.is_file_clipboard_required());
    Client::set_is_file_clipboard_required(is_required);
}

#[cfg(not(target_os = "ios"))]
pub fn send_clipboard_msg(msg: Message, _is_file: bool) {
    send_clipboard_msg_impl(msg, _is_file, None);
}

// `except_session_id` is the session the content came from, to avoid sending it back.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn send_clipboard_msg_to_other_sessions(msg: Message, except_session_id: u64) {
    send_clipboard_msg_impl(msg, false, Some(except_session_id));
}

#[cfg(not(target_os = "ios"))]
fn send_clipboard_msg_impl(msg: Message, _is_file: bool, except_session_id: Option<u64>) {
    for s in sessions::get_sessions() {
        if !s.is_default() {
            continue;
        }
        if let Some(except_session_id) = except_session_id {
            if s.lc.read().unwrap().session_id == except_session_id {
                continue;
            }
        }
        #[cfg(feature = "unix-file-copy-paste")]
        if _is_file {
            if crate::is_support_file_copy_paste_num(s.lc.read().unwrap().version)
                && s.is_file_clipboard_required()
            {
                s.send(Data::Message(msg.clone()));
            }
            continue;
        }
        if s.is_text_clipboard_required() {
            // Check if the client supports multi clipboards
            if let Some(message::Union::MultiClipboards(multi_clipboards)) = &msg.union {
                let version = s.ui_handler.peer_info.read().unwrap().version.clone();
                let platform = s.ui_handler.peer_info.read().unwrap().platform.clone();
                if let Some(msg_out) = crate::clipboard::get_msg_if_not_support_multi_clip(
                    &version,
                    &platform,
                    multi_clipboards,
                ) {
                    s.send(Data::Message(msg_out));
                    continue;
                }
            }
            s.send(Data::Message(msg.clone()));
        }
    }
}
