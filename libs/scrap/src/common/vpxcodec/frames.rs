use super::*;

#[derive(Clone, Copy, Debug)]
pub struct EncodeFrame<'a> {
    /// Compressed data.
    pub data: &'a [u8],
    /// Whether the frame is a keyframe.
    pub key: bool,
    /// Presentation timestamp (in timebase units).
    pub pts: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxEncoderConfig {
    /// The width (in pixels).
    pub width: c_uint,
    /// The height (in pixels).
    pub height: c_uint,
    /// The bitrate ratio
    pub quality: f32,
    /// The codec
    pub codec: VpxVideoCodecId,
    /// keyframe interval
    pub keyframe_interval: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxDecoderConfig {
    pub codec: VpxVideoCodecId,
}

pub struct EncodeFrames<'a> {
    pub(super) ctx: &'a mut vpx_codec_ctx_t,
    pub(super) iter: vpx_codec_iter_t,
}

impl<'a> Iterator for EncodeFrames<'a> {
    type Item = EncodeFrame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            unsafe {
                let pkt = vpx_codec_get_cx_data(self.ctx, &mut self.iter);
                if pkt.is_null() {
                    return None;
                } else if (*pkt).kind == vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT {
                    let f = &(*pkt).data.frame;
                    return Some(Self::Item {
                        data: slice::from_raw_parts(f.buf as _, f.sz as _),
                        key: (f.flags & VPX_FRAME_IS_KEY) != 0,
                        pts: f.pts,
                    });
                } else {
                    // Ignore the packet.
                }
            }
        }
    }
}
