pub use self::vpxcodec::*;
use base::message_proto::{video_frame, Chroma, VideoFrame};
use hbb_common::{bail, log, ResultType};
use std::{ffi::c_void, slice};

cfg_if! {
    if #[cfg(quartz)] {
        mod quartz;
        pub use self::quartz::*;
    } else if #[cfg(x11)] {
        cfg_if! {
            if #[cfg(feature="wayland")] {
                mod linux;
                mod wayland;
                mod x11;
                #[cfg(all(target_os = "linux", feature = "drm"))]
                pub mod drmtap_dl;
                #[cfg(all(target_os = "linux", feature = "drm"))]
                pub mod drm_reader;
                #[cfg(all(target_os = "linux", feature = "drm"))]
                pub mod drm_render;
                pub use self::linux::*;
                pub use self::wayland::set_map_err;
                pub use self::x11::PixelBuffer;
            } else {
                mod x11;
                pub use self::x11::*;
            }
        }
    } else if #[cfg(dxgi)] {
        mod dxgi;
        pub use self::dxgi::*;
    } else if #[cfg(target_os = "android")] {
        mod android;
        pub use self::android::*;
    }else {
        //TODO: Fallback implementation.
    }
}

pub mod codec;
pub mod convert;
#[cfg(feature = "hwcodec")]
pub mod hwcodec;
#[cfg(feature = "mediacodec")]
pub mod mediacodec;
pub mod vpxcodec;
#[cfg(feature = "vram")]
pub mod vram;
pub use self::convert::*;
pub const STRIDE_ALIGN: usize = 64; // commonly used in libvpx vpx_img_alloc caller
pub const HW_STRIDE_ALIGN: usize = 0; // recommended by av_frame_get_buffer

pub mod aom;
#[cfg(not(any(target_os = "ios")))]
pub mod camera;
pub mod record;
mod vpx;

mod image_types;
pub use image_types::*;
mod traits;
pub use traits::*;
mod frame;
pub use frame::*;

#[cfg(x11)]
#[inline]
pub fn is_x11() -> bool {
    base::platform::linux::is_x11_or_headless()
}

#[cfg(x11)]
#[inline]
pub fn is_cursor_embedded() -> bool {
    if is_x11() {
        x11::IS_CURSOR_EMBEDDED
    } else {
        false
    }
}

#[cfg(not(x11))]
#[inline]
pub fn is_cursor_embedded() -> bool {
    false
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodecName {
    VP8,
    VP9,
    AV1,
    H264RAM(String),
    H265RAM(String),
    H264VRAM,
    H265VRAM,
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum CodecFormat {
    VP8,
    VP9,
    AV1,
    H264,
    H265,
    Unknown,
}

impl From<&VideoFrame> for CodecFormat {
    fn from(it: &VideoFrame) -> Self {
        match it.union {
            Some(video_frame::Union::Vp8s(_)) => CodecFormat::VP8,
            Some(video_frame::Union::Vp9s(_)) => CodecFormat::VP9,
            Some(video_frame::Union::Av1s(_)) => CodecFormat::AV1,
            Some(video_frame::Union::H264s(_)) => CodecFormat::H264,
            Some(video_frame::Union::H265s(_)) => CodecFormat::H265,
            _ => CodecFormat::Unknown,
        }
    }
}

impl From<&video_frame::Union> for CodecFormat {
    fn from(it: &video_frame::Union) -> Self {
        match it {
            video_frame::Union::Vp8s(_) => CodecFormat::VP8,
            video_frame::Union::Vp9s(_) => CodecFormat::VP9,
            video_frame::Union::Av1s(_) => CodecFormat::AV1,
            video_frame::Union::H264s(_) => CodecFormat::H264,
            video_frame::Union::H265s(_) => CodecFormat::H265,
            _ => CodecFormat::Unknown,
        }
    }
}

impl From<&CodecName> for CodecFormat {
    fn from(value: &CodecName) -> Self {
        match value {
            CodecName::VP8 => Self::VP8,
            CodecName::VP9 => Self::VP9,
            CodecName::AV1 => Self::AV1,
            CodecName::H264RAM(_) | CodecName::H264VRAM => Self::H264,
            CodecName::H265RAM(_) | CodecName::H265VRAM => Self::H265,
        }
    }
}

impl ToString for CodecFormat {
    fn to_string(&self) -> String {
        match self {
            CodecFormat::VP8 => "VP8".into(),
            CodecFormat::VP9 => "VP9".into(),
            CodecFormat::AV1 => "AV1".into(),
            CodecFormat::H264 => "H264".into(),
            CodecFormat::H265 => "H265".into(),
            CodecFormat::Unknown => "Unknown".into(),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    FailedCall(String),
    BadPtr(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[macro_export]
macro_rules! generate_call_macro {
    ($func_name:ident, $allow_err:expr) => {
        macro_rules! $func_name {
            ($x:expr) => {{
                let result = unsafe { $x };
                let result_int = unsafe { std::mem::transmute::<_, i32>(result) };
                if result_int != 0 {
                    let message = format!(
                        "errcode={} {}:{}:{}:{}",
                        result_int,
                        module_path!(),
                        file!(),
                        line!(),
                        column!()
                    );
                    if $allow_err {
                        log::warn!("Failed to call {}, {}", stringify!($func_name), message);
                    } else {
                        return Err(crate::Error::FailedCall(message).into());
                    }
                }
                result
            }};
        }
    };
}

#[macro_export]
macro_rules! generate_call_ptr_macro {
    ($func_name:ident) => {
        macro_rules! $func_name {
            ($x:expr) => {{
                let result = unsafe { $x };
                let result_int = unsafe { std::mem::transmute::<_, isize>(result) };
                if result_int == 0 {
                    return Err(crate::Error::BadPtr(format!(
                        "errcode={} {}:{}:{}:{}",
                        result_int,
                        module_path!(),
                        file!(),
                        line!(),
                        column!()
                    ))
                    .into());
                }
                result
            }};
        }
    };
}

pub trait GoogleImage {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn stride(&self) -> Vec<i32>;
    fn planes(&self) -> Vec<*mut u8>;
    fn chroma(&self) -> Chroma;
    fn get_bytes_per_row(w: usize, fmt: ImageFormat, align: usize) -> usize {
        let bytes_per_pixel = match fmt {
            ImageFormat::Raw => 3,
            ImageFormat::ARGB | ImageFormat::ABGR => 4,
        };
        // https://github.com/lemenkov/libyuv/blob/6900494d90ae095d44405cd4cc3f346971fa69c9/source/convert_argb.cc#L128
        // https://github.com/lemenkov/libyuv/blob/6900494d90ae095d44405cd4cc3f346971fa69c9/source/convert_argb.cc#L129
        (w * bytes_per_pixel + align - 1) & !(align - 1)
    }
    // rgb [in/out] fmt and stride must be set in ImageRgb
    fn to(&self, rgb: &mut ImageRgb) {
        rgb.w = self.width();
        rgb.h = self.height();
        let bytes_per_row = Self::get_bytes_per_row(rgb.w, rgb.fmt, rgb.align());
        rgb.raw.resize(rgb.h * bytes_per_row, 0);
        let stride = self.stride();
        let planes = self.planes();
        unsafe {
            match (self.chroma(), rgb.fmt()) {
                (Chroma::I420, ImageFormat::Raw) => {
                    super::I420ToRAW(
                        planes[0],
                        stride[0],
                        planes[1],
                        stride[1],
                        planes[2],
                        stride[2],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                (Chroma::I420, ImageFormat::ARGB) => {
                    super::I420ToARGB(
                        planes[0],
                        stride[0],
                        planes[1],
                        stride[1],
                        planes[2],
                        stride[2],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                (Chroma::I420, ImageFormat::ABGR) => {
                    super::I420ToABGR(
                        planes[0],
                        stride[0],
                        planes[1],
                        stride[1],
                        planes[2],
                        stride[2],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                (Chroma::I444, ImageFormat::ARGB) => {
                    super::I444ToARGB(
                        planes[0],
                        stride[0],
                        planes[1],
                        stride[1],
                        planes[2],
                        stride[2],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                (Chroma::I444, ImageFormat::ABGR) => {
                    super::I444ToABGR(
                        planes[0],
                        stride[0],
                        planes[1],
                        stride[1],
                        planes[2],
                        stride[2],
                        rgb.raw.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                // (Chroma::I444, ImageFormat::Raw), new version libyuv have I444ToRAW
                _ => log::error!("unsupported pixfmt: {:?}", self.chroma()),
            }
        }
    }
    fn data(&self) -> (&[u8], &[u8], &[u8]) {
        unsafe {
            let stride = self.stride();
            let planes = self.planes();
            let h = (self.height() as usize + 1) & !1;
            let n = stride[0] as usize * h;
            let y = slice::from_raw_parts(planes[0], n);
            let n = stride[1] as usize * (h >> 1);
            let u = slice::from_raw_parts(planes[1], n);
            let v = slice::from_raw_parts(planes[2], n);
            (y, u, v)
        }
    }
}

#[cfg(target_os = "android")]
pub fn screen_size() -> (u16, u16, u16) {
    SCREEN_SIZE.lock().unwrap().clone()
}

#[cfg(target_os = "android")]
pub fn is_start() -> Option<bool> {
    android::is_start()
}
