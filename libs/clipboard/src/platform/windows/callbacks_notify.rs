#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use super::*;

pub fn create_cliprdr_context(
    enable_files: bool,
    enable_others: bool,
    response_wait_timeout_secs: u32,
) -> ResultType<Box<CliprdrClientContext>> {
    Ok(CliprdrClientContext::create(
        enable_files,
        enable_others,
        response_wait_timeout_secs,
        Some(notify_callback),
        Some(handle_clipboard_files),
        Some(client_format_list),
        Some(client_format_list_response),
        Some(client_format_data_request),
        Some(client_format_data_response),
        Some(client_file_contents_request),
        Some(client_file_contents_response),
    )?)
}

extern "C" fn notify_callback(conn_id: UINT32, msg: *const NOTIFICATION_MESSAGE) -> UINT {
    log::debug!("notify_callback called");
    let data = unsafe {
        let msg = &*msg;
        let details = if msg.details.is_null() {
            Ok("")
        } else {
            CStr::from_ptr(msg.details as _).to_str()
        };
        match (CStr::from_ptr(msg.msg as _).to_str(), details) {
            (Ok(m), Ok(d)) => {
                let msgtype = format!(
                    "custom-{}-nocancel-nook-hasclose",
                    if msg.r#type == 0 {
                        "info"
                    } else if msg.r#type == 1 {
                        "warn"
                    } else {
                        "error"
                    }
                );
                let title = "Clipboard";
                let text = if d.is_empty() {
                    m.to_string()
                } else {
                    format!("{} {}", m, d)
                };
                ClipboardFile::NotifyCallback {
                    r#type: msgtype,
                    title: title.to_string(),
                    text,
                }
            }
            _ => {
                log::error!("notify_callback: failed to convert msg");
                return ERR_CODE_INVALID_PARAMETER;
            }
        }
    };
    // no need to handle result here
    allow_err!(send_data(conn_id as _, data));

    0
}

extern "C" fn handle_clipboard_files(
    conn_id: UINT32,
    n_files: size_t,
    file_names: *mut *mut WCHAR,
) -> UINT {
    if n_files == 0 {
        return 0;
    }

    let data = unsafe {
        let mut files = Vec::new();
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        for i in 0..n_files {
            let file_name_ptr = *file_names.offset(i as isize);
            if !file_name_ptr.is_null() {
                let mut len = 0;
                while *file_name_ptr.offset(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(file_name_ptr, len as usize);
                let os_string = OsString::from_wide(slice);
                match os_string.to_str() {
                    Some(n) => match std::fs::metadata(n) {
                        Ok(meta) => {
                            if meta.is_file() {
                                files.push((n.to_owned(), meta.len()));
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "handle_clipboard_files: Failed to get metadata for file '{}': {}",
                                n,
                                e
                            );
                        }
                    },
                    None => {
                        log::warn!("handle_clipboard_files: Failed to convert file name to UTF-8");
                    }
                };
            }
        }
        if files.is_empty() {
            return 0;
        }

        ClipboardFile::Files { files }
    };
    // no need to handle result here
    allow_err!(send_data(conn_id as _, data));

    0
}
