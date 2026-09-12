use super::*;

#[cfg(not(target_os = "android"))]
pub fn check_clipboard(
    ctx: &mut Option<ClipboardContext>,
    side: ClipboardSide,
    force: bool,
) -> Option<Message> {
    let (msg, clipboards) = read_clipboard_message(ctx, side, force)?;
    *LAST_MULTI_CLIPBOARDS.lock().unwrap() = clipboards;
    Some(msg)
}

#[cfg(target_os = "linux")]
pub fn peek_clipboard(
    ctx: &mut Option<ClipboardContext>,
    side: ClipboardSide,
    force: bool,
) -> Option<Message> {
    let (msg, _) = read_clipboard_message(ctx, side, force)?;
    Some(msg)
}

#[cfg(not(target_os = "android"))]
fn read_clipboard_message(
    ctx: &mut Option<ClipboardContext>,
    side: ClipboardSide,
    force: bool,
) -> Option<(Message, MultiClipboards)> {
    if ctx.is_none() {
        *ctx = ClipboardContext::new().ok();
    }
    let ctx2 = ctx.as_mut()?;
    match ctx2.get(side, force) {
        Ok(content) => {
            if !content.is_empty() {
                let mut msg = Message::new();
                let clipboards = proto::create_multi_clipboards(content);
                msg.set_multi_clipboards(clipboards.clone());
                return Some((msg, clipboards));
            }
        }
        Err(e) => {
            log::error!("Failed to get clipboard content. {}", e);
        }
    }
    None
}

#[cfg(all(feature = "unix-file-copy-paste", target_os = "macos"))]
pub fn is_file_url_set_by_rustdesk(url: &Vec<String>) -> bool {
    if url.len() != 1 {
        return false;
    }
    url.iter()
        .next()
        .map(|s| {
            for prefix in &["file:///tmp/.rustdesk_", "//tmp/.rustdesk_"] {
                if s.starts_with(prefix) {
                    return s[prefix.len()..].parse::<uuid::Uuid>().is_ok();
                }
            }
            false
        })
        .unwrap_or(false)
}

#[cfg(feature = "unix-file-copy-paste")]
pub fn check_clipboard_files(
    ctx: &mut Option<ClipboardContext>,
    side: ClipboardSide,
    force: bool,
) -> Option<Vec<String>> {
    if ctx.is_none() {
        *ctx = ClipboardContext::new().ok();
    }
    let ctx2 = ctx.as_mut()?;
    match ctx2.get_files(side, force) {
        Ok(Some(urls)) => {
            if !urls.is_empty() {
                return Some(urls);
            }
        }
        Err(e) => {
            log::error!("Failed to get clipboard file urls. {}", e);
        }
        _ => {}
    }
    None
}

#[cfg(all(target_os = "linux", feature = "unix-file-copy-paste"))]
pub fn update_clipboard_files(files: Vec<String>, side: ClipboardSide) {
    if !files.is_empty() {
        std::thread::spawn(move || {
            do_update_clipboard_(vec![ClipboardData::FileUrl(files)], side);
        });
    }
}

#[cfg(feature = "unix-file-copy-paste")]
pub fn try_empty_clipboard_files(_side: ClipboardSide, _conn_id: i32) {
    std::thread::spawn(move || {
        if let Err(e) = try_empty_clipboard_files_sync(_side, _conn_id) {
            log::error!("Failed to empty clipboard files: {}", e);
        }
    });
}

#[cfg(feature = "unix-file-copy-paste")]
pub fn try_empty_clipboard_files_sync(_side: ClipboardSide, _conn_id: i32) -> ResultType<()> {
    let mut ctx = CLIPBOARD_CTX.lock().unwrap();
    if ctx.is_none() {
        match ClipboardContext::new() {
            Ok(x) => {
                *ctx = Some(x);
            }
            Err(e) => {
                log::error!("Failed to create clipboard context: {}", e);
                bail!("Failed to create clipboard context: {}", e);
            }
        }
    }
    #[allow(unused_mut)]
    if let Some(mut ctx) = ctx.as_mut() {
        #[cfg(target_os = "linux")]
        {
            use clipboard::platform::unix;
            if unix::fuse::empty_local_files(_side == ClipboardSide::Client, _conn_id) {
                ctx.try_empty_clipboard_files(_side);
            }
        }
        #[cfg(target_os = "macos")]
        {
            ctx.try_empty_clipboard_files(_side);
            // No need to make sure the context is enabled.
            clipboard::ContextSend::proc(|context| -> ResultType<()> {
                if !context.empty_clipboard(_conn_id)? {
                    bail!("Failed to empty clipboard files for conn_id {}", _conn_id);
                }
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn try_empty_clipboard_files(side: ClipboardSide, conn_id: i32) {
    log::debug!("try to empty {} cliprdr for conn_id {}", side, conn_id);
    let _ = clipboard::ContextSend::proc(|context| -> ResultType<()> {
        context.empty_clipboard(conn_id)?;
        Ok(())
    });
}

#[cfg(target_os = "windows")]
pub fn check_clipboard_cm() -> ResultType<MultiClipboards> {
    let mut ctx = CLIPBOARD_CTX.lock().unwrap();
    if ctx.is_none() {
        match ClipboardContext::new() {
            Ok(x) => {
                *ctx = Some(x);
            }
            Err(e) => {
                hbb_common::bail!("Failed to create clipboard context: {}", e);
            }
        }
    }
    if let Some(ctx) = ctx.as_mut() {
        let content = ctx.get(ClipboardSide::Host, false)?;
        let clipboards = proto::create_multi_clipboards(content);
        Ok(clipboards)
    } else {
        hbb_common::bail!("Failed to create clipboard context");
    }
}

#[cfg(not(target_os = "android"))]
fn update_clipboard_(multi_clipboards: Vec<Clipboard>, side: ClipboardSide) {
    let to_update_data = proto::from_multi_clipboards(multi_clipboards);
    if to_update_data.is_empty() {
        return;
    }
    do_update_clipboard_(to_update_data, side);
}

#[cfg(not(target_os = "android"))]
fn do_update_clipboard_(mut to_update_data: Vec<ClipboardData>, side: ClipboardSide) {
    let mut ctx = CLIPBOARD_CTX.lock().unwrap();
    if ctx.is_none() {
        match ClipboardContext::new() {
            Ok(x) => {
                *ctx = Some(x);
            }
            Err(e) => {
                log::error!("Failed to create clipboard context: {}", e);
                return;
            }
        }
    }
    if let Some(ctx) = ctx.as_mut() {
        to_update_data = append_owner_marker(to_update_data, side);
        if let Err(e) = ctx.set(&to_update_data) {
            log::debug!("Failed to set clipboard: {}", e);
        } else {
            log::debug!("{} updated on {}", CLIPBOARD_NAME, side);
        }
    }
}

#[cfg(not(target_os = "android"))]
fn append_owner_marker(mut data: Vec<ClipboardData>, side: ClipboardSide) -> Vec<ClipboardData> {
    data.push(ClipboardData::Special((
        RUSTDESK_CLIPBOARD_OWNER_FORMAT.to_owned(),
        side.get_owner_data(),
    )));
    data
}

#[cfg(target_os = "linux")]
pub fn set_text_clipboard_with_owner_sync(text: &str, side: ClipboardSide) -> ResultType<()> {
    let mut ctx = CLIPBOARD_CTX.lock().unwrap();
    if ctx.is_none() {
        *ctx = Some(ClipboardContext::new()?);
    }
    let clipboard_ctx = match ctx.as_mut() {
        Some(ctx) => ctx,
        None => bail!("Failed to create clipboard context"),
    };
    let data = append_owner_marker(vec![ClipboardData::Text(text.to_owned())], side);
    clipboard_ctx.set_with_owner_marker_for_linux(&data)
}

#[cfg(not(target_os = "android"))]
pub fn update_clipboard(multi_clipboards: Vec<Clipboard>, side: ClipboardSide) {
    std::thread::spawn(move || {
        update_clipboard_(multi_clipboards, side);
    });
}
