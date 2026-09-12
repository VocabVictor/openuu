pub const OPTION_VIEW_ONLY: &str = "view_only";
pub const OPTION_SHOW_MONITORS_TOOLBAR: &str = "show_monitors_toolbar";
pub const OPTION_SHOW_REMOTE_CURSOR: &str = "show_remote_cursor";
pub const OPTION_FOLLOW_REMOTE_CURSOR: &str = "follow_remote_cursor";
pub const OPTION_FOLLOW_REMOTE_WINDOW: &str = "follow_remote_window";
pub const OPTION_SHOW_QUALITY_MONITOR: &str = "show_quality_monitor";
pub const OPTION_DISABLE_AUDIO: &str = "disable_audio";
pub const OPTION_DISABLE_CLIPBOARD: &str = "disable_clipboard";
pub const OPTION_LOCK_AFTER_SESSION_END: &str = "lock_after_session_end";
pub const OPTION_PRIVACY_MODE: &str = "privacy_mode";
pub const OPTION_TOUCH_MODE: &str = "touch-mode";
pub const OPTION_SYNC_INIT_CLIPBOARD: &str = "sync-init-clipboard";
pub const OPTION_THEME: &str = "theme";
pub const OPTION_REMOTE_MENUBAR_DRAG_LEFT: &str = "remote-menubar-drag-left";
pub const OPTION_REMOTE_MENUBAR_DRAG_RIGHT: &str = "remote-menubar-drag-right";
pub const OPTION_HIDE_AB_TAGS_PANEL: &str = "hideAbTagsPanel";
pub const OPTION_ENABLE_CONFIRM_CLOSING_TABS: &str = "enable-confirm-closing-tabs";
pub const OPTION_ENABLE_OPEN_NEW_CONNECTIONS_IN_TABS: &str = "enable-open-new-connections-in-tabs";
pub const OPTION_TEXTURE_RENDER: &str = "use-texture-render";
// Internal health record written by the texture-render watchdog/probe;
// "failed-*" flips the texture-render default to opt-in on this machine.
pub const OPTION_TEXTURE_RENDER_HEALTH: &str = "texture-render-health";
pub const OPTION_ALLOW_D3D_RENDER: &str = "allow-d3d-render";
pub const OPTION_ENABLE_CHECK_UPDATE: &str = "enable-check-update";
pub const OPTION_ALLOW_AUTO_UPDATE: &str = "allow-auto-update";
pub const OPTION_SYNC_AB_WITH_RECENT_SESSIONS: &str = "sync-ab-with-recent-sessions";
pub const OPTION_SYNC_AB_TAGS: &str = "sync-ab-tags";
pub const OPTION_FILTER_AB_BY_INTERSECTION: &str = "filter-ab-by-intersection";
pub const OPTION_ACCESS_MODE: &str = "access-mode";
pub const OPTION_ENABLE_KEYBOARD: &str = "enable-keyboard";
pub const OPTION_ENABLE_CLIPBOARD: &str = "enable-clipboard";
pub const OPTION_ENABLE_FILE_TRANSFER: &str = "enable-file-transfer";
pub const OPTION_ENABLE_CAMERA: &str = "enable-camera";
pub const OPTION_ENABLE_TERMINAL: &str = "enable-terminal";
pub const OPTION_TERMINAL_PERSISTENT: &str = "terminal-persistent";
pub const OPTION_ENABLE_AUDIO: &str = "enable-audio";
pub const OPTION_ENABLE_TUNNEL: &str = "enable-tunnel";
pub const OPTION_ENABLE_REMOTE_RESTART: &str = "enable-remote-restart";
pub const OPTION_ENABLE_RECORD_SESSION: &str = "enable-record-session";
pub const OPTION_ENABLE_BLOCK_INPUT: &str = "enable-block-input";
pub const OPTION_ENABLE_PRIVACY_MODE: &str = "enable-privacy-mode";
pub const OPTION_ENABLE_PERM_CHANGE_IN_ACCEPT_WINDOW: &str = "enable-perm-change-in-accept-window";
pub const OPTION_ALLOW_SCOPE_VIOLATION_CLOSE: &str = "allow-scope-violation-close";
pub const OPTION_ALLOW_SCOPE_VIOLATION_ALARM: &str = "allow-scope-violation-alarm";
pub const OPTION_ALLOW_REMOTE_CONFIG_MODIFICATION: &str = "allow-remote-config-modification";
pub const OPTION_ENABLE_LAN_DISCOVERY: &str = "enable-lan-discovery";
pub const OPTION_DIRECT_ACCESS_PORT: &str = "direct-access-port";
pub const OPTION_WHITELIST: &str = "whitelist";
pub const OPTION_ID_WHITELIST: &str = "id-whitelist";
pub const OPTION_ALLOW_AUTO_DISCONNECT: &str = "allow-auto-disconnect";
pub const OPTION_AUTO_DISCONNECT_TIMEOUT: &str = "auto-disconnect-timeout";
pub const OPTION_ALLOW_ONLY_CONN_WINDOW_OPEN: &str = "allow-only-conn-window-open";
pub const OPTION_ALLOW_AUTO_RECORD_INCOMING: &str = "allow-auto-record-incoming";
pub const OPTION_ALLOW_AUTO_RECORD_OUTGOING: &str = "allow-auto-record-outgoing";
pub const OPTION_HIDE_RECORDING_BUTTON: &str = "hide-recording-button";
pub const OPTION_WINDOWS_SERVICE_VIDEO_SAVE_DIRECTORY: &str =
    "windows-service-video-save-directory";
pub const OPTION_VIDEO_SAVE_DIRECTORY: &str = "video-save-directory";
pub const OPTION_ENABLE_ABR: &str = "enable-abr";
pub const OPTION_ALLOW_REMOVE_WALLPAPER: &str = "allow-remove-wallpaper";
pub const OPTION_ALLOW_ALWAYS_SOFTWARE_RENDER: &str = "allow-always-software-render";
pub const OPTION_ENABLE_HWCODEC: &str = "enable-hwcodec";
pub const OPTION_APPROVE_MODE: &str = "approve-mode";
pub const OPTION_VERIFICATION_METHOD: &str = "verification-method";
pub const OPTION_TEMPORARY_PASSWORD_LENGTH: &str = "temporary-password-length";
pub const OPTION_CUSTOM_RENDEZVOUS_SERVER: &str = "custom-rendezvous-server";
pub const OPTION_API_SERVER: &str = "api-server";
pub const OPTION_KEY: &str = "key";
pub const OPTION_PRESET_ADDRESS_BOOK_NAME: &str = "preset-address-book-name";
pub const OPTION_PRESET_ADDRESS_BOOK_TAG: &str = "preset-address-book-tag";
pub const OPTION_PRESET_ADDRESS_BOOK_ALIAS: &str = "preset-address-book-alias";
pub const OPTION_PRESET_ADDRESS_BOOK_PASSWORD: &str = "preset-address-book-password";
pub const OPTION_PRESET_ADDRESS_BOOK_NOTE: &str = "preset-address-book-note";
pub const OPTION_PRESET_DEVICE_USERNAME: &str = "preset-device-username";
pub const OPTION_PRESET_DEVICE_NAME: &str = "preset-device-name";
pub const OPTION_PRESET_NOTE: &str = "preset-note";
pub const OPTION_ENABLE_DIRECTX_CAPTURE: &str = "enable-directx-capture";
pub const OPTION_ENABLE_ANDROID_SOFTWARE_ENCODING_HALF_SCALE: &str =
    "enable-android-software-encoding-half-scale";
pub const OPTION_ENABLE_TRUSTED_DEVICES: &str = "enable-trusted-devices";
pub const OPTION_AV1_TEST: &str = "av1-test";
/// Maximum number of files allowed during a single file transfer request.
///
/// Key: `file-transfer-max-files`.
/// Unit: number of files (not bytes).
///
/// Behaviour:
/// - If set to a positive integer N, at most N files are allowed.
/// - If set to 0, a safe built-in default is used (see DEFAULT_MAX_VALIDATED_FILES).
/// - If unset, negative, or non-integer, no explicit limit is enforced for backward compatibility.
pub const OPTION_FILE_TRANSFER_MAX_FILES: &str = "file-transfer-max-files";
pub const OPTION_DISABLE_UDP: &str = "disable-udp";
pub const OPTION_SHOW_VIRTUAL_MOUSE: &str = "show-virtual-mouse";
// joystick is the virtual mouse.
// So `OPTION_SHOW_VIRTUAL_MOUSE` should also be set if `OPTION_SHOW_VIRTUAL_JOYSTICK` is set.
pub const OPTION_SHOW_VIRTUAL_JOYSTICK: &str = "show-virtual-joystick";
pub const OPTION_ENABLE_FLUTTER_HTTP_ON_RUST: &str = "enable-flutter-http-on-rust";
pub const OPTION_ALLOW_ASK_FOR_NOTE: &str = "allow-ask-for-note";
