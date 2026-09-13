use super::*;

impl Encoder {
    pub fn update(update: EncodingUpdate) {
        log::info!("update:{:?}", update);
        let mut decodings = PEER_DECODINGS.lock().unwrap();
        match update {
            EncodingUpdate::Update(id, decoding) => {
                decodings.insert(id, decoding);
            }
            EncodingUpdate::Remove(id) => {
                decodings.remove(&id);
            }
            EncodingUpdate::NewOnlyVP9(id) => {
                decodings.insert(
                    id,
                    SupportedDecoding {
                        ability_vp9: 1,
                        ..Default::default()
                    },
                );
            }
            EncodingUpdate::Check => {}
        }

        let vp8_useable = decodings.len() > 0 && decodings.iter().all(|(_, s)| s.ability_vp8 > 0);
        let av1_useable = decodings.len() > 0
            && decodings.iter().all(|(_, s)| s.ability_av1 > 0)
            && !disable_av1();
        let _all_support_h264_decoding =
            decodings.len() > 0 && decodings.iter().all(|(_, s)| s.ability_h264 > 0);
        let _all_support_h265_decoding =
            decodings.len() > 0 && decodings.iter().all(|(_, s)| s.ability_h265 > 0);
        #[allow(unused_mut)]
        let mut h264vram_encoding = false;
        #[allow(unused_mut)]
        let mut h265vram_encoding = false;
        #[cfg(feature = "vram")]
        if enable_vram_option(true) {
            if _all_support_h264_decoding {
                if VRamEncoder::available(CodecFormat::H264).len() > 0 {
                    h264vram_encoding = true;
                }
            }
            if _all_support_h265_decoding {
                if VRamEncoder::available(CodecFormat::H265).len() > 0 {
                    h265vram_encoding = true;
                }
            }
        }
        #[allow(unused_mut)]
        let mut h264hw_encoding: Option<String> = None;
        #[allow(unused_mut)]
        let mut h265hw_encoding: Option<String> = None;
        #[cfg(feature = "hwcodec")]
        if enable_hwcodec_option() {
            if _all_support_h264_decoding {
                h264hw_encoding =
                    HwRamEncoder::try_get(CodecFormat::H264).map_or(None, |c| Some(c.name));
            }
            if _all_support_h265_decoding {
                h265hw_encoding =
                    HwRamEncoder::try_get(CodecFormat::H265).map_or(None, |c| Some(c.name));
            }
        }
        let h264_useable =
            _all_support_h264_decoding && (h264vram_encoding || h264hw_encoding.is_some());
        let h265_useable =
            _all_support_h265_decoding && (h265vram_encoding || h265hw_encoding.is_some());
        let mut format = ENCODE_CODEC_FORMAT.lock().unwrap();
        let preferences: Vec<_> = decodings
            .iter()
            .filter(|(_, s)| {
                s.prefer == PreferCodec::VP9.into()
                    || s.prefer == PreferCodec::VP8.into() && vp8_useable
                    || s.prefer == PreferCodec::AV1.into() && av1_useable
                    || s.prefer == PreferCodec::H264.into() && h264_useable
                    || s.prefer == PreferCodec::H265.into() && h265_useable
            })
            .map(|(_, s)| s.prefer)
            .collect();
        *USABLE_ENCODING.lock().unwrap() = Some(SupportedEncoding {
            vp8: vp8_useable,
            av1: av1_useable,
            h264: h264_useable,
            h265: h265_useable,
            ..Default::default()
        });
        // find the most frequent preference
        let mut counts = Vec::new();
        for pref in &preferences {
            match counts.iter_mut().find(|(p, _)| p == pref) {
                Some((_, count)) => *count += 1,
                None => counts.push((pref.clone(), 1)),
            }
        }
        let max_count = counts.iter().map(|(_, count)| *count).max().unwrap_or(0);
        let (most_frequent, _) = counts
            .into_iter()
            .find(|(_, count)| *count == max_count)
            .unwrap_or((PreferCodec::Auto.into(), 0));
        let preference = most_frequent.enum_value_or(PreferCodec::Auto);

        // auto: h265 > h264 > av1 (when affordable) > vp9 > vp8 (auto_codec.rs)
        let av1_test = Config::get_option(base::config::keys::OPTION_AV1_TEST) != "N";
        let mut system = System::new();
        system.refresh_memory();
        let auto_codec = auto_codec::choose_auto_codec(&auto_codec::AutoCodecInputs {
            vp8_useable,
            av1_useable: av1_useable && av1_test,
            h264_useable,
            h265_useable,
            hw_encoder_available: h264vram_encoding
                || h265vram_encoding
                || h264hw_encoding.is_some()
                || h265hw_encoding.is_some(),
            logical_cores: num_cpus::get(),
            total_memory: system.total_memory(),
        });

        *format = match preference {
            PreferCodec::VP8 => CodecFormat::VP8,
            PreferCodec::VP9 => CodecFormat::VP9,
            PreferCodec::AV1 => CodecFormat::AV1,
            PreferCodec::H264 => {
                if h264vram_encoding || h264hw_encoding.is_some() {
                    CodecFormat::H264
                } else {
                    auto_codec
                }
            }
            PreferCodec::H265 => {
                if h265vram_encoding || h265hw_encoding.is_some() {
                    CodecFormat::H265
                } else {
                    auto_codec
                }
            }
            PreferCodec::Auto => auto_codec,
        };
        if decodings.len() > 0 {
            log::info!(
                "usable: vp8={vp8_useable}, av1={av1_useable}, h264={h264_useable}, h265={h265_useable}",
            );
            log::info!(
                "connection count: {}, used preference: {:?}, encoder: {:?}",
                decodings.len(),
                preference,
                *format
            )
        }
    }
}
