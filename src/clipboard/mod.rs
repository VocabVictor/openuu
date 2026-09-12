#[cfg(not(target_os = "android"))]
use arboard::{ClipboardData, ClipboardFormat};
#[cfg(target_os = "linux")]
use arboard::{LinuxClipboardKind, SetExtLinux};
use hbb_common::{bail, log, ResultType};
use base::message_proto::*;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

mod check;
pub use check::*;
mod context;
pub use context::*;

pub const CLIPBOARD_NAME: &'static str = "clipboard";
#[cfg(feature = "unix-file-copy-paste")]
pub const FILE_CLIPBOARD_NAME: &'static str = "file-clipboard";
pub const CLIPBOARD_INTERVAL: u64 = 333;

pub const OPTION_ALLOW_SYNC_CLIPBOARD_BETWEEN_SESSIONS: &str =
    "allow-sync-clipboard-between-sessions";

#[cfg(all(feature = "flutter", not(any(target_os = "android", target_os = "ios"))))]
pub fn is_sync_clipboard_between_sessions_enabled() -> bool {
    hbb_common::config::option2bool(
        OPTION_ALLOW_SYNC_CLIPBOARD_BETWEEN_SESSIONS,
        &hbb_common::config::LocalConfig::get_option(OPTION_ALLOW_SYNC_CLIPBOARD_BETWEEN_SESSIONS),
    )
}

// This format is used to store the flag in the clipboard.
const RUSTDESK_CLIPBOARD_OWNER_FORMAT: &'static str = "dyn.com.rustdesk.owner";

// Add special format for Excel XML Spreadsheet
const CLIPBOARD_FORMAT_EXCEL_XML_SPREADSHEET: &'static str = "XML Spreadsheet";

#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref ARBOARD_MTX: Arc<Mutex<()>> = Arc::new(Mutex::new(()));
    // cache the clipboard msg
    static ref LAST_MULTI_CLIPBOARDS: Arc<Mutex<MultiClipboards>> = Arc::new(Mutex::new(MultiClipboards::new()));
    // For updating in server and getting content in cm.
    // Clipboard on Linux is "server--clients" mode.
    // The clipboard content is owned by the server and passed to the clients when requested.
    // Plain text is the only exception, it does not require the server to be present.
    static ref CLIPBOARD_CTX: Arc<Mutex<Option<ClipboardContext>>> = Arc::new(Mutex::new(None));
}

#[cfg(not(target_os = "android"))]
const CLIPBOARD_GET_MAX_RETRY: usize = 3;
#[cfg(not(target_os = "android"))]
const CLIPBOARD_GET_RETRY_INTERVAL_DUR: Duration = Duration::from_millis(33);

#[cfg(not(target_os = "android"))]
fn valid_rgba_dimensions(width: i32, height: i32, data_len: usize) -> Option<(usize, usize)> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let expected_len = width.checked_mul(height)?.checked_mul(4)?;
    (data_len == expected_len).then_some((width, height))
}

#[cfg(not(target_os = "android"))]
const SUPPORTED_FORMATS: &[ClipboardFormat] = &[
    ClipboardFormat::Text,
    ClipboardFormat::Html,
    ClipboardFormat::Rtf,
    ClipboardFormat::ImageRgba,
    ClipboardFormat::ImagePng,
    ClipboardFormat::ImageSvg,
    #[cfg(feature = "unix-file-copy-paste")]
    ClipboardFormat::FileUrl,
    ClipboardFormat::Special(CLIPBOARD_FORMAT_EXCEL_XML_SPREADSHEET),
    ClipboardFormat::Special(RUSTDESK_CLIPBOARD_OWNER_FORMAT),
];

pub fn is_support_multi_clipboard(peer_version: &str, peer_platform: &str) -> bool {
    use hbb_common::get_version_number;
    if get_version_number(peer_version) < get_version_number("1.3.0") {
        return false;
    }
    if ["", &hbb_common::whoami::Platform::Ios.to_string()].contains(&peer_platform) {
        return false;
    }
    if "Android" == peer_platform && get_version_number(peer_version) < get_version_number("1.3.3")
    {
        return false;
    }
    true
}

#[cfg(not(target_os = "android"))]
pub fn get_current_clipboard_msg(
    peer_version: &str,
    peer_platform: &str,
    side: ClipboardSide,
) -> Option<Message> {
    let mut multi_clipboards = LAST_MULTI_CLIPBOARDS.lock().unwrap();
    if multi_clipboards.clipboards.is_empty() {
        let mut ctx = ClipboardContext::new().ok()?;
        *multi_clipboards = proto::create_multi_clipboards(ctx.get(side, true).ok()?);
    }
    if multi_clipboards.clipboards.is_empty() {
        return None;
    }

    if is_support_multi_clipboard(peer_version, peer_platform) {
        let mut msg = Message::new();
        msg.set_multi_clipboards(multi_clipboards.clone());
        Some(msg)
    } else {
        // Find the first text clipboard and send it.
        multi_clipboards
            .clipboards
            .iter()
            .find(|c| c.format.enum_value() == Ok(base::message_proto::ClipboardFormat::Text))
            .map(|c| {
                let mut msg = Message::new();
                msg.set_clipboard(c.clone());
                msg
            })
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum ClipboardSide {
    Host,
    Client,
}

impl ClipboardSide {
    // 01: the clipboard is owned by the host
    // 10: the clipboard is owned by the client
    fn get_owner_data(&self) -> Vec<u8> {
        match self {
            ClipboardSide::Host => vec![0b01],
            ClipboardSide::Client => vec![0b10],
        }
    }

    fn is_owner(&self, data: &[u8]) -> bool {
        if data.len() == 0 {
            return false;
        }
        data[0] & 0b11 != 0
    }
}

impl std::fmt::Display for ClipboardSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClipboardSide::Host => write!(f, "host"),
            ClipboardSide::Client => write!(f, "client"),
        }
    }
}

pub use proto::get_msg_if_not_support_multi_clip;
mod proto {
    #[cfg(not(target_os = "android"))]
    use arboard::ClipboardData;
    use hbb_common::{
        compress::{compress as compress_func, decompress},
    };
    use base::message_proto::{Clipboard, ClipboardFormat, Message, MultiClipboards};

    fn plain_to_proto(s: String, format: ClipboardFormat) -> Clipboard {
        let compressed = compress_func(s.as_bytes());
        let compress = compressed.len() < s.as_bytes().len();
        let content = if compress {
            compressed
        } else {
            s.bytes().collect::<Vec<u8>>()
        };
        Clipboard {
            compress,
            content: content.into(),
            format: format.into(),
            ..Default::default()
        }
    }

    #[cfg(not(target_os = "android"))]
    fn image_to_proto(a: arboard::ImageData) -> Clipboard {
        match &a {
            arboard::ImageData::Rgba(rgba) => {
                let compressed = compress_func(&a.bytes());
                let compress = compressed.len() < a.bytes().len();
                let content = if compress {
                    compressed
                } else {
                    a.bytes().to_vec()
                };
                Clipboard {
                    compress,
                    content: content.into(),
                    width: rgba.width as _,
                    height: rgba.height as _,
                    format: ClipboardFormat::ImageRgba.into(),
                    ..Default::default()
                }
            }
            arboard::ImageData::Png(png) => Clipboard {
                compress: false,
                content: png.to_owned().to_vec().into(),
                format: ClipboardFormat::ImagePng.into(),
                ..Default::default()
            },
            arboard::ImageData::Svg(_) => {
                let compressed = compress_func(&a.bytes());
                let compress = compressed.len() < a.bytes().len();
                let content = if compress {
                    compressed
                } else {
                    a.bytes().to_vec()
                };
                Clipboard {
                    compress,
                    content: content.into(),
                    format: ClipboardFormat::ImageSvg.into(),
                    ..Default::default()
                }
            }
        }
    }

    fn special_to_proto(d: Vec<u8>, s: String) -> Clipboard {
        let compressed = compress_func(&d);
        let compress = compressed.len() < d.len();
        let content = if compress {
            compressed
        } else {
            d
        };
        Clipboard {
            compress,
            content: content.into(),
            format: ClipboardFormat::Special.into(),
            special_name: s,
            ..Default::default()
        }
    }

    #[cfg(not(target_os = "android"))]
    fn clipboard_data_to_proto(data: ClipboardData) -> Option<Clipboard> {
        let d = match data {
            ClipboardData::Text(s) => plain_to_proto(s, ClipboardFormat::Text),
            ClipboardData::Rtf(s) => plain_to_proto(s, ClipboardFormat::Rtf),
            ClipboardData::Html(s) => plain_to_proto(s, ClipboardFormat::Html),
            ClipboardData::Image(a) => image_to_proto(a),
            ClipboardData::Special((s, d)) => special_to_proto(d, s),
            _ => return None,
        };
        Some(d)
    }

    #[cfg(not(target_os = "android"))]
    pub fn create_multi_clipboards(vec_data: Vec<ClipboardData>) -> MultiClipboards {
        MultiClipboards {
            clipboards: vec_data
                .into_iter()
                .filter_map(clipboard_data_to_proto)
                .collect(),
            ..Default::default()
        }
    }

    #[cfg(not(target_os = "android"))]
    fn from_clipboard(clipboard: Clipboard) -> Option<ClipboardData> {
        let data = if clipboard.compress {
            decompress(&clipboard.content)
        } else {
            clipboard.content.into()
        };
        match clipboard.format.enum_value() {
            Ok(ClipboardFormat::Text) => String::from_utf8(data).ok().map(ClipboardData::Text),
            Ok(ClipboardFormat::Rtf) => String::from_utf8(data).ok().map(ClipboardData::Rtf),
            Ok(ClipboardFormat::Html) => String::from_utf8(data).ok().map(ClipboardData::Html),
            Ok(ClipboardFormat::ImageRgba) => {
                let (width, height) =
                    super::valid_rgba_dimensions(clipboard.width, clipboard.height, data.len())?;
                Some(ClipboardData::Image(arboard::ImageData::rgba(
                    width,
                    height,
                    data.into(),
                )))
            }
            Ok(ClipboardFormat::ImagePng) => {
                Some(ClipboardData::Image(arboard::ImageData::png(data.into())))
            }
            Ok(ClipboardFormat::ImageSvg) => Some(ClipboardData::Image(arboard::ImageData::svg(
                std::str::from_utf8(&data).unwrap_or_default(),
            ))),
            Ok(ClipboardFormat::Special) => {
                Some(ClipboardData::Special((clipboard.special_name, data)))
            }
            _ => None,
        }
    }

    #[cfg(not(target_os = "android"))]
    pub fn from_multi_clipboards(multi_clipboards: Vec<Clipboard>) -> Vec<ClipboardData> {
        multi_clipboards
            .into_iter()
            .filter_map(from_clipboard)
            .collect()
    }

    pub fn get_msg_if_not_support_multi_clip(
        version: &str,
        platform: &str,
        multi_clipboards: &MultiClipboards,
    ) -> Option<Message> {
        if crate::clipboard::is_support_multi_clipboard(version, platform) {
            return None;
        }

        // Find the first text clipboard and send it.
        multi_clipboards
            .clipboards
            .iter()
            .find(|c| c.format.enum_value() == Ok(ClipboardFormat::Text))
            .map(|c| {
                let mut msg = Message::new();
                msg.set_clipboard(c.clone());
                msg
            })
    }

    #[cfg(all(test, not(target_os = "android")))]
    mod tests {
        use super::{from_clipboard, special_to_proto};
        use arboard::ClipboardData;

        #[test]
        fn preserves_uncompressed_special_clipboard_data() {
            let data = vec![0x01, 0x02, 0x03];
            let name = "custom-format".to_owned();

            let clipboard = special_to_proto(data.clone(), name.clone());

            assert!(!clipboard.compress);
            assert_eq!(clipboard.content.as_ref(), data.as_slice());
            assert_eq!(clipboard.special_name, name);
            assert!(matches!(
                from_clipboard(clipboard),
                Some(ClipboardData::Special((restored_name, restored_data)))
                    if restored_name == name && restored_data == data
            ));
        }
    }
}

#[cfg(all(test, not(target_os = "android")))]
mod rgba_tests {
    use super::valid_rgba_dimensions;

    #[test]
    fn validates_dimensions_against_content_length() {
        assert_eq!(valid_rgba_dimensions(1, 1, 4), Some((1, 1)));
        assert_eq!(valid_rgba_dimensions(1, 1, 3), None);
        assert_eq!(valid_rgba_dimensions(-1, 1, 4), None);
        assert_eq!(valid_rgba_dimensions(0, 1, 0), None);
        assert_eq!(valid_rgba_dimensions(i32::MAX, i32::MAX, 4), None);
        #[cfg(target_pointer_width = "32")]
        assert_eq!(valid_rgba_dimensions(i32::MAX, 2, 0), None);
    }
}

#[cfg(target_os = "android")]
pub fn handle_msg_clipboard(mut cb: Clipboard) {
    use hbb_common::protobuf::Message;

    if cb.compress {
        cb.content = bytes::Bytes::from(hbb_common::compress::decompress(&cb.content));
    }
    let multi_clips = MultiClipboards {
        clipboards: vec![cb],
        ..Default::default()
    };
    if let Ok(bytes) = multi_clips.write_to_bytes() {
        let _ = scrap::android::ffi::call_clipboard_manager_update_clipboard(&bytes);
    }
}

#[cfg(target_os = "android")]
pub fn handle_msg_multi_clipboards(mut mcb: MultiClipboards) {
    use hbb_common::protobuf::Message;

    for cb in mcb.clipboards.iter_mut() {
        if cb.compress {
            cb.content = bytes::Bytes::from(hbb_common::compress::decompress(&cb.content));
        }
    }
    if let Ok(bytes) = mcb.write_to_bytes() {
        let _ = scrap::android::ffi::call_clipboard_manager_update_clipboard(&bytes);
    }
}

#[cfg(target_os = "android")]
pub fn get_clipboards_msg(client: bool) -> Option<Message> {
    let mut clipboards = scrap::android::ffi::get_clipboards(client)?;
    let mut msg = Message::new();
    for c in &mut clipboards.clipboards {
        let compressed = hbb_common::compress::compress(&c.content);
        let compress = compressed.len() < c.content.len();
        if compress {
            c.content = compressed.into();
        }
        c.compress = compress;
    }
    msg.set_multi_clipboards(clipboards);
    Some(msg)
}

// We need this mod to notify multiple subscribers when the clipboard changes.
// Because only one clipboard master(listener) can trigger the clipboard change event multiple listeners are created on Linux(x11).
// https://github.com/rustdesk-org/clipboard-master/blob/4fb62e5b62fb6350d82b571ec7ba94b3cd466695/src/master/x11.rs#L226
#[cfg(not(target_os = "android"))]
pub mod clipboard_listener {
    use clipboard_master::{CallbackResult, ClipboardHandler, Master, Shutdown};
    use hbb_common::{bail, log, ResultType};
    use std::{
        collections::HashMap,
        io,
        sync::mpsc::{channel, Sender},
        sync::{Arc, Mutex},
        thread::JoinHandle,
    };

    lazy_static::lazy_static! {
        pub static ref CLIPBOARD_LISTENER: Arc<Mutex<ClipboardListener>> = Default::default();
    }

    struct Handler {
        subscribers: Arc<Mutex<HashMap<String, Sender<CallbackResult>>>>,
    }

    impl ClipboardHandler for Handler {
        fn on_clipboard_change(&mut self) -> CallbackResult {
            let sub_lock = self.subscribers.lock().unwrap();
            for tx in sub_lock.values() {
                tx.send(CallbackResult::Next).ok();
            }
            CallbackResult::Next
        }

        fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
            let msg = format!("Clipboard listener error: {}", error);
            let sub_lock = self.subscribers.lock().unwrap();
            for tx in sub_lock.values() {
                tx.send(CallbackResult::StopWithError(io::Error::new(
                    io::ErrorKind::Other,
                    msg.clone(),
                )))
                .ok();
            }
            CallbackResult::Next
        }
    }

    #[derive(Default)]
    pub struct ClipboardListener {
        subscribers: Arc<Mutex<HashMap<String, Sender<CallbackResult>>>>,
        handle: Option<(Shutdown, JoinHandle<()>)>,
    }

    pub fn subscribe(name: String, tx: Sender<CallbackResult>) -> ResultType<()> {
        log::info!("Subscribe clipboard listener: {}", &name);
        let mut listener_lock = CLIPBOARD_LISTENER.lock().unwrap();
        listener_lock
            .subscribers
            .lock()
            .unwrap()
            .insert(name.clone(), tx);

        cleanup_stale_listener(&mut listener_lock);
        if listener_lock.handle.is_none() {
            log::info!("Start clipboard listener thread");
            let handler = Handler {
                subscribers: listener_lock.subscribers.clone(),
            };
            let (tx_start_res, rx_start_res) = channel();
            let h = start_clipboard_master_thread(handler, tx_start_res);
            let shutdown = match rx_start_res.recv() {
                Ok((Some(s), _)) => s,
                Ok((None, err)) => {
                    bail!(err);
                }

                Err(e) => {
                    bail!("Failed to create clipboard listener: {}", e);
                }
            };
            listener_lock.handle = Some((shutdown, h));
            log::info!("Clipboard listener thread started");
        }

        log::info!("Clipboard listener subscribed: {}", name);
        Ok(())
    }

    fn cleanup_stale_listener(listener: &mut ClipboardListener) {
        if !listener
            .handle
            .as_ref()
            .map(|(_, h)| h.is_finished())
            .unwrap_or(false)
        {
            return;
        }
        if let Some((shutdown, h)) = listener.handle.take() {
            log::warn!("Cleaning up stale clipboard listener handle");
            if let Err(e) = h.join() {
                log::error!("Clipboard listener thread panicked during stale cleanup: {:?}", e);
            }
            drop(shutdown);
        }
    }

    pub fn unsubscribe(name: &str) {
        log::info!("Unsubscribe clipboard listener: {}", name);
        let mut listener_lock = CLIPBOARD_LISTENER.lock().unwrap();
        let is_empty = {
            let mut sub_lock = listener_lock.subscribers.lock().unwrap();
            if let Some(tx) = sub_lock.remove(name) {
                tx.send(CallbackResult::Stop).ok();
            }
            sub_lock.is_empty()
        };
        if is_empty {
            if let Some((shutdown, h)) = listener_lock.handle.take() {
                log::info!("Stop clipboard listener thread");
                shutdown.signal();
                h.join().ok();
                log::info!("Clipboard listener thread stopped");
            }
        }
        log::info!("Clipboard listener unsubscribed: {}", name);
    }

    fn start_clipboard_master_thread(
        handler: impl ClipboardHandler + Send + 'static,
        tx_start_res: Sender<(Option<Shutdown>, String)>,
    ) -> JoinHandle<()> {
        // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage#:~:text=The%20window%20must%20belong%20to%20the%20current%20thread.
        let h = std::thread::spawn(move || match Master::new(handler) {
            Ok(mut master) => {
                tx_start_res
                    .send((Some(master.shutdown_channel()), "".to_owned()))
                    .ok();
                log::debug!("Clipboard listener started");
                if let Err(err) = master.run() {
                    log::error!("Failed to run clipboard listener: {}", err);
                } else {
                    log::debug!("Clipboard listener stopped");
                }
            }
            Err(err) => {
                tx_start_res
                    .send((
                        None,
                        format!("Failed to create clipboard listener: {}", err),
                    ))
                    .ok();
            }
        });
        h
    }
}
