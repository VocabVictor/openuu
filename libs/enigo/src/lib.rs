//! Enigo lets you simulate mouse and keyboard input-events as if they were
//! made by the actual hardware. The goal is to make it available on different
//! operating systems like Linux, macOS and Windows – possibly many more but
//! [Redox](https://redox-os.org/) and *BSD are planned. Please see the
//! [Repo](https://github.com/enigo-rs/enigo) for the current status.
//!
//! I consider this library in an early alpha status, the API will change in
//! in the future. The keyboard handling is far from being very usable. I plan
//! to build a simple
//! [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! that will resemble something like:
//!
//! `"hello {+SHIFT}world{-SHIFT} and break line{ENTER}"`
//!
//! The current status is that you can just print
//! [unicode](http://unicode.org/)
//! characters like [emoji](http://getemoji.com/) without the `{+SHIFT}`
//! [DSL](https://en.wikipedia.org/wiki/Domain-specific_language)
//! or any other "special" key on the Linux, macOS and Windows operating system.
//!
//! Possible use cases could be for testing user interfaces on different
//! platforms,
//! building remote control applications or just automating tasks for user
//! interfaces unaccessible by a public API or scripting language.
//!
//! For the keyboard there are currently two modes you can use. The first mode
//! is represented by the [key_sequence]() function
//! its purpose is to simply write unicode characters. This is independent of
//! the keyboardlayout. Please note that
//! you're not be able to use modifier keys like Control
//! to influence the outcome. If you want to use modifier keys to e.g.
//! copy/paste
//! use the Layout variant. Please note that this is indeed layout dependent.

//! # Examples
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new();
//! //paste
//! enigo.key_down(Key::Control);
//! enigo.key_click(Key::Layout('v'));
//! enigo.key_up(Key::Control);
//! ```
//!
//! ```no_run
//! use enigo::*;
//! let mut enigo = Enigo::new();
//! enigo.mouse_move_to(500, 200);
//! enigo.mouse_down(MouseButton::Left);
//! enigo.mouse_move_relative(100, 100);
//! enigo.mouse_up(MouseButton::Left);
//! enigo.key_sequence("hello world");
//! ```
#![deny(missing_docs)]

#[cfg(target_os = "macos")]
#[macro_use]
extern crate objc;

// TODO(dustin) use interior mutability not &mut self

#[cfg(target_os = "windows")]
mod win;
#[cfg(target_os = "windows")]
pub use win::Enigo;
#[cfg(target_os = "windows")]
pub use win::ENIGO_INPUT_EXTRA_VALUE;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::Enigo;
#[cfg(target_os = "macos")]
pub use macos::ENIGO_INPUT_EXTRA_VALUE;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use crate::linux::Enigo;

/// DSL parser module
pub mod dsl;

#[cfg(feature = "with_serde")]
#[macro_use]
extern crate serde_derive;

#[cfg(feature = "with_serde")]
extern crate serde;

///
pub type ResultType = std::result::Result<(), Box<dyn std::error::Error>>;

mod mouse;
pub use mouse::*;
mod key;
pub use key::*;
mod stub;
#[cfg(any(target_os = "android", target_os = "ios"))]
use stub::Enigo;
#[cfg(test)]
mod tests;
