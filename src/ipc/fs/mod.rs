#[cfg(target_os = "linux")]
use super::ipc_auth::active_uid;
use crate::ipc::{connect, Data};
use hbb_common::{config, log, ResultType};
use std::{
    ffi::CString,
    io::{Error, ErrorKind},
    os::unix::ffi::OsStrExt,
    path::Path,
};

mod parent_dir;
pub(crate) use parent_dir::*;
mod secure_dir;
pub(crate) use secure_dir::*;
mod pid_file;
pub(crate) use pid_file::*;

struct FdGuard(i32);
impl Drop for FdGuard {
    fn drop(&mut self) {
        unsafe {
            hbb_common::libc::close(self.0);
        }
    }
}

#[cfg(test)]
mod tests {

    mod tests_a;
    mod tests_b;
}
