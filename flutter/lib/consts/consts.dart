import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';
part 'key_maps.dart';
part 'platform.dart';

const int kMaxVirtualDisplayCount = 4;
const int kAllVirtualDisplay = -1;

const double kDesktopRemoteTabBarHeight = 28.0;
const int kInvalidWindowId = -1;
const int kMainWindowId = 0;

const kAllDisplayValue = -1;

const kKeyLegacyMode = 'legacy';
const kKeyMapMode = 'map';
const kKeyTranslateMode = 'translate';

const String kPlatformAdditionsIsWayland = "is_wayland";
const String kPlatformAdditionsIsInstalled = "is_installed";
const String kPlatformAdditionsIddImpl = "idd_impl";
const String kPlatformAdditionsRustDeskVirtualDisplays =
    "rustdesk_virtual_displays";
const String kPlatformAdditionsAmyuniVirtualDisplays =
    "amyuni_virtual_displays";
const String kPlatformAdditionsHasFileClipboard = "has_file_clipboard";
const String kPlatformAdditionsSupportedPrivacyModeImpl =
    "supported_privacy_mode_impl";

const String kPrivacyModeImplMag = 'privacy_mode_impl_mag';
const String kPrivacyModeImplExcludeFromCapture =
    'privacy_mode_impl_exclude_from_capture';

const String kPeerPlatformWindows = "Windows";
const String kPeerPlatformLinux = "Linux";
const String kPeerPlatformMacOS = "Mac OS";
const String kPeerPlatformAndroid = "Android";
const String kPeerPlatformWebDesktop = "WebDesktop";

const double kScrollbarThickness = 12.0;

/// [kAppTypeMain] used by 'Desktop Main Page' , 'Mobile (Client and Server)', "Install Page"
const String kAppTypeMain = "main";

/// [kAppTypeConnectionManager] only for 'Desktop CM Page'
const String kAppTypeConnectionManager = "cm";

const String kAppTypeDesktopRemote = "remote";
const String kAppTypeDesktopFileTransfer = "file transfer";
const String kAppTypeDesktopViewCamera = "view camera";
const String kAppTypeDesktopPortForward = "port forward";
const String kAppTypeDesktopTerminal = "terminal";

const String kWindowMainWindowOnTop = "main_window_on_top";
const String kWindowRefreshCurrentUser = "refresh_current_user";
const String kWindowGetScreenList = "get_screen_list";
// This method is not used, maybe it can be removed.
const String kWindowDisableGrabKeyboard = "disable_grab_keyboard";
const String kWindowActionRebuild = "rebuild";
const String kWindowEventHide = "hide";
const String kWindowEventShow = "show";
const String kWindowConnect = "connect";
const String kWindowBumpMouse = "bump_mouse";

const String kWindowEventNewRemoteDesktop = "new_remote_desktop";
const String kWindowEventNewFileTransfer = "new_file_transfer";
const String kWindowEventNewViewCamera = "new_view_camera";
const String kWindowEventNewPortForward = "new_port_forward";
const String kWindowEventNewTerminal = "new_terminal";
const String kWindowEventRestoreTerminalSessions = "restore_terminal_sessions";
const String kWindowEventActiveSession = "active_session";
const String kWindowEventActiveDisplaySession = "active_display_session";
const String kWindowEventGetRemoteList = "get_remote_list";
const String kWindowEventGetSessionIdList = "get_session_id_list";
const String kWindowEventRemoteWindowCoords = "remote_window_coords";
const String kWindowEventSetFullscreen = "set_fullscreen";

const String kWindowEventMoveTabToNewWindow = "move_tab_to_new_window";
const String kWindowEventGetCachedSessionData = "get_cached_session_data";
const String kWindowEventOpenMonitorSession = "open_monitor_session";

const String kOptionViewStyle = "view_style";
const String kOptionScrollStyle = "scroll_style";
const String kOptionEdgeScrollEdgeThickness = "edge-scroll-edge-thickness";
const String kOptionImageQuality = "image_quality";
const String kOptionOpenNewConnInTabs = "enable-open-new-connections-in-tabs";
const String kOptionTextureRender = "use-texture-render";
const String kOptionD3DRender = "allow-d3d-render";
const String kOptionOpenInTabs = "allow-open-in-tabs";
const String kOptionOpenInWindows = "allow-open-in-windows";
const String kOptionForceAlwaysRelay = "force-always-relay";
const String kOptionViewOnly = "view_only";
const String kOptionEnableLanDiscovery = "enable-lan-discovery";
const String kOptionWhitelist = "whitelist";
const String kOptionIdWhitelist = "id-whitelist";
const String kOptionEnableAbr = "enable-abr";
const String kOptionEnableRecordSession = "enable-record-session";
const String kOptionDirectServer = "direct-server";
const String kOptionDirectAccessPort = "direct-access-port";
const String kOptionAllowAutoDisconnect = "allow-auto-disconnect";
const String kOptionAutoDisconnectTimeout = "auto-disconnect-timeout";
const String kOptionEnableHwcodec = "enable-hwcodec";
const String kOptionAllowAutoRecordIncoming = "allow-auto-record-incoming";
const String kOptionAllowAutoRecordOutgoing = "allow-auto-record-outgoing";
const String kOptionHideRecordingButton = "hide-recording-button";
const String kOptionVideoSaveDirectory = "video-save-directory";
const String kOptionAccessMode = "access-mode";
const String kOptionEnableKeyboard = "enable-keyboard";
// "Settings -> Security -> Permissions"
const String kOptionEnableClipboard = "enable-clipboard";
const String kOptionEnableFileTransfer = "enable-file-transfer";
const String kOptionEnableAudio = "enable-audio";
const String kOptionEnableCamera = "enable-camera";
const String kOptionEnableTerminal = "enable-terminal";
const String kOptionTerminalPersistent = "terminal-persistent";
const String kOptionAllowTerminalClipboardWrite =
    "allow-terminal-clipboard-write";
const String kTerminalClipboardWriteUnconfigured = "";
const String kTerminalClipboardWriteAllowed = "Y";
const String kTerminalClipboardWriteDenied = "N";
const String kOptionEnableTunnel = "enable-tunnel";
const String kOptionEnableRemoteRestart = "enable-remote-restart";
const String kOptionEnableBlockInput = "enable-block-input";
const String kOptionEnablePrivacyMode = "enable-privacy-mode";
const String kOptionEnablePermChangeInAcceptWindow =
    "enable-perm-change-in-accept-window";
const String kOptionAllowRemoteConfigModification =
    "allow-remote-config-modification";
const String kOptionVerificationMethod = "verification-method";
const String kOptionApproveMode = "approve-mode";
const String kOptionAllowNumericOneTimePassword =
    "allow-numeric-one-time-password";
const String kOptionCollapseToolbar = "collapse_toolbar";
const String kOptionHideToolbar = "hide-toolbar";
const String kOptionShowRemoteCursor = "show_remote_cursor";
const String kOptionFollowRemoteCursor = "follow_remote_cursor";
const String kOptionFollowRemoteWindow = "follow_remote_window";
const String kOptionZoomCursor = "zoom-cursor";
const String kOptionShowQualityMonitor = "show_quality_monitor";
const String kOptionDisableAudio = "disable_audio";
const String kOptionEnableFileCopyPaste = "enable-file-copy-paste";
// "Settings -> Display -> Other default options"
const String kOptionDisableClipboard = "disable_clipboard";
const String kOptionLockAfterSessionEnd = "lock_after_session_end";
const String kOptionPrivacyMode = "privacy_mode";
const String kOptionTouchMode = "touch-mode";
const String kOptionI444 = "i444";
const String kOptionSwapLeftRightMouse = "swap-left-right-mouse";
const String kOptionCodecPreference = "codec-preference";
const String kOptionRemoteMenubarDragLeft = "remote-menubar-drag-left";
const String kOptionRemoteMenubarDragRight = "remote-menubar-drag-right";
const String kOptionRemoteMenubarEdge = "remote-menubar-edge";
const String kOptionRemoteMenubarFraction = "remote-menubar-frac";
const String kOptionAllowMultiEdgeToolbarDock =
    "allow-multi-edge-toolbar-dock";
const String kOptionHideAbTagsPanel = "hideAbTagsPanel";
const String kOptionRemoteMenubarState = "remoteMenubarState";
const String kOptionPeerSorting = "peer-sorting";
const String kOptionPeerTabIndex = "peer-tab-index";
const String kOptionPeerTabOrder = "peer-tab-order";
const String kOptionPeerTabVisible = "peer-tab-visible";
const String kOptionPeerCardUiType = "peer-card-ui-type";
const String kOptionCurrentAbName = "current-ab-name";
const String kOptionEnableConfirmClosingTabs = "enable-confirm-closing-tabs";
const String kOptionEnablePortForwardMux = "enable-port-forward-mux";
const String kOptionAllowAlwaysSoftwareRender = "allow-always-software-render";
const String kOptionEnableCheckUpdate = "enable-check-update";
const String kOptionAllowAutoUpdate = "allow-auto-update";
const String kOptionAllowRemoveWallpaper = "allow-remove-wallpaper";
const String kOptionStopService = "stop-service";
const String kOptionDirectxCapture = "enable-directx-capture";
const String kOptionAllowRemoteCmModification = "allow-remote-cm-modification";
const String kOptionEnableTcpPunch = "enable-tcp-punch";
const String kOptionEnableUdpPunch = "enable-udp-punch";
const String kOptionEnableIpv6Punch = "enable-ipv6-punch";
const String kOptionAllowSyncClipboardBetweenSessions =
    "allow-sync-clipboard-between-sessions";
const String kOptionEnableWebrtc = "enable-webrtc";
const String kOptionEnableTrustedDevices = "enable-trusted-devices";
const String kOptionShowVirtualMouse = "show-virtual-mouse";
const String kOptionVirtualMouseScale = "virtual-mouse-scale";
const String kOptionShowVirtualJoystick = "show-virtual-joystick";
const String kOptionAllowAskForNoteAtEndOfConnection = "allow-ask-for-note";
const String kOptionAllowMonitorSwitchMainToolbar = "allow-monitor-switch-main-toolbar";
const String kOptionAllowMonitorSwitchMinToolbar = "allow-monitor-switch-min-toolbar";
const String kOptionEnableShowTerminalExtraKeys = "enable-show-terminal-extra-keys";
const String kOptionShowTerminalCtrlKeys = "show-terminal-extra-ctrl-keys";

// network options
const String kOptionAllowWebSocket = "allow-websocket";
const String kOptionAllowInsecureTLSFallback = "allow-insecure-tls-fallback";
const String kOptionDisableUdp = "disable-udp";
const String kOptionEnableFlutterHttpOnRust = "enable-flutter-http-on-rust";

// builtin options
const String kOptionHideServerSetting = "hide-server-settings";
const String kOptionHideProxySetting = "hide-proxy-settings";
const String kOptionHideWebSocketSetting = "hide-websocket-settings";
const String kOptionHideStopService = "hide-stop-service";
const String kOptionHideGeneralSetting = "hide-general-settings";
const String kOptionHideSecuritySetting = "hide-security-settings";
const String kOptionHideNetworkSetting = "hide-network-settings";
const String kOptionRemovePresetPasswordWarning =
    "remove-preset-password-warning";
const String kOptionDisableChangePermanentPassword =
    "disable-change-permanent-password";
const String kOptionDisableChangeId = "disable-change-id";
const String kOptionDisableUnlockPin = "disable-unlock-pin";
const kHideUsernameOnCard = "hide-username-on-card";
const String kOptionHideHelpCards = "hide-help-cards";
const String kOptionAllowDeepLinkPassword = "allow-deep-link-password";
const String kOptionAllowDeepLinkServerSettings =
    "allow-deep-link-server-settings";

const String kOptionToggleViewOnly = "view-only";
const String kOptionToggleShowMyCursor = "show-my-cursor";

const String kOptionDisableFloatingWindow = "disable-floating-window";

const String kOptionKeepScreenOn = "keep-screen-on";

const String kOptionKeepAwakeDuringIncomingSessions = "keep-awake-during-incoming-sessions";
const String kOptionKeepAwakeDuringOutgoingSessions = "keep-awake-during-outgoing-sessions";

const String kOptionShowMobileAction = "showMobileActions";

const String kUrlActionClose = "close";

const String kTabLabelHomePage = "Home";
const String kTabLabelSettingPage = "Settings";

const String kWindowPrefix = "wm_";
const int kWindowMainId = 0;

const String kPointerEventKindTouch = "touch";
const String kPointerEventKindMouse = "mouse";

const String kMouseEventTypeDefault = "";
const String kMouseEventTypePanStart = "pan_start";
const String kMouseEventTypePanUpdate = "pan_update";
const String kMouseEventTypePanEnd = "pan_end";
const String kMouseEventTypeDown = "down";
const String kMouseEventTypeUp = "up";

const String kKeyFlutterKey = "flutter_key";

const String kKeyShowDisplaysAsIndividualWindows =
    'displays_as_individual_windows';
const String kKeyUseAllMyDisplaysForTheRemoteSession =
    'use_all_my_displays_for_the_remote_session';
const String kKeyShowMonitorsToolbar = 'show_monitors_toolbar';
const String kKeyReverseMouseWheel = "reverse_mouse_wheel";

const String kMsgboxTextWaitingForImage = 'Connected, waiting for image...';

// the executable name of the portable version
const String kEnvPortableExecutable = "RUSTDESK_APPNAME";

const Color kColorWarn = Color.fromARGB(255, 245, 133, 59);
const Color kColorCanvas = Colors.black;

const int kMobileDefaultDisplayWidth = 720;
const int kMobileDefaultDisplayHeight = 1280;

const int kDesktopDefaultDisplayWidth = 1080;
const int kDesktopDefaultDisplayHeight = 720;

const int kMobileMaxDisplaySize = 1280;
const int kDesktopMaxDisplaySize = 3840;

const double kDesktopFileTransferRowHeight = 30.0;
const double kDesktopFileTransferHeaderHeight = 25.0;

const double kMinFps = 5;
const double kDefaultFps = 30;
const double kMaxFps = 120;

const double kMinQuality = 10;
const double kDefaultQuality = 50;
const double kMaxQuality = 100;
const double kMaxMoreQuality = 2000;

// trackpad speed
const String kKeyTrackpadSpeed = 'trackpad-speed';
const int kMinTrackpadSpeed = 10;
const int kDefaultTrackpadSpeed = 100;
const int kMaxTrackpadSpeed = 1000;

// relative mouse mode
/// Throttle duration (in milliseconds) for updating pointer lock center during
/// window move/resize events. Lower values provide more responsive updates but
/// may cause performance issues during rapid window operations.
const int kDefaultPointerLockCenterThrottleMs = 100;

/// Minimum server version required for relative mouse mode (MOUSE_TYPE_MOVE_RELATIVE).
/// Servers older than this version will ignore relative mouse events.
///
/// IMPORTANT: This value must be kept in sync with the Rust constant
/// `MIN_VERSION_RELATIVE_MOUSE_MODE` in `src/common.rs`.
const String kMinVersionForRelativeMouseMode = '1.4.5';

/// Maximum delta value for relative mouse movement.
/// Large values could cause issues with i32 overflow on server side,
/// and no reasonable mouse movement should exceed this bound.
///
/// IMPORTANT: This value must be kept in sync with the Rust constant
/// `MAX_RELATIVE_MOUSE_DELTA` in `src/server/input_service.rs`.
const int kMaxRelativeMouseDelta = 10000;

/// Debounce duration (in milliseconds) for relative mouse mode toggle.
/// This prevents double-toggle from race condition between Rust rdev grab loop
/// and Flutter keyboard handling. Value should be small enough to allow
/// intentional quick toggles but large enough to prevent accidental double-triggers.
const int kRelativeMouseModeToggleDebounceMs = 150;

// incomming (should be incoming) is kept, because change it will break the previous setting.

double kNewWindowOffset = isWindows
    ? 56.0
    : isLinux
        ? 50.0
        : isMacOS
            ? 30.0
            : 50.0;

const kDragToResizeAreaPaddingSize = 5.0;
EdgeInsets get kDragToResizeAreaPadding => !kUseCompatibleUiMode && isLinux
    ? stateGlobal.fullscreen.isTrue || stateGlobal.isMaximized.value
        ? EdgeInsets.zero
        : EdgeInsets.all(kDragToResizeAreaPaddingSize)
    : EdgeInsets.zero;
// https://en.wikipedia.org/wiki/Non-breaking_space
const int $nbsp = 0x00A0;

extension StringExtension on String {
  String get nonBreaking => replaceAll(' ', String.fromCharCode($nbsp));
}

const Size kConnectionManagerWindowSizeClosedChat = Size(300, 490);
const Size kConnectionManagerWindowSizeOpenChat = Size(700, 490);
// Tabbar transition duration, now we remove the duration
const Duration kTabTransitionDuration = Duration.zero;
const double kEmptyMarginTop = 50;
const double kDesktopIconButtonSplashRadius = 20;

/// [kMinCursorSize] indicates min cursor (w, h)
const int kMinCursorSize = 12;

const kFullScreenEdgeSize = 0.0;
const kMaximizeEdgeSize = 0.0;
// Do not use kWindowResizeEdgeSize directly. Use `windowResizeEdgeSize` in `common.dart` instead.
const kWindowResizeEdgeSize = 5.0;
final kWindowBorderWidth = isWindows ? 0.0 : 1.0;
const kDesktopMenuPadding = EdgeInsets.only(left: 12.0, right: 3.0);
const kFrameBorderRadius = 12.0;
const kFrameClipRRectBorderRadius = 12.0;
const kFrameBoxShadowBlurRadius = 32.0;
const kFrameBoxShadowOffsetFocused = 4.0;
const kFrameBoxShadowOffsetUnfocused = 2.0;

const kInvalidValueStr = 'InvalidValueStr';

// Config key shared by flutter and other ui.
const kCommConfKeyTheme = 'theme';
const kCommConfKeyLang = 'lang';

const kMobilePageConstraints = BoxConstraints(maxWidth: 600);

/// [kMouseControlDistance] indicates the distance that self-side move to get control of mouse.
const kMouseControlDistance = 12;

/// [kMouseControlTimeoutMSec] indicates the timeout (in milliseconds) that self-side can get control of mouse.
const kMouseControlTimeoutMSec = 1000;

/// [kRemoteViewStyleOriginal] Show remote image without scaling.
const kRemoteViewStyleOriginal = 'original';

/// [kRemoteViewStyleAdaptive] Show remote image scaling by ratio factor.
const kRemoteViewStyleAdaptive = 'adaptive';

/// [kRemoteViewStyleCustom] Show remote image at a user-defined scale percent.
const kRemoteViewStyleCustom = 'custom';

/// [kRemoteScrollStyleAuto] Scroll image auto by position.
const kRemoteScrollStyleAuto = 'scrollauto';

/// [kRemoteScrollStyleBar] Scroll image with scroll bar.
const kRemoteScrollStyleBar = 'scrollbar';

/// [kRemoteScrollStyleEdge] Scroll image auto at edges.
const kRemoteScrollStyleEdge = 'scrolledge';

/// [kScrollModeDefault] Mouse or touchpad, the default scroll mode.
const kScrollModeDefault = 'default';

/// [kScrollModeReverse] Mouse or touchpad, the reverse scroll mode.
const kScrollModeReverse = 'reverse';

/// [kRemoteImageQualityBest] Best image quality.
const kRemoteImageQualityBest = 'best';

/// [kRemoteImageQualityBalanced] Balanced image quality, mid performance.
const kRemoteImageQualityBalanced = 'balanced';

/// [kRemoteImageQualityLow] Low image quality, better performance.
const kRemoteImageQualityLow = 'low';

/// [kRemoteImageQualityCustom] Custom image quality.
const kRemoteImageQualityCustom = 'custom';

const kIgnoreDpi = true;

const Set<PointerDeviceKind> kTouchBasedDeviceKinds = {
  PointerDeviceKind.touch,
  PointerDeviceKind.stylus,
  PointerDeviceKind.invertedStylus,
};

// Scale custom related constants
const String kCustomScalePercentKey =
    'custom_scale_percent'; // Flutter option key for storing custom scale percent (integer 5-1000)
const int kScaleCustomMinPercent = 5;
const int kScaleCustomPivotPercent = 100; // 100% should be at 1/3 of track
const int kScaleCustomMaxPercent = 1000;
const double kScaleCustomPivotPos = 1.0 / 3.0; // first 1/3 → up to 100%
const double kScaleCustomDetentEpsilon =
    0.006; // snap range around pivot (~0.6%)
const Duration kDebounceCustomScaleDuration = Duration(milliseconds: 300);

// ================================ mobile ================================

// Magic numbers, maybe need to avoid it or use a better way to get them.
const kMobileDelaySoftKeyboard = Duration(milliseconds: 30);
const kMobileDelaySoftKeyboardFocus = Duration(milliseconds: 30);

/// Android constants
const kActionApplicationDetailsSettings =
    "android.settings.APPLICATION_DETAILS_SETTINGS";
const kActionAccessibilitySettings = "android.settings.ACCESSIBILITY_SETTINGS";

const kRecordAudio = "android.permission.RECORD_AUDIO";
const kRequestIgnoreBatteryOptimizations =
    "android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS";
const kSystemAlertWindow = "android.permission.SYSTEM_ALERT_WINDOW";
const kAndroid13Notification = "android.permission.POST_NOTIFICATIONS";

/// A convenient method to transform a build number to the corresponding windows version.
extension WindowsTargetExt on int {
  WindowsTarget get windowsVersion => getWindowsTarget(this);
}

const kCheckSoftwareUpdateFinish = 'check_software_update_finish';
