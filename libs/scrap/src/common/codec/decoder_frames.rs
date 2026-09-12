use super::*;

impl Decoder {
    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub fn handle_video_frame(
        &mut self,
        frame: &video_frame::Union,
        rgb: &mut ImageRgb,
        _texture: &mut ImageTexture,
        _pixelbuffer: &mut bool,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        match frame {
            video_frame::Union::Vp8s(vp8s) => {
                if let Some(vp8) = &mut self.vp8 {
                    Decoder::handle_vpxs_video_frame(vp8, vp8s, rgb, chroma)
                } else {
                    bail!("vp8 decoder not available");
                }
            }
            video_frame::Union::Vp9s(vp9s) => {
                if let Some(vp9) = &mut self.vp9 {
                    Decoder::handle_vpxs_video_frame(vp9, vp9s, rgb, chroma)
                } else {
                    bail!("vp9 decoder not available");
                }
            }
            video_frame::Union::Av1s(av1s) => {
                if let Some(av1) = &mut self.av1 {
                    Decoder::handle_av1s_video_frame(av1, av1s, rgb, chroma)
                } else {
                    bail!("av1 decoder not available");
                }
            }
            #[cfg(any(feature = "hwcodec", feature = "vram"))]
            video_frame::Union::H264s(h264s) => {
                *chroma = Some(Chroma::I420);
                #[cfg(feature = "vram")]
                if let Some(decoder) = &mut self.h264_vram {
                    *_pixelbuffer = false;
                    return Decoder::handle_vram_video_frame(decoder, h264s, _texture);
                }
                #[cfg(feature = "hwcodec")]
                if let Some(decoder) = &mut self.h264_ram {
                    return Decoder::handle_hwram_video_frame(decoder, h264s, rgb, &mut self.i420);
                }
                Err(anyhow!("don't support h264!"))
            }
            #[cfg(any(feature = "hwcodec", feature = "vram"))]
            video_frame::Union::H265s(h265s) => {
                *chroma = Some(Chroma::I420);
                #[cfg(feature = "vram")]
                if let Some(decoder) = &mut self.h265_vram {
                    *_pixelbuffer = false;
                    return Decoder::handle_vram_video_frame(decoder, h265s, _texture);
                }
                #[cfg(feature = "hwcodec")]
                if let Some(decoder) = &mut self.h265_ram {
                    return Decoder::handle_hwram_video_frame(decoder, h265s, rgb, &mut self.i420);
                }
                Err(anyhow!("don't support h265!"))
            }
            #[cfg(feature = "mediacodec")]
            video_frame::Union::H264s(h264s) => {
                *chroma = Some(Chroma::I420);
                if let Some(decoder) = &mut self.h264_media_codec {
                    Decoder::handle_mediacodec_video_frame(decoder, h264s, rgb)
                } else {
                    Err(anyhow!("don't support h264!"))
                }
            }
            #[cfg(feature = "mediacodec")]
            video_frame::Union::H265s(h265s) => {
                *chroma = Some(Chroma::I420);
                if let Some(decoder) = &mut self.h265_media_codec {
                    Decoder::handle_mediacodec_video_frame(decoder, h265s, rgb)
                } else {
                    Err(anyhow!("don't support h265!"))
                }
            }
            _ => Err(anyhow!("unsupported video frame type!")),
        }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub(super) fn handle_vpxs_video_frame(
        decoder: &mut VpxDecoder,
        vpxs: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        let mut last_frame = vpxcodec::Image::new();
        for vpx in vpxs.frames.iter() {
            for frame in decoder.decode(&vpx.data)? {
                drop(last_frame);
                last_frame = frame;
            }
        }
        for frame in decoder.flush()? {
            drop(last_frame);
            last_frame = frame;
        }
        if last_frame.is_null() {
            Ok(false)
        } else {
            *chroma = Some(last_frame.chroma());
            last_frame.to(rgb);
            Ok(true)
        }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    pub(super) fn handle_av1s_video_frame(
        decoder: &mut AomDecoder,
        av1s: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        chroma: &mut Option<Chroma>,
    ) -> ResultType<bool> {
        let mut last_frame = aom::Image::new();
        for av1 in av1s.frames.iter() {
            for frame in decoder.decode(&av1.data)? {
                drop(last_frame);
                last_frame = frame;
            }
        }
        for frame in decoder.flush()? {
            drop(last_frame);
            last_frame = frame;
        }
        if last_frame.is_null() {
            Ok(false)
        } else {
            *chroma = Some(last_frame.chroma());
            last_frame.to(rgb);
            Ok(true)
        }
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    #[cfg(feature = "hwcodec")]
    pub(super) fn handle_hwram_video_frame(
        decoder: &mut HwRamDecoder,
        frames: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        i420: &mut Vec<u8>,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in frames.frames.iter() {
            for image in decoder.decode(&h264.data)? {
                // TODO: just process the last frame
                if image.to_fmt(rgb, i420).is_ok() {
                    ret = true;
                }
            }
        }
        return Ok(ret);
    }

    #[cfg(feature = "vram")]
    pub(super) fn handle_vram_video_frame(
        decoder: &mut VRamDecoder,
        frames: &EncodedVideoFrames,
        texture: &mut ImageTexture,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h26x in frames.frames.iter() {
            for image in decoder.decode(&h26x.data)? {
                *texture = ImageTexture {
                    texture: image.frame.texture,
                    w: image.frame.width as _,
                    h: image.frame.height as _,
                };
                ret = true;
            }
        }
        return Ok(ret);
    }

    // rgb [in/out] fmt and stride must be set in ImageRgb
    #[cfg(feature = "mediacodec")]
    pub(super) fn handle_mediacodec_video_frame(
        decoder: &mut MediaCodecDecoder,
        frames: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
    ) -> ResultType<bool> {
        let mut ret = false;
        for h264 in frames.frames.iter() {
            return decoder.decode(&h264.data, rgb);
        }
        return Ok(false);
    }

    pub(super) fn preference(id: Option<&str>) -> (PreferCodec, Chroma) {
        let id = id.unwrap_or_default();
        if id.is_empty() {
            return (PreferCodec::Auto, Chroma::I420);
        }
        let options = PeerConfig::load(id).options;
        let codec = options
            .get("codec-preference")
            .map_or("".to_owned(), |c| c.to_owned());
        let codec = if codec == "vp8" {
            PreferCodec::VP8
        } else if codec == "vp9" {
            PreferCodec::VP9
        } else if codec == "av1" {
            PreferCodec::AV1
        } else if codec == "h264" {
            PreferCodec::H264
        } else if codec == "h265" {
            PreferCodec::H265
        } else {
            PreferCodec::Auto
        };
        let chroma = if options.get("i444") == Some(&"Y".to_string()) {
            Chroma::I444
        } else {
            Chroma::I420
        };
        (codec, chroma)
    }
}
