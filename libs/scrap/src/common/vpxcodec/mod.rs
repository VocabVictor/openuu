// https://github.com/astraw/vpx-encode
// https://github.com/astraw/env-libvpx-sys
// https://github.com/rust-av/vpx-rs/blob/master/src/decoder.rs
// https://github.com/chromium/chromium/blob/e7b24573bc2e06fed4749dd6b6abfce67f29052f/media/video/vpx_video_encoder.cc#L522

use hbb_common::anyhow::{anyhow, Context};
use hbb_common::log;
use hbb_common::ResultType;
use base::message_proto::{Chroma, EncodedVideoFrame, EncodedVideoFrames, VideoFrame};

use crate::codec::{base_bitrate, codec_thread_num, EncoderApi};
use crate::{EncodeInput, EncodeYuvFormat, GoogleImage, Pixfmt, STRIDE_ALIGN};

use super::vpx::{vp8e_enc_control_id::*, vpx_codec_err_t::*, *};
use crate::{generate_call_macro, generate_call_ptr_macro, Error, Result};
use hbb_common::bytes::Bytes;
use std::os::raw::{c_int, c_uint};
use std::{ptr, slice};

generate_call_macro!(call_vpx, false);
generate_call_ptr_macro!(call_vpx_ptr);

mod types;
pub use types::*;
mod encoder_api;
mod encoder_impl;
mod frames;
pub use frames::*;
mod decoder;
pub use decoder::*;
mod image;
pub use image::*;
