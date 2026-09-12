//! windows implementation
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use crate::{
    send_data, send_data_exclude, ClipboardFile, CliprdrError, CliprdrServiceContext,
    ProgressPercent, ResultType, ERR_CODE_INVALID_PARAMETER, ERR_CODE_SEND_MSG,
    ERR_CODE_SERVER_FUNCTION_NONE, VEC_MSG_CHANNEL,
};
use hbb_common::{allow_err, log};
use std::{
    boxed::Box,
    ffi::{CStr, CString},
    result::Result,
};

// only used error code will be recorded here
/// success
const CHANNEL_RC_OK: u32 = 0;
/// error code from WinError.h
/// success
const ERROR_SUCCESS: u32 = 0;
/// allocation failure
const CHANNEL_RC_NO_MEMORY: u32 = 12;
/// error code from WinError.h
/// used by FreeRDP to represent errors.
const ERROR_INTERNAL_ERROR: u32 = 0x54F;

mod win_types;
pub use win_types::*;
mod cliprdr_types;
pub use cliprdr_types::*;
mod context;
pub use context::*;
mod server_clip;
pub use server_clip::*;
mod server_msgs;
pub use server_msgs::*;
mod callbacks_notify;
pub use callbacks_notify::*;
mod callbacks_client;
use callbacks_client::*;
#[cfg(test)]
mod tests;
