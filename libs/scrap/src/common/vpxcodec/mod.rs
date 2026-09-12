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

impl VpxDecoder {
    /// Create a new decoder
    ///
    /// # Errors
    ///
    /// The function may fail if the underlying libvpx does not provide
    /// the VP9 decoder.
    pub fn new(config: VpxDecoderConfig) -> Result<Self> {
        // This is sound because `vpx_codec_ctx` is a repr(C) struct without any field that can
        // cause UB if uninitialized.
        let i = match config.codec {
            VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_dx()),
            VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_dx()),
        };
        let mut ctx = Default::default();
        let cfg = vpx_codec_dec_cfg_t {
            threads: codec_thread_num(64) as _,
            w: 0,
            h: 0,
        };
        /*
        unsafe {
            println!("{}", vpx_codec_get_caps(i));
        }
        */
        call_vpx!(vpx_codec_dec_init_ver(
            &mut ctx,
            i,
            &cfg,
            0,
            VPX_DECODER_ABI_VERSION as _,
        ));
        Ok(Self { ctx })
    }

    /// Feed some compressed data to the encoder
    ///
    /// The `data` slice is sent to the decoder
    ///
    /// It matches a call to `vpx_codec_decode`.
    pub fn decode<'a>(&'a mut self, data: &[u8]) -> Result<DecodeFrames<'a>> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            data.as_ptr(),
            data.len() as _,
            ptr::null_mut(),
            0,
        ));

        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the decoder to return any pending frame
    pub fn flush<'a>(&'a mut self) -> Result<DecodeFrames<'a>> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        ));
        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }
}

impl Drop for VpxDecoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

pub struct DecodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for DecodeFrames<'a> {
    type Item = Image;
    fn next(&mut self) -> Option<Self::Item> {
        let img = unsafe { vpx_codec_get_frame(self.ctx, &mut self.iter) };
        if img.is_null() {
            return None;
        } else {
            return Some(Image(img));
        }
    }
}

// https://chromium.googlesource.com/webm/libvpx/+/bali/vpx/src/vpx_image.c
pub struct Image(*mut vpx_image_t);
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
    pub fn format(&self) -> vpx_img_fmt_t {
        // VPX_IMG_FMT_I420
        self.inner().fmt
    }

    #[inline]
    pub fn inner(&self) -> &vpx_image_t {
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
            vpx_img_fmt::VPX_IMG_FMT_I444 => Chroma::I444,
            _ => Chroma::I420,
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { vpx_img_free(self.0) };
        }
    }
}

unsafe impl Send for vpx_codec_ctx_t {}
