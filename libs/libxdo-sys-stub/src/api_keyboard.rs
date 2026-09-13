use super::*;

pub unsafe extern "C" fn xdo_new(display: *const c_char) -> *mut xdo_t {
    get_lib().map_or(std::ptr::null_mut(), |lib| (lib.xdo_new)(display))
}

pub unsafe extern "C" fn xdo_new_with_opened_display(
    xdpy: *mut Display,
    display: *const c_char,
    close_display_when_freed: c_int,
) -> *mut xdo_t {
    get_lib()
        .and_then(|lib| lib.xdo_new_with_opened_display)
        .map_or(std::ptr::null_mut(), |f| {
            f(xdpy, display, close_display_when_freed)
        })
}

pub unsafe extern "C" fn xdo_free(xdo: *mut xdo_t) {
    if xdo.is_null() {
        return;
    }
    if let Some(lib) = get_lib() {
        (lib.xdo_free)(xdo);
    }
}

pub unsafe extern "C" fn xdo_send_keysequence_window(
    xdo: *const xdo_t,
    window: Window,
    keysequence: *const c_char,
    delay: useconds_t,
) -> c_int {
    get_lib().map_or(1, |lib| {
        (lib.xdo_send_keysequence_window)(xdo, window, keysequence, delay)
    })
}

pub unsafe extern "C" fn xdo_send_keysequence_window_down(
    xdo: *const xdo_t,
    window: Window,
    keysequence: *const c_char,
    delay: useconds_t,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_send_keysequence_window_down)
        .map_or(1, |f| f(xdo, window, keysequence, delay))
}

pub unsafe extern "C" fn xdo_send_keysequence_window_up(
    xdo: *const xdo_t,
    window: Window,
    keysequence: *const c_char,
    delay: useconds_t,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_send_keysequence_window_up)
        .map_or(1, |f| f(xdo, window, keysequence, delay))
}

pub unsafe extern "C" fn xdo_enter_text_window(
    xdo: *const xdo_t,
    window: Window,
    string: *const c_char,
    delay: useconds_t,
) -> c_int {
    get_lib()
        .and_then(|lib| lib.xdo_enter_text_window)
        .map_or(1, |f| f(xdo, window, string, delay))
}
