use super::*;

impl Encoder {
    pub fn new(config: EncoderCfg, i444: bool) -> ResultType<Encoder> {
        log::info!("new encoder: {config:?}, i444: {i444}");
        match config {
            EncoderCfg::VPX(_) => Ok(Encoder {
                codec: Box::new(VpxEncoder::new(config, i444)?),
            }),
            EncoderCfg::AOM(_) => Ok(Encoder {
                codec: Box::new(AomEncoder::new(config, i444)?),
            }),

            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(ref hw) => match HwRamEncoder::new(config.clone(), i444) {
                Ok(hw) => Ok(Encoder {
                    codec: Box::new(hw),
                }),
                Err(e) => {
                    log::error!("new hw encoder failed: {e:?}");
                    HwCodecConfig::note_failed(crate::hwcodec::config::ram_encoder_id(&hw.name));
                    *ENCODE_CODEC_FORMAT.lock().unwrap() = CodecFormat::VP9;
                    Err(e)
                }
            },
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(ref v) => match VRamEncoder::new(config.clone(), i444) {
                Ok(tex) => Ok(Encoder {
                    codec: Box::new(tex),
                }),
                Err(e) => {
                    log::error!("new vram encoder failed: {e:?}");
                    HwCodecConfig::note_failed(crate::hwcodec::config::vram_encoder_id(&v.feature));
                    *ENCODE_CODEC_FORMAT.lock().unwrap() = CodecFormat::VP9;
                    Err(e)
                }
            },
        }
    }
}

impl Encoder {
    #[inline]
    pub fn negotiated_codec() -> CodecFormat {
        ENCODE_CODEC_FORMAT.lock().unwrap().clone()
    }

    pub fn supported_encoding() -> SupportedEncoding {
        #[allow(unused_mut)]
        let mut encoding = SupportedEncoding {
            vp8: true,
            av1: !disable_av1(),
            i444: Some(CodecAbility {
                vp9: true,
                av1: true,
                ..Default::default()
            })
            .into(),
            ..Default::default()
        };
        #[cfg(feature = "hwcodec")]
        if enable_hwcodec_option() {
            encoding.h264 |= HwRamEncoder::try_get(CodecFormat::H264).is_some();
            encoding.h265 |= HwRamEncoder::try_get(CodecFormat::H265).is_some();
        }
        #[cfg(feature = "vram")]
        if enable_vram_option(true) {
            encoding.h264 |= VRamEncoder::available(CodecFormat::H264).len() > 0;
            encoding.h265 |= VRamEncoder::available(CodecFormat::H265).len() > 0;
        }
        encoding
    }

    pub fn usable_encoding() -> Option<SupportedEncoding> {
        USABLE_ENCODING.lock().unwrap().clone()
    }

    pub fn set_fallback(config: &EncoderCfg) {
        let format = match config {
            EncoderCfg::VPX(vpx) => match vpx.codec {
                VpxVideoCodecId::VP8 => CodecFormat::VP8,
                VpxVideoCodecId::VP9 => CodecFormat::VP9,
            },
            EncoderCfg::AOM(_) => CodecFormat::AV1,
            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(hw) => {
                let name = hw.name.to_lowercase();
                if name.contains("vp8") {
                    CodecFormat::VP8
                } else if name.contains("vp9") {
                    CodecFormat::VP9
                } else if name.contains("av1") {
                    CodecFormat::AV1
                } else if name.contains("h264") {
                    CodecFormat::H264
                } else {
                    CodecFormat::H265
                }
            }
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(vram) => match vram.feature.data_format {
                hwcodec::common::DataFormat::H264 => CodecFormat::H264,
                hwcodec::common::DataFormat::H265 => CodecFormat::H265,
                _ => {
                    log::error!(
                        "should not reach here, vram not support {:?}",
                        vram.feature.data_format
                    );
                    return;
                }
            },
        };
        let current = ENCODE_CODEC_FORMAT.lock().unwrap().clone();
        if current != format {
            log::info!("codec fallback: {:?} -> {:?}", current, format);
            *ENCODE_CODEC_FORMAT.lock().unwrap() = format;
        }
    }

    pub fn use_i444(config: &EncoderCfg) -> bool {
        let decodings = PEER_DECODINGS.lock().unwrap().clone();
        let prefer_i444 = decodings
            .iter()
            .all(|d| d.1.prefer_chroma == Chroma::I444.into());
        let i444_useable = match config {
            EncoderCfg::VPX(vpx) => match vpx.codec {
                VpxVideoCodecId::VP8 => false,
                VpxVideoCodecId::VP9 => decodings.iter().all(|d| d.1.i444.vp9),
            },
            EncoderCfg::AOM(_) => decodings.iter().all(|d| d.1.i444.av1),
            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(_) => false,
            #[cfg(feature = "vram")]
            EncoderCfg::VRAM(_) => false,
        };
        prefer_i444 && i444_useable && !decodings.is_empty()
    }
}
