use clipboard::ClipboardFile;
use base::message_proto::*;

mod convert;
pub use convert::*;
#[cfg(feature = "unix-file-copy-paste")]
pub mod unix_file_clip;
