#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(improper_ctypes)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/aom_ffi.rs"));

use crate::codec::{base_bitrate, codec_thread_num};
use crate::{codec::EncoderApi, EncodeFrame, STRIDE_ALIGN};
use crate::{common::GoogleImage, generate_call_macro, generate_call_ptr_macro, Error, Result};
use crate::{EncodeInput, EncodeYuvFormat, Pixfmt};
use hbb_common::{
    anyhow::{anyhow, Context},
    bytes::Bytes,
    log, ResultType,
};
use base::message_proto::{Chroma, EncodedVideoFrame, EncodedVideoFrames, VideoFrame};
use std::{ptr, slice};

generate_call_macro!(call_aom, false);
generate_call_macro!(call_aom_allow_err, true);
generate_call_ptr_macro!(call_aom_ptr);

mod webrtc;
mod encoder;
mod frames_decoder;
pub use frames_decoder::*;
mod image;
pub use image::*;

impl Default for aom_codec_enc_cfg_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for aom_codec_ctx_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl Default for aom_image_t {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AomEncoderConfig {
    pub width: u32,
    pub height: u32,
    pub quality: f32,
    pub keyframe_interval: Option<usize>,
}

pub struct AomEncoder {
    ctx: aom_codec_ctx_t,
    width: usize,
    height: usize,
    i444: bool,
    yuvfmt: EncodeYuvFormat,
}

unsafe impl Send for aom_codec_ctx_t {}
