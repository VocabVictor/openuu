use super::*;

impl Decoder {
    pub fn supported_decodings(
        id_for_perfer: Option<&str>,
        _use_texture_render: bool,
        _luid: Option<i64>,
        mark_unsupported: &Vec<CodecFormat>,
    ) -> SupportedDecoding {
        let (prefer, prefer_chroma) = Self::preference(id_for_perfer);

        #[allow(unused_mut)]
        let mut decoding = SupportedDecoding {
            ability_vp8: 1,
            ability_vp9: 1,
            ability_av1: if disable_av1() { 0 } else { 1 },
            i444: Some(CodecAbility {
                vp9: true,
                av1: true,
                ..Default::default()
            })
            .into(),
            prefer: prefer.into(),
            prefer_chroma: prefer_chroma.into(),
            ..Default::default()
        };
        #[cfg(feature = "hwcodec")]
        {
            decoding.ability_h264 |= if HwRamDecoder::try_get(CodecFormat::H264).is_some() {
                1
            } else {
                0
            };
            decoding.ability_h265 |= if HwRamDecoder::try_get(CodecFormat::H265).is_some() {
                1
            } else {
                0
            };
        }
        #[cfg(feature = "vram")]
        if enable_vram_option(false) && _use_texture_render {
            decoding.ability_h264 |= if VRamDecoder::available(CodecFormat::H264, _luid).len() > 0 {
                1
            } else {
                0
            };
            decoding.ability_h265 |= if VRamDecoder::available(CodecFormat::H265, _luid).len() > 0 {
                1
            } else {
                0
            };
        }
        #[cfg(feature = "mediacodec")]
        if enable_hwcodec_option() {
            decoding.ability_h264 =
                if H264_DECODER_SUPPORT.load(std::sync::atomic::Ordering::SeqCst) {
                    1
                } else {
                    0
                };
            decoding.ability_h265 =
                if H265_DECODER_SUPPORT.load(std::sync::atomic::Ordering::SeqCst) {
                    1
                } else {
                    0
                };
        }
        for unsupported in mark_unsupported {
            match unsupported {
                CodecFormat::VP8 => decoding.ability_vp8 = 0,
                CodecFormat::VP9 => decoding.ability_vp9 = 0,
                CodecFormat::AV1 => decoding.ability_av1 = 0,
                CodecFormat::H264 => decoding.ability_h264 = 0,
                CodecFormat::H265 => decoding.ability_h265 = 0,
                _ => {}
            }
        }
        decoding
    }

    pub fn new(format: CodecFormat, _luid: Option<i64>) -> Decoder {
        log::info!("try create new decoder, format: {format:?}, _luid: {_luid:?}");
        let (mut vp8, mut vp9, mut av1) = (None, None, None);
        #[cfg(feature = "hwcodec")]
        let (mut h264_ram, mut h265_ram) = (None, None);
        #[cfg(feature = "vram")]
        let (mut h264_vram, mut h265_vram) = (None, None);
        #[cfg(feature = "mediacodec")]
        let (mut h264_media_codec, mut h265_media_codec) = (None, None);
        let mut valid = false;

        match format {
            CodecFormat::VP8 => {
                match VpxDecoder::new(VpxDecoderConfig {
                    codec: VpxVideoCodecId::VP8,
                }) {
                    Ok(v) => vp8 = Some(v),
                    Err(e) => log::error!("create VP8 decoder failed: {}", e),
                }
                valid = vp8.is_some();
            }
            CodecFormat::VP9 => {
                match VpxDecoder::new(VpxDecoderConfig {
                    codec: VpxVideoCodecId::VP9,
                }) {
                    Ok(v) => vp9 = Some(v),
                    Err(e) => log::error!("create VP9 decoder failed: {}", e),
                }
                valid = vp9.is_some();
            }
            CodecFormat::AV1 => {
                match AomDecoder::new() {
                    Ok(v) => av1 = Some(v),
                    Err(e) => log::error!("create AV1 decoder failed: {}", e),
                }
                valid = av1.is_some();
            }
            CodecFormat::H264 => {
                #[cfg(feature = "vram")]
                if !valid && enable_vram_option(false) && _luid.clone().unwrap_or_default() != 0 {
                    match VRamDecoder::new(format, _luid) {
                        Ok(v) => h264_vram = Some(v),
                        Err(e) => log::error!("create H264 vram decoder failed: {}", e),
                    }
                    valid = h264_vram.is_some();
                }
                #[cfg(feature = "hwcodec")]
                if !valid {
                    match HwRamDecoder::new(format) {
                        Ok(v) => h264_ram = Some(v),
                        Err(e) => log::error!("create H264 ram decoder failed: {}", e),
                    }
                    valid = h264_ram.is_some();
                }
                #[cfg(feature = "mediacodec")]
                if !valid && enable_hwcodec_option() {
                    h264_media_codec = MediaCodecDecoder::new(format);
                    if h264_media_codec.is_none() {
                        log::error!("create H264 media codec decoder failed");
                    }
                    valid = h264_media_codec.is_some();
                }
            }
            CodecFormat::H265 => {
                #[cfg(feature = "vram")]
                if !valid && enable_vram_option(false) && _luid.clone().unwrap_or_default() != 0 {
                    match VRamDecoder::new(format, _luid) {
                        Ok(v) => h265_vram = Some(v),
                        Err(e) => log::error!("create H265 vram decoder failed: {}", e),
                    }
                    valid = h265_vram.is_some();
                }
                #[cfg(feature = "hwcodec")]
                if !valid {
                    match HwRamDecoder::new(format) {
                        Ok(v) => h265_ram = Some(v),
                        Err(e) => log::error!("create H265 ram decoder failed: {}", e),
                    }
                    valid = h265_ram.is_some();
                }
                #[cfg(feature = "mediacodec")]
                if !valid && enable_hwcodec_option() {
                    h265_media_codec = MediaCodecDecoder::new(format);
                    if h265_media_codec.is_none() {
                        log::error!("create H265 media codec decoder failed");
                    }
                    valid = h265_media_codec.is_some();
                }
            }
            CodecFormat::Unknown => {
                log::error!("unknown codec format, cannot create decoder");
            }
        }
        if !valid {
            log::error!("failed to create {format:?} decoder");
        } else {
            log::info!("create {format:?} decoder success");
        }
        Decoder {
            vp8,
            vp9,
            av1,
            #[cfg(feature = "hwcodec")]
            h264_ram,
            #[cfg(feature = "hwcodec")]
            h265_ram,
            #[cfg(feature = "vram")]
            h264_vram,
            #[cfg(feature = "vram")]
            h265_vram,
            #[cfg(feature = "mediacodec")]
            h264_media_codec,
            #[cfg(feature = "mediacodec")]
            h265_media_codec,
            format,
            valid,
            #[cfg(feature = "hwcodec")]
            i420: vec![],
        }
    }

    pub fn format(&self) -> CodecFormat {
        self.format
    }

    pub fn valid(&self) -> bool {
        self.valid
    }
}
