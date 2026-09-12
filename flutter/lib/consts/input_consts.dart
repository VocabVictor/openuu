part of 'consts.dart';

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

extension StringExtension on String {
  String get nonBreaking => replaceAll(' ', String.fromCharCode($nbsp));
}

const Size kConnectionManagerWindowSizeClosedChat = Size(300, 490);
const Size kConnectionManagerWindowSizeOpenChat = Size(700, 490);
// Tabbar transition duration, now we remove the duration
const Duration kTabTransitionDuration = Duration.zero;
const double kEmptyMarginTop = 50;
