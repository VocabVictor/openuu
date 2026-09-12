use super::*;

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
