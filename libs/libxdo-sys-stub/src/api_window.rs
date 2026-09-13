use super::*;

pub unsafe extern "C" fn xdo_get_active_window(
    xdo: *const xdo_t,
    window_ret: *mut Window,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_active_window)
        .map_or(1, |f| f(xdo, window_ret))
}

pub unsafe extern "C" fn xdo_get_focused_window(
    xdo: *const xdo_t,
    window_ret: *mut Window,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_focused_window)
        .map_or(1, |f| f(xdo, window_ret))
}

pub unsafe extern "C" fn xdo_get_focused_window_sane(
    xdo: *const xdo_t,
    window_ret: *mut Window,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_focused_window_sane)
        .map_or(1, |f| f(xdo, window_ret))
}

pub unsafe extern "C" fn xdo_get_window_location(
    xdo: *const xdo_t,
    window: Window,
    x: *mut c_int,
    y: *mut c_int,
    screen_ret: *mut *mut Screen,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_window_location)
        .map_or(1, |f| f(xdo, window, x, y, screen_ret))
}

pub unsafe extern "C" fn xdo_get_window_size(
    xdo: *const xdo_t,
    window: Window,
    width: *mut c_uint,
    height: *mut c_uint,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_window_size)
        .map_or(1, |f| f(xdo, window, width, height))
}

pub unsafe extern "C" fn xdo_get_input_state(xdo: *const xdo_t) -> c_uint {
    get_lib()
        .and_then(|lib| lib.xdo_get_input_state)
        .map_or(0, |f| f(xdo))
}

pub unsafe extern "C" fn xdo_activate_window(xdo: *const xdo_t, wid: Window) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_activate_window)
        .map_or(1, |f| f(xdo, wid))
}

pub unsafe extern "C" fn xdo_wait_for_mouse_move_from(
    xdo: *const xdo_t,
    origin_x: c_int,
    origin_y: c_int,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_wait_for_mouse_move_from)
        .map_or(1, |f| f(xdo, origin_x, origin_y))
}

pub unsafe extern "C" fn xdo_wait_for_mouse_move_to(
    xdo: *const xdo_t,
    dest_x: c_int,
    dest_y: c_int,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_wait_for_mouse_move_to)
        .map_or(1, |f| f(xdo, dest_x, dest_y))
}

pub unsafe extern "C" fn xdo_set_window_class(
    xdo: *const xdo_t,
    wid: Window,
    name: *const c_char,
    class: *const c_char,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_set_window_class)
        .map_or(1, |f| f(xdo, wid, name, class))
}

pub unsafe extern "C" fn xdo_search_windows(
    xdo: *const xdo_t,
    search: *const xdo_search_t,
    windowlist_ret: *mut *mut Window,
    nwindows_ret: *mut c_uint,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_search_windows)
        .map_or(1, |f| f(xdo, search, windowlist_ret, nwindows_ret))
}
