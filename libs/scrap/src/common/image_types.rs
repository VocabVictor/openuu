use super::*;

#[repr(usize)]
#[derive(Debug, Copy, Clone)]
pub enum ImageFormat {
    Raw,
    ABGR,
    ARGB,
}

#[repr(C)]
#[derive(Clone)]
pub struct ImageRgb {
    pub raw: Vec<u8>,
    pub w: usize,
    pub h: usize,
    pub fmt: ImageFormat,
    pub align: usize,
}

impl ImageRgb {
    pub fn new(fmt: ImageFormat, align: usize) -> Self {
        Self {
            raw: Vec::new(),
            w: 0,
            h: 0,
            fmt,
            align,
        }
    }

    #[inline]
    pub fn fmt(&self) -> ImageFormat {
        self.fmt
    }

    #[inline]
    pub fn align(&self) -> usize {
        self.align
    }

    #[inline]
    pub fn set_align(&mut self, align: usize) {
        self.align = align;
    }
}

pub struct ImageTexture {
    pub texture: *mut c_void,
    pub w: usize,
    pub h: usize,
}

impl Default for ImageTexture {
    fn default() -> Self {
        Self {
            texture: std::ptr::null_mut(),
            w: 0,
            h: 0,
        }
    }
}
