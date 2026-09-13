use super::*;

pub(super) struct CapturerPtr(pub(super) *mut Capturer);

impl Clone for CapturerPtr {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl TraitCapturer for CapturerPtr {
    fn frame<'a>(&'a mut self, timeout: std::time::Duration) -> std::io::Result<Frame<'a>> {
        unsafe { (*self.0).frame(timeout) }
    }
}

pub(super) struct CapDisplayInfo {
    pub(super) rects: Vec<((i32, i32), usize, usize)>,
    pub(super) displays: Vec<DisplayInfo>,
    pub(super) num: usize,
    pub(super) primary: usize,
    pub(super) current: usize,
    pub(super) capturer: CapturerPtr,
}
