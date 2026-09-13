use crate::{
    codec::{
        base_bitrate, codec_thread_num, enable_hwcodec_option, fps_compensated_kbs, EncoderApi,
        EncoderCfg,
    },
    convert::*,
    CodecFormat, EncodeInput, ImageFormat, ImageRgb, Pixfmt, HW_STRIDE_ALIGN,
};
use base::message_proto::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame};
use hbb_common::{
    anyhow::{anyhow, bail, Context},
    bytes::Bytes,
    log,
    serde_derive::{Deserialize, Serialize},
    serde_json, ResultType,
};
use hwcodec::{
    common::{
        DataFormat, HwcodecErrno,
        Quality::{self, *},
        RateControl::{self, *},
    },
    ffmpeg::AVPixelFormat,
    ffmpeg_ram::{
        decode::{DecodeContext, DecodeFrame, Decoder},
        encode::{EncodeContext, EncodeFrame, Encoder},
        ffmpeg_linesize_offset_length, CodecInfo,
    },
};

const DEFAULT_PIXFMT: AVPixelFormat = AVPixelFormat::AV_PIX_FMT_NV12;
pub const DEFAULT_FPS: i32 = 30;
const DEFAULT_GOP: i32 = i32::MAX;
const DEFAULT_HW_QUALITY: Quality = Quality_Default;
pub const ERR_HEVC_POC: i32 = HwcodecErrno::HWCODEC_ERR_HEVC_COULD_NOT_FIND_POC as i32;

crate::generate_call_macro!(call_yuv, false);

mod ram_encoder;
pub use ram_encoder::*;
mod ram_decoder;
pub use ram_decoder::*;
pub mod config;
pub use config::*;
mod check;
pub use check::*;

#[cfg(not(target_os = "android"))]
lazy_static::lazy_static! {
    static ref CONFIG: std::sync::Arc<std::sync::Mutex<Option<HwCodecConfig>>> = Default::default();
    static ref CONFIG_SET_BY_IPC: std::sync::Arc<std::sync::Mutex<bool>> = Default::default();
}
