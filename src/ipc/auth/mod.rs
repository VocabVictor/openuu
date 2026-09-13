use crate::ipc::{Connection, ConnectionTmpl};
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
use hbb_common::{anyhow, bail, log, ResultType};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use hbb_common::{
    libc,
    tokio::io::{AsyncRead, AsyncWrite},
};
#[cfg(windows)]
use parity_tokio_ipc::SecurityAttributes;
#[cfg(windows)]
use std::io;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::os::unix::io::RawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(windows)]
use windows::Win32::{Foundation::HANDLE, System::Pipes::GetNamedPipeClientProcessId};
#[cfg(windows)]
mod windows_conn;
#[cfg(windows)]
mod windows_auth;
#[cfg(windows)]
pub(crate) use windows_auth::*;
#[cfg(windows)]
pub(crate) use windows_conn::*;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_peer;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) use unix_peer::*;
mod exe_path;
pub(crate) use exe_path::*;
mod authorize;
pub(crate) use authorize::*;
#[cfg(test)]
mod tests;
