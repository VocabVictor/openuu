use super::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum Data {
    Login {
        id: i32,
        is_file_transfer: bool,
        is_view_camera: bool,
        is_terminal: bool,
        peer_id: String,
        name: String,
        avatar: String,
        authorized: bool,
        port_forward: String,
        keyboard: bool,
        clipboard: bool,
        audio: bool,
        file: bool,
        file_transfer_enabled: bool,
        restart: bool,
        recording: bool,
        block_input: bool,
        privacy_mode: bool,
        from_switch: bool,
    },
    ChatMessage {
        text: String,
    },
    SwitchPermission {
        name: String,
        enabled: bool,
    },
    SystemInfo(Option<String>),
    ClickTime(i64),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    MouseMoveTime(i64),
    Authorize,
    Close,
    #[cfg(windows)]
    SAS,
    UserSid(Option<u32>),
    OnlineStatus(Option<(i64, bool)>),
    Config((String, Option<String>)),
    Options(Option<HashMap<String, String>>),
    NatType(Option<i32>),
    ConfirmedKey(Option<(Vec<u8>, Vec<u8>)>),
    RawMessage(Vec<u8>),
    Socks(Option<config::Socks5Server>),
    FS(FS),
    Test,
    SyncConfig(Option<Box<(Config, Config2)>>),
    #[cfg(target_os = "windows")]
    ClipboardFile(ClipboardFile),
    ClipboardFileEnabled(bool),
    #[cfg(target_os = "windows")]
    ClipboardNonFile(Option<(String, Vec<ClipboardNonFile>)>),
    PrivacyModeState((i32, PrivacyModeState, String)),
    TestRendezvousServer,
    Deployed,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Keyboard(DataKeyboard),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    KeyboardResponse(DataKeyboardResponse),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Mouse(DataMouse),
    Control(DataControl),
    Theme(String),
    Language(String),
    Empty,
    Disconnected,
    DataPortableService(DataPortableService),
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    SwitchSidesRequest(String),
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    SwitchSidesUuid(String, String, SwitchSidesUuidAction, Option<bool>),
    #[cfg(feature = "flutter")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    SwitchSidesBack,
    UrlLink(String),
    VoiceCallIncoming,
    StartVoiceCall,
    VoiceCallResponse(bool),
    CloseVoiceCall(String),
    #[cfg(windows)]
    SyncWinCpuUsage(Option<f64>),
    FileTransferLog((String, String)),
    #[cfg(windows)]
    ControlledSessionCount(usize),
    CmErr(String),
    // CM-side file reading responses (Windows only)
    // These are sent from CM back to Connection when CM handles file reading
    /// Response to ReadFile: contains initial file list or error
    ReadJobInitResult {
        id: i32,
        file_num: i32,
        include_hidden: bool,
        conn_id: i32,
        /// Serialized protobuf bytes of FileDirectory, or error string
        result: Result<Vec<u8>, String>,
    },
    /// File data block read by CM.
    ///
    /// The actual data is sent separately via `send_raw()` after this message to avoid
    /// JSON encoding overhead for large binary data. This mirrors the `WriteBlock` pattern.
    ///
    /// **Protocol:**
    /// - Sender: `send(FileBlockFromCM{...})` then `send_raw(data)`
    /// - Receiver: `next()` returns `FileBlockFromCM`, then `next_raw()` returns data bytes
    ///
    /// **Note on empty data (e.g., empty files):**
    /// Empty data is supported. The IPC connection uses `BytesCodec` with `raw=false` (default),
    /// which prefixes each frame with a length header. So `send_raw(Bytes::new())` sends a
    /// 1-byte frame (length=0), and `next_raw()` correctly returns an empty `BytesMut`.
    /// See `libs/hbb_common/src/bytes_codec.rs` test `test_codec2` for verification.
    FileBlockFromCM {
        id: i32,
        file_num: i32,
        /// Data is sent separately via `send_raw()` to avoid JSON encoding overhead.
        /// This field is skipped during serialization; sender must call `send_raw()` after sending.
        /// Receiver must call `next_raw()` and populate this field manually.
        #[serde(skip)]
        data: bytes::Bytes,
        compressed: bool,
        conn_id: i32,
    },
    /// File read completed successfully
    FileReadDone {
        id: i32,
        file_num: i32,
        conn_id: i32,
    },
    /// File read failed with error
    FileReadError {
        id: i32,
        file_num: i32,
        err: String,
        conn_id: i32,
    },
    /// Digest info from CM for overwrite detection
    FileDigestFromCM {
        id: i32,
        file_num: i32,
        last_modified: u64,
        file_size: u64,
        is_resume: bool,
        conn_id: i32,
    },
    /// Response to ReadAllFiles: recursive directory listing
    AllFilesResult {
        id: i32,
        conn_id: i32,
        path: String,
        /// Serialized protobuf bytes of FileDirectory, or error string
        result: Result<Vec<u8>, String>,
    },
    CheckHwcodec,
    #[cfg(feature = "flutter")]
    VideoConnCount(Option<usize>),
    // Although the key is not necessary, it is used to avoid hardcoding the key.
    WaylandScreencastRestoreToken((String, String)),
    HwCodecConfig(Option<String>),
    RemoveTrustedDevices(Vec<Bytes>),
    ClearTrustedDevices,
    #[cfg(all(
        feature = "flutter",
        not(any(target_os = "android", target_os = "ios"))
    ))]
    ControllingSessionCount(usize),
    #[cfg(target_os = "linux")]
    TerminalSessionCount(usize),
    #[cfg(target_os = "windows")]
    PortForwardSessionCount(Option<usize>),
    SocksWs(Option<Box<(Option<config::Socks5Server>, String)>>),
    #[cfg(target_os = "macos")]
    HasNoActiveConns(Option<bool>),
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    Whiteboard((String, crate::whiteboard::CustomEvent)),
    ControlPermissionsRemoteModify(Option<bool>),
    #[cfg(target_os = "windows")]
    FileTransferEnabledState(Option<bool>),
    /// CM -> server: the connection manager's WINDOW went away, which is not the same event
    /// as the operator disconnecting a peer. Linux only, and deliberately: there a session
    /// logout closes every window, and the close arrives at the CM indistinguishable from a
    /// person clicking it - measured on KDE, the CM gets no signal and logind still reports the
    /// session active. So the ambiguous case ends the session WITHOUT the no-retry reason and
    /// the peer is allowed to reconnect (landing on the greeter after a logout), while the
    /// explicit Disconnect button keeps sending `Close` and kicking for good.
    #[cfg(target_os = "linux")]
    CmWindowClosed,
    // --- DRM/KMS capture (opt-in `drm` feature) over the `_drm` service-scoped channel ---
    // All of the following are `cfg(all(linux, drm))`, so the drm-off IPC wire is byte-identical
    // to upstream. Protocol on `_drm`: on connect the root service sends `DrmDisplayList`, the
    // client replies `DrmStart{display}`, then the service streams `DrmFrame` + send_raw(BGRA) and
    // `DrmCursor` + send_raw(RGBA). A frame/cursor header is ALWAYS immediately followed by exactly
    // one `send_raw()` payload (the same header-then-raw pairing as `FileBlockFromCM`). This keeps
    // the header extensible. The zero-copy `DrmFrameDmabuf(DmabufDesc)` sibling below carries only a
    // small JSON metadata descriptor; the scanout dma-buf fd rides an SCM_RIGHTS ancillary message on
    // the same `DrmConn` send (see `DrmConn::send_msg`), so it has NO trailing `send_raw()` body.
    /// Client -> service: begin streaming the chosen display.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    // `need_cpu` is set by an unprivileged consumer that could not open a render-node convert context
    // (drmtap_open_render failed, e.g. no /dev/dri/renderD* access). The service then streams the
    // CPU-converted `DrmFrame` path for this connection instead of a dma-buf fd the consumer cannot
    // detile, so a render-node-less seat still captures instead of losing the stream.
    DrmStart { display: i32, need_cpu: bool },
    /// Service -> client: the enumerated DRM displays (sent once, before frames).
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmDisplayList(Vec<DrmDisplayInfo>),
    /// Service -> client: the connector topology changed mid-stream (a monitor hotplug/unplug/modeset,
    /// observed by the service's udev DRM-uevent listener). Carries the freshly-enumerated list so the
    /// consumer can swap its sticky positive availability cache off the hot path, WITHOUT re-probing
    /// `_drm` (which would trip the enumeration restart loop). Interleaved with frames on the same
    /// stream; carries no `send_raw()` body and no fd.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmDisplaysChanged(Vec<DrmDisplayInfo>),
    /// Service -> client: a frame header; the packed BGRA pixels follow via `send_raw()`.
    /// CPU-fallback path (no render node, or no transferable dma-buf): pixels cross the wire.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmFrame { width: u32, height: u32 },
    /// Service -> client: a zero-copy dma-buf frame descriptor. The scanout fd is NOT a field; when
    /// `desc.has_fd` it rides an SCM_RIGHTS ancillary message on the same `DrmConn::send_msg`, and
    /// there is NO trailing `send_raw()` body. The unprivileged `--server` imports the fd and does
    /// the EGL detile/convert itself (see `DmabufDesc`).
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmFrameDmabuf(DmabufDesc),
    /// Service -> client: a hardware-cursor header; the RGBA pixels follow via `send_raw()`.
    #[cfg(all(target_os = "linux", feature = "drm"))]
    DrmCursor {
        id: u64,
        width: u32,
        height: u32,
        hotx: i32,
        hoty: i32,
    },
}
