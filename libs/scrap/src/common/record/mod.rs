use crate::CodecFormat;
use base::message_proto::{message, video_frame, EncodedVideoFrame, Message};
#[cfg(feature = "hwcodec")]
use hbb_common::anyhow::anyhow;
use hbb_common::{bail, chrono, log, ResultType};
#[cfg(feature = "hwcodec")]
use hwcodec::mux::{MuxContext, Muxer};
use std::{
    fs::{File, OpenOptions},
    io,
    ops::{Deref, DerefMut},
    path::PathBuf,
    sync::mpsc::Sender,
    time::Instant,
};
use webm::mux::{self, Segment, Track, VideoTrack, Writer};

const MIN_SECS: u64 = 1;

mod context;
pub use context::*;
mod recorder;
mod webm_recorder;
use webm_recorder::*;
#[cfg(feature = "hwcodec")]
mod hw;
#[cfg(feature = "hwcodec")]
use hw::*;
#[cfg(test)]
mod tests;
