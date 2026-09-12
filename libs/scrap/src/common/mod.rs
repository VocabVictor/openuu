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
mod platform_codec;
pub use platform_codec::*;
mod google_image;
pub use google_image::*;

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

#[cfg(target_os = "android")]
pub fn screen_size() -> (u16, u16, u16) {
    SCREEN_SIZE.lock().unwrap().clone()
}

#[cfg(target_os = "android")]
pub fn is_start() -> Option<bool> {
    android::is_start()
}
