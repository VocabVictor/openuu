use super::*;

pub struct EncodeFrames<'a> {
    pub(super) ctx: &'a mut aom_codec_ctx_t,
    pub(super) iter: aom_codec_iter_t,
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
    pub(super) ctx: aom_codec_ctx_t,
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
    pub(super) ctx: &'a mut aom_codec_ctx_t,
    pub(super) iter: aom_codec_iter_t,
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
