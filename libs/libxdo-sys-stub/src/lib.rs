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
mod api_keyboard;
pub use api_keyboard::*;
mod api_mouse;
pub use api_mouse::*;
mod api_window;
pub use api_window::*;
