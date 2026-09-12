use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VpxVideoCodecId {
    VP8,
    VP9,
}

impl Default for VpxVideoCodecId {
    fn default() -> VpxVideoCodecId {
        VpxVideoCodecId::VP9
    }
}

pub struct VpxEncoder {
    pub(super) ctx: vpx_codec_ctx_t,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) id: VpxVideoCodecId,
    pub(super) i444: bool,
    pub(super) yuvfmt: EncodeYuvFormat,
}

pub struct VpxDecoder {
    pub(super) ctx: vpx_codec_ctx_t,
}
