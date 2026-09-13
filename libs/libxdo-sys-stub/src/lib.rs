//! Dynamic loading wrapper for libxdo.
//!
//! Provides the same API as libxdo-sys but loads libxdo at runtime,
//! allowing the program to run on systems without libxdo installed
//! (e.g., Wayland-only environments).

use hbb_common::{
    libc::{c_char, c_int, c_uint},
    libloading::{Library, Symbol},
    log,
};
use std::sync::OnceLock;

pub use hbb_common::x11::xlib::{Display, Screen, Window};

#[repr(C)]
pub struct xdo_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct charcodemap_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct xdo_search_t {
    _private: [u8; 0],
}

pub type useconds_t = c_uint;

pub const CURRENTWINDOW: Window = 0;

mod fn_types;
use fn_types::*;
mod loader;
use loader::*;

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
