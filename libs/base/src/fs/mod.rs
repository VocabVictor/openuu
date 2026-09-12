#[cfg(windows)]
use std::os::windows::prelude::*;
use std::{
    fmt::{Debug, Display},
    io::Cursor,
    path::{Path, PathBuf},
    sync::atomic::{AtomicI32, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufStream as TokioBufStream},
};

use crate::message_proto::*;
// https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html
use hbb_common::{
    anyhow::anyhow,
    bail,
    compress::{compress, decompress},
    config::Config,
    get_version_number, ResultType, Stream,
};

static NEXT_JOB_ID: AtomicI32 = AtomicI32::new(1);

mod dir_read;
pub use dir_read::*;
mod job_types;
pub use job_types::*;
mod job_structs;
pub use job_structs::*;
mod validation;
pub use validation::*;
mod job_new;
mod job_write;
mod job_read;
mod job_state;
mod messages;
pub use messages::*;
mod jobs;
pub use jobs::*;
mod file_ops;
pub use file_ops::*;

#[cfg(test)]
#[path = "fs_transfer_tests.rs"]
mod transfer_network_tests;

pub fn get_next_job_id() -> i32 {
    NEXT_JOB_ID.fetch_add(1, Ordering::SeqCst)
}

pub fn update_next_job_id(id: i32) {
    NEXT_JOB_ID.store(id, Ordering::SeqCst);
}

impl TransferJob {
}

#[cfg(test)]
mod tests {
    use super::*;
    use protobuf::Message as _;

    mod transfer_tests;
    use transfer_tests::*;
    mod helpers;
    use helpers::*;
    mod validation_tests;
    use validation_tests::*;

}
