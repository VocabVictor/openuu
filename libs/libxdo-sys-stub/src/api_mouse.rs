use super::*;

pub unsafe extern "C" fn xdo_click_window(
    xdo: *const xdo_t,
    window: Window,
    button: c_int,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_click_window)
        .map_or(1, |f| f(xdo, window, button))
}

pub unsafe extern "C" fn xdo_mouse_down(xdo: *const xdo_t, window: Window, button: c_int) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_mouse_down)
        .map_or(1, |f| f(xdo, window, button))
}

pub unsafe extern "C" fn xdo_mouse_up(xdo: *const xdo_t, window: Window, button: c_int) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_mouse_up)
        .map_or(1, |f| f(xdo, window, button))
}

pub unsafe extern "C" fn xdo_move_mouse(
    xdo: *const xdo_t,
    x: c_int,
    y: c_int,
    screen: c_int,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_move_mouse)
        .map_or(1, |f| f(xdo, x, y, screen))
}

pub unsafe extern "C" fn xdo_move_mouse_relative(xdo: *const xdo_t, x: c_int, y: c_int) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_move_mouse_relative)
        .map_or(1, |f| f(xdo, x, y))
}

pub unsafe extern "C" fn xdo_move_mouse_relative_to_window(
    xdo: *const xdo_t,
    window: Window,
    x: c_int,
    y: c_int,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_move_mouse_relative_to_window)
        .map_or(1, |f| f(xdo, window, x, y))
}

pub unsafe extern "C" fn xdo_get_mouse_location(
    xdo: *const xdo_t,
    x: *mut c_int,
    y: *mut c_int,
    screen_num: *mut c_int,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_mouse_location)
        .map_or(1, |f| f(xdo, x, y, screen_num))
}

pub unsafe extern "C" fn xdo_get_mouse_location2(
    xdo: *const xdo_t,
    x: *mut c_int,
    y: *mut c_int,
    screen_num: *mut c_int,
    window: *mut Window,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_get_mouse_location2)
        .map_or(1, |f| f(xdo, x, y, screen_num, window))
}
