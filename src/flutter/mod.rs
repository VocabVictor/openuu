use crate::{
    client::*,
    flutter_ffi::{EventToUI, SessionID},
    ui_session_interface::{io_loop, InvokeUiSession, Session},
};
use flutter_rust_bridge::StreamSink;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::dlopen::{
    symbor::{Library, Symbol},
    Error as LibError,
};
use hbb_common::{
    anyhow::anyhow, bail, config::LocalConfig, get_version_number, log,
    rendezvous_proto::ConnType, ResultType,
};
use base::message_proto::*;
use serde::Serialize;
use serde_json::json;
#[cfg(target_os = "windows")]
use std::io::{Error as IoError, ErrorKind as IoErrorKind};
use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    os::raw::{c_char, c_int, c_void},
    str::FromStr,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
};

mod plugin;
pub use plugin::*;
mod clipboard;
pub use clipboard::*;
mod video_renderer;
mod handler;
mod invoke_ui;
mod session;
pub use session::*;
#[cfg(not(any(target_os = "ios")))]
pub mod connection_manager;
mod events;
pub use events::*;
mod input;
pub use input::*;
mod helpers;
pub use helpers::*;
#[cfg(target_os = "windows")]
use plugin::load_plugin_in_app_path;
use handler::serialize_resolutions;

/// tag "main" for [Desktop Main Page] and [Mobile (Client and Server)] (the mobile don't need multiple windows, only one global event stream is needed)
/// tag "cm" only for [Desktop CM Page]
pub(crate) const APP_TYPE_MAIN: &str = "main";
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub(crate) const APP_TYPE_CM: &str = "cm";
#[cfg(any(target_os = "android", target_os = "ios"))]
pub(crate) const APP_TYPE_CM: &str = "main";

// Do not remove the following constants.
// Uncomment them when they are used.
// pub(crate) const APP_TYPE_DESKTOP_REMOTE: &str = "remote";
// pub(crate) const APP_TYPE_DESKTOP_FILE_TRANSFER: &str = "file transfer";
// pub(crate) const APP_TYPE_DESKTOP_PORT_FORWARD: &str = "port forward";

pub type FlutterSession = Arc<Session<FlutterHandler>>;

lazy_static::lazy_static! {
    pub(crate) static ref CUR_SESSION_ID: RwLock<SessionID> = Default::default(); // For desktop only
    static ref GLOBAL_EVENT_STREAM: RwLock<HashMap<String, StreamSink<String>>> = Default::default(); // rust to dart event channel
}

#[cfg(target_os = "windows")]
lazy_static::lazy_static! {
    pub static ref TEXTURE_RGBA_RENDERER_PLUGIN: Result<Library, LibError> = load_plugin_in_app_path("texture_rgba_renderer_plugin.dll");
}

#[cfg(target_os = "linux")]
lazy_static::lazy_static! {
    pub static ref TEXTURE_RGBA_RENDERER_PLUGIN: Result<Library, LibError> = Library::open("libtexture_rgba_renderer_plugin.so");
}

#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    pub static ref TEXTURE_RGBA_RENDERER_PLUGIN: Result<Library, LibError> = Library::open_self();
}

#[cfg(target_os = "windows")]
lazy_static::lazy_static! {
    pub static ref TEXTURE_GPU_RENDERER_PLUGIN: Result<Library, LibError> = load_plugin_in_app_path("flutter_gpu_texture_renderer_plugin.dll");
}

#[derive(Default)]
struct SessionHandler {
    event_stream: Option<StreamSink<EventToUI>>,
    // displays of current session.
    // We need this variable to check if the display is in use before pushing rgba to flutter.
    displays: Vec<usize>,
    renderer: VideoRenderer,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum RenderType {
    PixelBuffer,
    #[cfg(feature = "vram")]
    Texture,
}

#[derive(Clone)]
pub struct FlutterHandler {
    // ui session id -> display handler data
    session_handlers: Arc<RwLock<HashMap<SessionID, SessionHandler>>>,
    display_rgbas: Arc<RwLock<HashMap<usize, RgbaData>>>,
    peer_info: Arc<RwLock<PeerInfo>>,
    use_texture_render: Arc<AtomicBool>,
}

impl Default for FlutterHandler {
    fn default() -> Self {
        Self {
            session_handlers: Default::default(),
            display_rgbas: Default::default(),
            peer_info: Default::default(),
            use_texture_render: Arc::new(
                AtomicBool::new(crate::ui_interface::use_texture_render()),
            ),
        }
    }
}

#[derive(Default, Clone)]
struct RgbaData {
    // SAFETY: [rgba] is guarded by [rgba_valid], and it's safe to reach [rgba] with `rgba_valid == true`.
    // We must check the `rgba_valid` before reading [rgba].
    data: Vec<u8>,
    valid: bool,
}

pub type FlutterRgbaRendererPluginOnRgba = unsafe extern "C" fn(
    texture_rgba: *mut c_void,
    buffer: *const u8,
    len: c_int,
    width: c_int,
    height: c_int,
    dst_rgba_stride: c_int,
);

#[cfg(feature = "vram")]
pub type FlutterGpuTextureRendererPluginCApiSetTexture =
    unsafe extern "C" fn(output: *mut c_void, texture: *mut c_void);

#[cfg(feature = "vram")]
pub type FlutterGpuTextureRendererPluginCApiGetAdapterLuid = unsafe extern "C" fn() -> i64;

pub(super) type TextureRgbaPtr = usize;

struct DisplaySessionInfo {
    // TextureRgba pointer in flutter native.
    texture_rgba_ptr: TextureRgbaPtr,
    size: (usize, usize),
    #[cfg(feature = "vram")]
    gpu_output_ptr: usize,
    notify_render_type: Option<RenderType>,
}

// Video Texture Renderer in Flutter
#[derive(Clone)]
struct VideoRenderer {
    is_support_multi_ui_session: bool,
    map_display_sessions: Arc<RwLock<HashMap<usize, DisplaySessionInfo>>>,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    on_rgba_func: Option<Symbol<'static, FlutterRgbaRendererPluginOnRgba>>,
    #[cfg(feature = "vram")]
    on_texture_func: Option<Symbol<'static, FlutterGpuTextureRendererPluginCApiSetTexture>>,
}

// Server Side

pub fn get_cur_session_id() -> SessionID {
    CUR_SESSION_ID.read().unwrap().clone()
}

pub fn get_cur_peer_id() -> String {
    sessions::get_peer_id_by_session_id(&get_cur_session_id(), ConnType::DEFAULT_CONN)
        .unwrap_or("".to_string())
}

pub fn set_cur_session_id(session_id: SessionID) {
    if get_cur_session_id() != session_id {
        *CUR_SESSION_ID.write().unwrap() = session_id;
    }
}

#[inline]
pub fn get_cur_session() -> Option<FlutterSession> {
    sessions::get_session_by_session_id(&*CUR_SESSION_ID.read().unwrap())
}

pub mod sessions {

    use super::*;

    lazy_static::lazy_static! {
        // peer -> peer session, peer session -> ui sessions
        static ref SESSIONS: RwLock<HashMap<(String, ConnType), FlutterSession>> = Default::default();
    }

    #[inline]
    pub fn get_session_count(peer_id: String, conn_type: ConnType) -> usize {
        SESSIONS
            .read()
            .unwrap()
            .get(&(peer_id, conn_type))
            .map(|s| s.ui_handler.session_handlers.read().unwrap().len())
            .unwrap_or(0)
    }

    #[inline]
    pub fn get_peer_id_by_session_id(id: &SessionID, conn_type: ConnType) -> Option<String> {
        SESSIONS
            .read()
            .unwrap()
            .iter()
            .find_map(|((peer_id, t), s)| {
                if *t == conn_type
                    && s.ui_handler
                        .session_handlers
                        .read()
                        .unwrap()
                        .contains_key(id)
                {
                    Some(peer_id.clone())
                } else {
                    None
                }
            })
    }

    #[inline]
    pub fn get_session_by_session_id(id: &SessionID) -> Option<FlutterSession> {
        SESSIONS
            .read()
            .unwrap()
            .values()
            .find(|s| {
                s.ui_handler
                    .session_handlers
                    .read()
                    .unwrap()
                    .contains_key(id)
            })
            .cloned()
    }

    #[inline]
    pub fn get_session_by_peer_id(peer_id: String, conn_type: ConnType) -> Option<FlutterSession> {
        SESSIONS.read().unwrap().get(&(peer_id, conn_type)).cloned()
    }

    #[inline]
    pub fn remove_session_by_session_id(id: &SessionID) -> Option<FlutterSession> {
        let mut remove_peer_key = None;
        for (peer_key, s) in SESSIONS.write().unwrap().iter_mut() {
            let mut write_lock = s.ui_handler.session_handlers.write().unwrap();
            let remove_ret = write_lock.remove(id);
            match remove_ret {
                Some(_) => {
                    if write_lock.is_empty() {
                        remove_peer_key = Some(peer_key.clone());
                    } else {
                        check_remove_unused_displays(None, id, s, &write_lock);
                    }
                    break;
                }
                None => {}
            }
        }
        let s = SESSIONS.write().unwrap().remove(&remove_peer_key?);
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        update_session_count_to_server();
        s
    }

    /// Close every client session, returning how many peer sessions were closed.
    ///
    /// Used when the UI is gone but the process keeps running, e.g. the Android
    /// task is swiped away from recents while a foreground service keeps the
    /// process alive. The orphaned `io_loop` would otherwise keep answering
    /// `TestDelay`, so the peer never hits its inactivity timeout and the
    /// session stays established with no way to close it.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    pub fn close_all_sessions() -> usize {
        // Release held keys before draining: the release path sends through
        // `get_cur_session()`, which resolves against SESSIONS, so draining
        // first would take TO_RELEASE and then silently drop every key-up,
        // leaving the key stuck on the controlled side. A no-op when nothing
        // is held.
        crate::keyboard::release_remote_keys("map");
        // Drain so the map lock is released before closing each session.
        let sessions: Vec<FlutterSession> = SESSIONS
            .write()
            .unwrap()
            .drain()
            .map(|(_, session)| session)
            .collect();
        for session in sessions.iter() {
            let session_ids: Vec<SessionID> = session
                .ui_handler
                .session_handlers
                .read()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            for session_id in session_ids {
                session.close_event_stream(session_id);
            }
            session.close();
        }
        sessions.len()
    }

    /// Check if removing a session by session_id would result in removing the entire peer.
    ///
    /// Returns:
    /// - `true`: The session exists and removing it would leave the peer with no other sessions,
    ///           so the entire peer would be removed (equivalent to `remove_session_by_session_id` returning `Some`)
    /// - `false`: The session doesn't exist, or it exists but the peer has other sessions,
    ///            so the peer would not be removed (equivalent to `remove_session_by_session_id` returning `None`)
    #[inline]
    pub fn would_remove_peer_by_session_id(id: &SessionID) -> bool {
        for (_peer_key, s) in SESSIONS.read().unwrap().iter() {
            let read_lock = s.ui_handler.session_handlers.read().unwrap();
            if read_lock.contains_key(id) {
                // Found the session, check if it's the only one for this peer
                return read_lock.len() == 1;
            }
        }
        // Session not found
        false
    }

    fn check_remove_unused_displays(
        current: Option<usize>,
        session_id: &SessionID,
        session: &FlutterSession,
        handlers: &HashMap<SessionID, SessionHandler>,
    ) {
        // Set capture displays if some are not used any more.
        let mut remains_displays = HashSet::new();
        if let Some(current) = current {
            remains_displays.insert(current);
        }
        for (k, h) in handlers.iter() {
            if k == session_id {
                continue;
            }
            remains_displays.extend(
                h.renderer
                    .map_display_sessions
                    .read()
                    .unwrap()
                    .keys()
                    .cloned(),
            );
        }
        if !remains_displays.is_empty() {
            session.capture_displays(
                vec![],
                vec![],
                remains_displays.iter().map(|d| *d as i32).collect(),
            );
        }
    }

    pub fn session_switch_display(is_desktop: bool, session_id: SessionID, value: Vec<i32>) {
        for s in SESSIONS.read().unwrap().values() {
            let mut write_lock = s.ui_handler.session_handlers.write().unwrap();
            if let Some(h) = write_lock.get_mut(&session_id) {
                h.displays = value.iter().map(|x| *x as usize).collect::<_>();
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                let displays_refresh = value.clone();
                if value.len() == 1 {
                    // Switch display.
                    // This operation will also cause the peer to send a switch display message.
                    // The switch display message will contain `SupportedResolutions`, which is useful when changing resolutions.
                    s.switch_display(value[0]);
                    // Reset the valid flag of the display.
                    s.next_rgba(value[0] as usize);

                    if !is_desktop {
                        s.capture_displays(vec![], vec![], value);
                    } else {
                        // Check if other displays are needed.
                        if value.len() == 1 {
                            check_remove_unused_displays(
                                Some(value[0] as _),
                                &session_id,
                                &s,
                                &write_lock,
                            );
                        }
                    }
                } else {
                    // Try capture all displays.
                    s.capture_displays(vec![], vec![], value);
                }
                // When switching display, we also need to send "Refresh display" message.
                // On the controlled side:
                // 1. If this display is not currently captured -> Refresh -> Message "Refresh display" is not required.
                // One more key frame (first frame) will be sent because the refresh message.
                // 2. If this display is currently captured -> Not refresh -> Message "Refresh display" is required.
                // Without the message, the control side cannot see the latest display image.
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    let is_support_multi_ui_session = crate::common::is_support_multi_ui_session(
                        &s.ui_handler.peer_info.read().unwrap().version,
                    );
                    if is_support_multi_ui_session {
                        for display in displays_refresh.iter() {
                            s.refresh_video(*display);
                        }
                    }
                }
                break;
            }
        }
    }

    #[inline]
    pub fn insert_session(session_id: SessionID, conn_type: ConnType, session: FlutterSession) {
        SESSIONS
            .write()
            .unwrap()
            .entry((session.get_id(), conn_type))
            .or_insert(session)
            .ui_handler
            .session_handlers
            .write()
            .unwrap()
            .insert(session_id, Default::default());
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        update_session_count_to_server();
    }

    #[inline]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn update_session_count_to_server() {
        crate::ipc::update_controlling_session_count(SESSIONS.read().unwrap().len()).ok();
    }

    #[inline]
    pub fn insert_peer_session_id(
        peer_id: String,
        conn_type: ConnType,
        session_id: SessionID,
        displays: Vec<i32>,
    ) -> bool {
        if let Some(s) = SESSIONS.read().unwrap().get(&(peer_id, conn_type)) {
            let mut h = SessionHandler::default();
            h.displays = displays.iter().map(|x| *x as usize).collect::<_>();
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            let is_support_multi_ui_session = crate::common::is_support_multi_ui_session(
                &s.ui_handler.peer_info.read().unwrap().version,
            );
            #[cfg(any(target_os = "android", target_os = "ios"))]
            let is_support_multi_ui_session = false;
            h.renderer.is_support_multi_ui_session = is_support_multi_ui_session;
            let _ = s
                .ui_handler
                .session_handlers
                .write()
                .unwrap()
                .insert(session_id, h);
            // If the session is a single display session, it may be a software rgba rendered display.
            // If this is the second time the display is opened, the old valid flag may be true.
            if displays.len() == 1 {
                s.ui_handler.next_rgba(displays[0] as usize);
            }
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn get_sessions() -> Vec<FlutterSession> {
        SESSIONS.read().unwrap().values().cloned().collect()
    }

    #[inline]
    #[cfg(not(target_os = "ios"))]
    pub fn has_sessions_running(conn_type: ConnType) -> bool {
        SESSIONS.read().unwrap().iter().any(|((_, r#type), s)| {
            *r#type == conn_type && s.session_handlers.read().unwrap().len() != 0
        })
    }

    #[inline]
    #[cfg(not(target_os = "ios"))]
    pub fn has_connected_sessions_running(conn_type: ConnType) -> bool {
        SESSIONS.read().unwrap().iter().any(|((_, r#type), s)| {
            *r#type == conn_type
                && s.session_handlers.read().unwrap().len() != 0
                && s.connection_round_state.lock().unwrap().is_connected()
        })
    }
}

pub(super) mod async_tasks {
    use hbb_common::{bail, tokio, ResultType};
    use std::{
        collections::HashMap,
        sync::{
            mpsc::{sync_channel, SyncSender},
            Arc, Mutex,
        },
    };

    type TxQueryOnlines = SyncSender<Vec<String>>;
    lazy_static::lazy_static! {
        static ref TX_QUERY_ONLINES: Arc<Mutex<Option<TxQueryOnlines>>> = Default::default();
    }

    #[inline]
    pub fn start_flutter_async_runner() {
        std::thread::spawn(start_flutter_async_runner_);
    }

    #[allow(dead_code)]
    pub fn stop_flutter_async_runner() {
        let _ = TX_QUERY_ONLINES.lock().unwrap().take();
    }

    #[tokio::main(flavor = "current_thread")]
    async fn start_flutter_async_runner_() {
        // Only one task is allowed to run at the same time.
        let (tx_onlines, rx_onlines) = sync_channel::<Vec<String>>(1);
        TX_QUERY_ONLINES.lock().unwrap().replace(tx_onlines);

        loop {
            match rx_onlines.recv() {
                Ok(ids) => {
                    crate::client::peer_online::query_online_states(ids, handle_query_onlines).await
                }
                _ => {
                    // unreachable!
                    break;
                }
            }
        }
    }

    pub fn query_onlines(ids: Vec<String>) -> ResultType<()> {
        if let Some(tx) = TX_QUERY_ONLINES.lock().unwrap().as_ref() {
            // Ignore if the channel is full.
            let _ = tx.try_send(ids)?;
        } else {
            bail!("No tx_query_onlines");
        }
        Ok(())
    }

    fn handle_query_onlines(onlines: Vec<String>, offlines: Vec<String>) {
        let data = HashMap::from([
            ("name", "callback_query_onlines".to_owned()),
            ("onlines", onlines.join(",")),
            ("offlines", offlines.join(",")),
        ]);
        let _res = super::push_global_event(
            super::APP_TYPE_MAIN,
            serde_json::ser::to_string(&data).unwrap_or("".to_owned()),
        );
    }
}
