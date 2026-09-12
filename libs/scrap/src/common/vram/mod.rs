use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    sync::{Arc, Mutex},
};

use crate::{
    codec::{enable_vram_option, EncoderApi, EncoderCfg},
    hwcodec::HwCodecConfig,
    AdapterDevice, CodecFormat, EncodeInput, EncodeYuvFormat, Pixfmt,
};
use base::message_proto::{EncodedVideoFrame, EncodedVideoFrames, VideoFrame};
use hbb_common::{
    anyhow::{anyhow, bail, Context},
    bytes::Bytes,
    log, ResultType,
};
use hwcodec::{
    common::{DataFormat, Driver, MAX_GOP},
    vram::{
        decode::{self, DecodeFrame, Decoder},
        encode::{self, EncodeFrame, Encoder},
        Available, DecodeContext, DynamicContext, EncodeContext, FeatureContext,
    },
};

// https://www.reddit.com/r/buildapc/comments/d2m4ny/two_graphics_cards_two_monitors/
// https://www.reddit.com/r/techsupport/comments/t2v9u6/dual_monitor_setup_with_dual_gpu/
// https://cybersided.com/two-monitors-two-gpus/
// https://learn.microsoft.com/en-us/windows/win32/api/d3d12/nf-d3d12-id3d12device-getadapterluid#remarks
lazy_static::lazy_static! {
    static ref ENOCDE_NOT_USE: Arc<Mutex<HashMap<String, bool>>> = Default::default();
    static ref FALLBACK_GDI_DISPLAYS: Arc<Mutex<HashSet<String>>> = Default::default();
}

mod encoder;
pub use encoder::*;

pub struct VRamDecoder {
    decoder: Decoder,
}

impl VRamDecoder {
    pub fn try_get(format: CodecFormat, luid: Option<i64>) -> Option<DecodeContext> {
        let v: Vec<_> = Self::available(format, luid);
        if v.len() > 0 {
            // prefer ffmpeg
            if let Some(ctx) = v.iter().find(|c| c.driver == Driver::FFMPEG) {
                return Some(ctx.clone());
            }
            Some(v[0].clone())
        } else {
            None
        }
    }

    pub fn available(format: CodecFormat, luid: Option<i64>) -> Vec<DecodeContext> {
        let luid = luid.unwrap_or_default();
        let data_format = match format {
            CodecFormat::H264 => DataFormat::H264,
            CodecFormat::H265 => DataFormat::H265,
            _ => return vec![],
        };
        crate::hwcodec::HwCodecConfig::get()
            .vram_decode
            .drain(..)
            .filter(|c| c.data_format == data_format && c.luid == luid && luid != 0)
            .collect()
    }

    pub fn possible_available_without_check() -> (bool, bool) {
        if !enable_vram_option(false) {
            return (false, false);
        }
        let v = crate::hwcodec::HwCodecConfig::get().vram_decode;
        (
            v.iter().any(|d| d.data_format == DataFormat::H264),
            v.iter().any(|d| d.data_format == DataFormat::H265),
        )
    }

    pub fn new(format: CodecFormat, luid: Option<i64>) -> ResultType<Self> {
        let ctx = Self::try_get(format, luid).ok_or(anyhow!("Failed to get decode context"))?;
        log::info!("try create vram decoder: {ctx:?}");
        match Decoder::new(ctx) {
            Ok(decoder) => Ok(Self { decoder }),
            Err(_) => {
                HwCodecConfig::clear(true, false);
                Err(anyhow!(format!(
                    "Failed to create decoder, format: {:?}",
                    format
                )))
            }
        }
    }
    pub fn decode<'a>(&'a mut self, data: &[u8]) -> ResultType<Vec<VRamDecoderImage<'a>>> {
        match self.decoder.decode(data) {
            Ok(v) => Ok(v.iter().map(|f| VRamDecoderImage { frame: f }).collect()),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

pub struct VRamDecoderImage<'a> {
    pub frame: &'a DecodeFrame,
}

impl VRamDecoderImage<'_> {}

pub(crate) fn check_available_vram() -> (Vec<FeatureContext>, Vec<DecodeContext>, String) {
    let d = DynamicContext {
        device: None,
        width: 1280,
        height: 720,
        kbitrate: 5000,
        framerate: 60,
        gop: MAX_GOP as _,
    };
    let encoders = encode::available(d);
    let decoders = decode::available();
    let available = Available {
        e: encoders.clone(),
        d: decoders.clone(),
    };
    (
        encoders,
        decoders,
        available.serialize().unwrap_or_default(),
    )
}
