use super::*;

pub(super) struct FrameSlot {
    // Row stride is `pixels.len() / height`, possibly padded; the format is per frame.
    pub(super) latest: Option<(usize, usize, Pixfmt, Vec<u8>)>,
    // TWO slots: two buffers can be idle at once -- the receive path takes one and publishes in two
    // SEPARATE acquisitions, so the encoder can hand its borrow back in between.
    pub(super) free: [Option<Vec<u8>>; 2],
    pub(super) ended: Option<String>,
}

impl FrameSlot {
    pub(super) fn publish(&mut self, w: usize, h: usize, fmt: Pixfmt, buf: Vec<u8>) {
        if let Some((.., old)) = self.latest.take() {
            self.recycle(old);
        }
        self.latest = Some((w, h, fmt, buf));
    }

    pub(super) fn recycle(&mut self, buf: Vec<u8>) {
        if let Some(slot) = self.free.iter_mut().find(|s| s.is_none()) {
            *slot = Some(buf);
        }
    }

    pub(super) fn take_free(&mut self) -> Option<Vec<u8>> {
        self.free.iter_mut().find_map(|s| s.take())
    }
}

/// `Shared.transform` before new() stores the real value: a cursor arriving this early is held
/// back and replayed once the session transform is in, because the producer will not resend it
/// until the shape changes.
pub(super) const TRANSFORM_PENDING: i32 = i32::MIN;

pub(super) struct Shared {
    pub(super) slot: Mutex<FrameSlot>,
    pub(super) cv: Condvar,
    // Session transform, TRANSFORM_PENDING until new() stores it post-handshake; the receive
    // thread turns cursor bitmaps with it and defers any cursor that races the store.
    pub(super) transform: std::sync::atomic::AtomicI32,
}

pub struct IpcDrmCapturer {
    pub(super) shared: Arc<Shared>,
    pub(super) stop: Arc<AtomicBool>,
    pub(super) display: i32,
    pub(super) connector: Option<String>,
    // What the encoder was sized from: CapturerInfo{width,height} is read once, at build time.
    // With a rotated output these are the ROTATED dimensions, matching the frames delivered.
    pub(super) session_size: Option<(usize, usize)>,
    // Output rotation in degrees: a rotated scanout holds the desktop drawn sideways, so frames
    // are turned back before delivery. Fixed per session; a rotation rebuilds the capturer.
    pub(super) transform: i32,
    // The wayland snapshot generation this session was built from: a later invalidation means
    // the layout (a rotation included) may have changed, and frame() asks for a rebuild.
    pub(super) snapshot_gen: u64,
    pub(super) cur: Vec<u8>,
    pub(super) cur_w: usize,
    pub(super) cur_h: usize,
    pub(super) cur_fmt: Pixfmt,
    pub(super) got_frame: bool,
}
