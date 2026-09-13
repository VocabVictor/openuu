#[cfg_attr(not(windows), allow(unused_imports))]
use super::*;

#[cfg(any(feature = "hwcodec", feature = "mediacodec"))]
pub fn enable_hwcodec_option() -> bool {
    use base::config::keys::OPTION_ENABLE_HWCODEC;

    if !cfg!(target_os = "ios") {
        return option2bool(
            OPTION_ENABLE_HWCODEC,
            &Config::get_option(OPTION_ENABLE_HWCODEC),
        );
    }
    false
}
#[cfg(feature = "vram")]
pub fn enable_vram_option(encode: bool) -> bool {
    use base::config::keys::OPTION_ENABLE_HWCODEC;

    if cfg!(windows) {
        let enable = option2bool(
            OPTION_ENABLE_HWCODEC,
            &Config::get_option(OPTION_ENABLE_HWCODEC),
        );
        if encode {
            enable && enable_directx_capture()
        } else {
            enable && allow_d3d_render()
        }
    } else {
        false
    }
}

#[cfg(windows)]
pub fn enable_directx_capture() -> bool {
    use base::config::keys::OPTION_ENABLE_DIRECTX_CAPTURE as OPTION;
    option2bool(OPTION, &Config::get_option(OPTION))
}

#[cfg(windows)]
pub fn allow_d3d_render() -> bool {
    use base::config::keys::OPTION_ALLOW_D3D_RENDER as OPTION;
    option2bool(OPTION, &hbb_common::config::LocalConfig::get_option(OPTION))
}

pub const BR_BEST: f32 = 1.5;
pub const BR_BALANCED: f32 = 0.67;
pub const BR_SPEED: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quality {
    Best,
    Balanced,
    Low,
    Custom(f32),
}

impl Default for Quality {
    fn default() -> Self {
        Self::Balanced
    }
}

impl Quality {
    pub fn is_custom(&self) -> bool {
        match self {
            Quality::Custom(_) => true,
            _ => false,
        }
    }

    pub fn ratio(&self) -> f32 {
        match self {
            Quality::Best => BR_BEST,
            Quality::Balanced => BR_BALANCED,
            Quality::Low => BR_SPEED,
            Quality::Custom(v) => *v,
        }
    }
}
