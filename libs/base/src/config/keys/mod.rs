//! Option keys shared across the app.
//!
//! The handful that `hbb_common` itself reads stay defined there and are
//! re-exported here, so callers always use this one path.

pub use hbb_common::config::keys::*;

mod options;
pub use options::*;
mod builtin;
pub use builtin::*;
mod connection;
pub use connection::*;
mod local;
pub use local::*;
mod display_settings;
pub use display_settings::*;
mod local_settings;
pub use local_settings::*;
mod settings;
pub use settings::*;
mod buildin_settings;
pub use buildin_settings::*;
#[cfg(test)]
mod tests;
