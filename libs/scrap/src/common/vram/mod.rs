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
mod decoder;
pub use decoder::*;
