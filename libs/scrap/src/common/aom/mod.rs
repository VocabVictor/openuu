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

pub struct EncodeFrames<'a> {
    ctx: &'a mut aom_codec_ctx_t,
    iter: aom_codec_iter_t,
}

impl<'a> Iterator for EncodeFrames<'a> {
    type Item = EncodeFrame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            unsafe {
                let pkt = aom_codec_get_cx_data(self.ctx, &mut self.iter);
                if pkt.is_null() {
                    return None;
                } else if (*pkt).kind == aom_codec_cx_pkt_kind::AOM_CODEC_CX_FRAME_PKT {
                    let f = &(*pkt).data.frame;
                    return Some(Self::Item {
                        data: slice::from_raw_parts(f.buf as _, f.sz as _),
                        key: (f.flags & AOM_FRAME_IS_KEY) != 0,
                        pts: f.pts,
                    });
                } else {
                    // Ignore the packet.
                }
            }
        }
    }
}

pub struct AomDecoder {
    ctx: aom_codec_ctx_t,
}

impl AomDecoder {
    pub fn new() -> Result<Self> {
        let i = call_aom_ptr!(aom_codec_av1_dx());
        let mut ctx = Default::default();
        let cfg = aom_codec_dec_cfg_t {
            threads: codec_thread_num(64) as _,
            w: 0,
            h: 0,
            allow_lowbitdepth: 1,
        };
        call_aom!(aom_codec_dec_init_ver(
            &mut ctx,
            i,
            &cfg,
            0,
            AOM_DECODER_ABI_VERSION as _,
        ));
        Ok(Self { ctx })
    }

    pub fn decode<'a>(&'a mut self, data: &[u8]) -> Result<DecodeFrames<'a>> {
        call_aom!(aom_codec_decode(
            &mut self.ctx,
            data.as_ptr(),
            data.len() as _,
            ptr::null_mut(),
        ));

        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the decoder to return any pending frame
    pub fn flush<'a>(&'a mut self) -> Result<DecodeFrames<'a>> {
        call_aom!(aom_codec_decode(
            &mut self.ctx,
            ptr::null(),
            0,
            ptr::null_mut(),
        ));
        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }
}

impl Drop for AomDecoder {
    fn drop(&mut self) {
        unsafe {
            let result = aom_codec_destroy(&mut self.ctx);
            if result != aom_codec_err_t::AOM_CODEC_OK {
                panic!("failed to destroy aom codec");
            }
        }
    }
}

pub struct DecodeFrames<'a> {
    ctx: &'a mut aom_codec_ctx_t,
    iter: aom_codec_iter_t,
}

impl<'a> Iterator for DecodeFrames<'a> {
    type Item = Image;
    fn next(&mut self) -> Option<Self::Item> {
        let img = unsafe { aom_codec_get_frame(self.ctx, &mut self.iter) };
        if img.is_null() {
            return None;
        } else {
            return Some(Image(img));
        }
    }
}

pub struct Image(*mut aom_image_t);
impl Image {
    #[inline]
    pub fn new() -> Self {
        Self(std::ptr::null_mut())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn format(&self) -> aom_img_fmt_t {
        self.inner().fmt
    }

    #[inline]
    pub fn inner(&self) -> &aom_image_t {
        unsafe { &*self.0 }
    }
}

impl GoogleImage for Image {
    #[inline]
    fn width(&self) -> usize {
        self.inner().d_w as _
    }

    #[inline]
    fn height(&self) -> usize {
        self.inner().d_h as _
    }

    #[inline]
    fn stride(&self) -> Vec<i32> {
        self.inner().stride.iter().map(|x| *x as i32).collect()
    }

    #[inline]
    fn planes(&self) -> Vec<*mut u8> {
        self.inner().planes.iter().map(|p| *p as *mut u8).collect()
    }

    fn chroma(&self) -> Chroma {
        match self.inner().fmt {
            aom_img_fmt::AOM_IMG_FMT_I444 => Chroma::I444,
            _ => Chroma::I420,
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { aom_img_free(self.0) };
        }
    }
}

unsafe impl Send for aom_codec_ctx_t {}
