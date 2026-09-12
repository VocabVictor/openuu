use super::*;

#[inline]
pub fn would_block_if_equal(old: &mut Vec<u8>, b: &[u8]) -> std::io::Result<()> {
    // does this really help?
    if b == &old[..] {
        return Err(std::io::ErrorKind::WouldBlock.into());
    }
    old.resize(b.len(), 0);
    old.copy_from_slice(b);
    Ok(())
}

pub trait TraitCapturer {
    // We doesn't support
    #[cfg(not(any(target_os = "ios")))]
    fn frame<'a>(&'a mut self, timeout: std::time::Duration) -> std::io::Result<Frame<'a>>;

    #[cfg(windows)]
    fn is_gdi(&self) -> bool;
    #[cfg(windows)]
    fn set_gdi(&mut self) -> bool;

    #[cfg(feature = "vram")]
    fn device(&self) -> AdapterDevice;

    #[cfg(feature = "vram")]
    fn set_output_texture(&mut self, texture: bool);
}

#[derive(Debug, Clone, Copy)]
pub struct AdapterDevice {
    pub device: *mut c_void,
    pub vendor_id: ::std::os::raw::c_uint,
    pub luid: i64,
}

impl Default for AdapterDevice {
    fn default() -> Self {
        Self {
            device: std::ptr::null_mut(),
            vendor_id: Default::default(),
            luid: Default::default(),
        }
    }
}

pub trait TraitPixelBuffer {
    fn data(&self) -> &[u8];

    fn width(&self) -> usize;

    fn height(&self) -> usize;

    fn stride(&self) -> Vec<usize>;

    fn pixfmt(&self) -> Pixfmt;
}
