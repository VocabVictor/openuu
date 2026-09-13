use super::*;

pub(super) type FnXdoNew = unsafe extern "C" fn(*const c_char) -> *mut xdo_t;
pub(super) type FnXdoNewWithOpenedDisplay =
    unsafe extern "C" fn(*mut Display, *const c_char, c_int) -> *mut xdo_t;
pub(super) type FnXdoFree = unsafe extern "C" fn(*mut xdo_t);
pub(super) type FnXdoSendKeysequenceWindow =
    unsafe extern "C" fn(*const xdo_t, Window, *const c_char, useconds_t) -> c_int;
pub(super) type FnXdoSendKeysequenceWindowDown =
    unsafe extern "C" fn(*const xdo_t, Window, *const c_char, useconds_t) -> c_int;
pub(super) type FnXdoSendKeysequenceWindowUp =
    unsafe extern "C" fn(*const xdo_t, Window, *const c_char, useconds_t) -> c_int;
pub(super) type FnXdoEnterTextWindow =
    unsafe extern "C" fn(*const xdo_t, Window, *const c_char, useconds_t) -> c_int;
pub(super) type FnXdoClickWindow = unsafe extern "C" fn(*const xdo_t, Window, c_int) -> c_int;
pub(super) type FnXdoMouseDown = unsafe extern "C" fn(*const xdo_t, Window, c_int) -> c_int;
pub(super) type FnXdoMouseUp = unsafe extern "C" fn(*const xdo_t, Window, c_int) -> c_int;
pub(super) type FnXdoMoveMouse = unsafe extern "C" fn(*const xdo_t, c_int, c_int, c_int) -> c_int;
pub(super) type FnXdoMoveMouseRelative = unsafe extern "C" fn(*const xdo_t, c_int, c_int) -> c_int;
pub(super) type FnXdoMoveMouseRelativeToWindow =
    unsafe extern "C" fn(*const xdo_t, Window, c_int, c_int) -> c_int;
pub(super) type FnXdoGetMouseLocation =
    unsafe extern "C" fn(*const xdo_t, *mut c_int, *mut c_int, *mut c_int) -> c_int;
pub(super) type FnXdoGetMouseLocation2 =
    unsafe extern "C" fn(*const xdo_t, *mut c_int, *mut c_int, *mut c_int, *mut Window) -> c_int;
pub(super) type FnXdoGetActiveWindow = unsafe extern "C" fn(*const xdo_t, *mut Window) -> c_int;
pub(super) type FnXdoGetFocusedWindow = unsafe extern "C" fn(*const xdo_t, *mut Window) -> c_int;
pub(super) type FnXdoGetFocusedWindowSane = unsafe extern "C" fn(*const xdo_t, *mut Window) -> c_int;
pub(super) type FnXdoGetWindowLocation =
    unsafe extern "C" fn(*const xdo_t, Window, *mut c_int, *mut c_int, *mut *mut Screen) -> c_int;
pub(super) type FnXdoGetWindowSize =
    unsafe extern "C" fn(*const xdo_t, Window, *mut c_uint, *mut c_uint) -> c_int;
pub(super) type FnXdoGetInputState = unsafe extern "C" fn(*const xdo_t) -> c_uint;
pub(super) type FnXdoActivateWindow = unsafe extern "C" fn(*const xdo_t, Window) -> c_int;
pub(super) type FnXdoWaitForMouseMoveFrom = unsafe extern "C" fn(*const xdo_t, c_int, c_int) -> c_int;
pub(super) type FnXdoWaitForMouseMoveTo = unsafe extern "C" fn(*const xdo_t, c_int, c_int) -> c_int;
pub(super) type FnXdoSetWindowClass =
    unsafe extern "C" fn(*const xdo_t, Window, *const c_char, *const c_char) -> c_int;
pub(super) type FnXdoSearchWindows =
    unsafe extern "C" fn(*const xdo_t, *const xdo_search_t, *mut *mut Window, *mut c_uint) -> c_int;
