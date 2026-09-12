use super::*;

pub struct VRamDecoder {
    pub(super) decoder: Decoder,
}

impl VRamDecoder {
    pub fn try_get(format: CodecFormat, luid: Option<i64>) -> Option<DecodeContext> {
        let v: Vec<_> = Self::available(format, luid);
        if v.len() > 0 {
            // prefer ffmpeg
            if let Some(ctx) = v.iter().find(|c| c.driver == Driver::FFMPEG) {
                return Some(ctx.clone());
            }
            Some(v[0].clone())
        } else {
            None
        }
    }

    pub fn available(format: CodecFormat, luid: Option<i64>) -> Vec<DecodeContext> {
        let luid = luid.unwrap_or_default();
        let data_format = match format {
            CodecFormat::H264 => DataFormat::H264,
            CodecFormat::H265 => DataFormat::H265,
            _ => return vec![],
        };
        crate::hwcodec::HwCodecConfig::get()
            .vram_decode
            .drain(..)
            .filter(|c| c.data_format == data_format && c.luid == luid && luid != 0)
            .collect()
    }

    pub fn possible_available_without_check() -> (bool, bool) {
        if !enable_vram_option(false) {
            return (false, false);
        }
        let v = crate::hwcodec::HwCodecConfig::get().vram_decode;
        (
            v.iter().any(|d| d.data_format == DataFormat::H264),
            v.iter().any(|d| d.data_format == DataFormat::H265),
        )
    }

    pub fn new(format: CodecFormat, luid: Option<i64>) -> ResultType<Self> {
        let ctx = Self::try_get(format, luid).ok_or(anyhow!("Failed to get decode context"))?;
        log::info!("try create vram decoder: {ctx:?}");
        match Decoder::new(ctx) {
            Ok(decoder) => Ok(Self { decoder }),
            Err(_) => {
                HwCodecConfig::clear(true, false);
                Err(anyhow!(format!(
                    "Failed to create decoder, format: {:?}",
                    format
                )))
            }
        }
    }
    pub fn decode<'a>(&'a mut self, data: &[u8]) -> ResultType<Vec<VRamDecoderImage<'a>>> {
        match self.decoder.decode(data) {
            Ok(v) => Ok(v.iter().map(|f| VRamDecoderImage { frame: f }).collect()),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

pub struct VRamDecoderImage<'a> {
    pub frame: &'a DecodeFrame,
}

impl VRamDecoderImage<'_> {}

pub(crate) fn check_available_vram() -> (Vec<FeatureContext>, Vec<DecodeContext>, String) {
    let d = DynamicContext {
        device: None,
        width: 1280,
        height: 720,
        kbitrate: 5000,
        framerate: 60,
        gop: MAX_GOP as _,
    };
    let encoders = encode::available(d);
    let decoders = decode::available();
    let available = Available {
        e: encoders.clone(),
        d: decoders.clone(),
    };
    (
        encoders,
        decoders,
        available.serialize().unwrap_or_default(),
    )
}
