use super::*;

#[derive(Default)]
pub(super) struct StateCursor {
    hcursor: u64,
    cursor_data: Arc<Message>,
    cached_cursor_data: HashMap<u64, Arc<Message>>,
}

impl super::service::Reset for StateCursor {
    fn reset(&mut self) {
        *self = Default::default();
        crate::platform::reset_input_cache();
        fix_key_down_timeout(true);
    }
}

pub(super) struct StatePos {
    cursor_pos: (i32, i32),
}

impl Default for StatePos {
    fn default() -> Self {
        Self {
            cursor_pos: (INVALID_CURSOR_POS, INVALID_CURSOR_POS),
        }
    }
}

impl super::service::Reset for StatePos {
    fn reset(&mut self) {
        self.cursor_pos = (INVALID_CURSOR_POS, INVALID_CURSOR_POS);
    }
}

impl StatePos {
    #[inline]
    pub(super) fn is_valid(&self) -> bool {
        self.cursor_pos.0 != INVALID_CURSOR_POS
    }

    #[inline]
    pub(super) fn is_moved(&self, x: i32, y: i32) -> bool {
        self.is_valid() && (self.cursor_pos.0 != x || self.cursor_pos.1 != y)
    }
}

#[derive(Default)]
pub(super) struct StateWindowFocus {
    display_idx: i32,
}

impl super::service::Reset for StateWindowFocus {
    fn reset(&mut self) {
        self.display_idx = INVALID_DISPLAY_IDX;
    }
}

impl StateWindowFocus {
    #[inline]
    pub(super) fn is_valid(&self) -> bool {
        self.display_idx != INVALID_DISPLAY_IDX
    }

    #[inline]
    pub(super) fn is_changed(&self, disp_idx: i32) -> bool {
        self.is_valid() && self.display_idx != disp_idx
    }
}

#[derive(Clone, Default)]
pub struct MouseCursorSub {
    inner: ConnInner,
    cached: HashMap<u64, Arc<Message>>,
}

impl From<ConnInner> for MouseCursorSub {
    fn from(inner: ConnInner) -> Self {
        Self {
            inner,
            cached: HashMap::new(),
        }
    }
}

impl Subscriber for MouseCursorSub {
    #[inline]
    fn id(&self) -> i32 {
        self.inner.id()
    }

    #[inline]
    fn send(&mut self, msg: Arc<Message>) {
        if let Some(message::Union::CursorData(cd)) = &msg.union {
            if let Some(msg) = self.cached.get(&cd.id) {
                self.inner.send(msg.clone());
            } else {
                self.inner.send(msg.clone());
                let mut tmp = Message::new();
                // only send id out, require client side cache also
                tmp.set_cursor_id(cd.id);
                self.cached.insert(cd.id, Arc::new(tmp));
            }
        } else {
            self.inner.send(msg);
        }
    }
}

pub const NAME_CURSOR: &'static str = "mouse_cursor";
pub const NAME_POS: &'static str = "mouse_pos";
pub const NAME_WINDOW_FOCUS: &'static str = "window_focus";
#[derive(Clone)]
pub struct MouseCursorService {
    pub sp: ServiceTmpl<MouseCursorSub>,
}

impl Deref for MouseCursorService {
    type Target = ServiceTmpl<MouseCursorSub>;

    fn deref(&self) -> &Self::Target {
        &self.sp
    }
}

impl DerefMut for MouseCursorService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sp
    }
}

impl MouseCursorService {
    pub fn new(name: String, need_snapshot: bool) -> Self {
        Self {
            sp: ServiceTmpl::<MouseCursorSub>::new(name, need_snapshot),
        }
    }
}

pub fn new_cursor() -> ServiceTmpl<MouseCursorSub> {
    let svc = MouseCursorService::new(NAME_CURSOR.to_owned(), true);
    ServiceTmpl::<MouseCursorSub>::repeat::<StateCursor, _, _>(&svc.clone(), 33, run_cursor);
    svc.sp
}

pub fn new_pos() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME_POS.to_owned(), false);
    GenericService::repeat::<StatePos, _, _>(&svc.clone(), 33, run_pos);
    svc.sp
}

pub fn new_window_focus() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME_WINDOW_FOCUS.to_owned(), false);
    GenericService::repeat::<StateWindowFocus, _, _>(&svc.clone(), 33, run_window_focus);
    svc.sp
}

#[inline]
pub(super) fn update_last_cursor_pos(x: i32, y: i32) {
    let mut lock = LATEST_SYS_CURSOR_POS.lock().unwrap();
    if lock.1 .0 != x || lock.1 .1 != y {
        (lock.0, lock.1) = (Some(Instant::now()), (x, y))
    }
}

pub(super) fn run_pos(sp: EmptyExtraFieldService, state: &mut StatePos) -> ResultType<()> {
    let (_, (x, y)) = *LATEST_SYS_CURSOR_POS.lock().unwrap();
    if x == INVALID_CURSOR_POS || y == INVALID_CURSOR_POS {
        return Ok(());
    }

    if state.is_moved(x, y) {
        let mut msg_out = Message::new();
        msg_out.set_cursor_position(CursorPosition {
            x,
            y,
            ..Default::default()
        });
        let exclude = {
            let now = get_time();
            let lock = LATEST_PEER_INPUT_CURSOR.lock().unwrap();
            if now - lock.time < 300 {
                lock.conn
            } else {
                0
            }
        };
        sp.send_without(msg_out, exclude);
    }
    state.cursor_pos = (x, y);

    sp.snapshot(|sps| {
        let mut msg_out = Message::new();
        msg_out.set_cursor_position(CursorPosition {
            x: state.cursor_pos.0,
            y: state.cursor_pos.1,
            ..Default::default()
        });
        sps.send(msg_out);
        Ok(())
    })?;
    Ok(())
}

pub(super) fn run_cursor(sp: MouseCursorService, state: &mut StateCursor) -> ResultType<()> {
    if let Some(hcursor) = crate::get_cursor()? {
        if hcursor != state.hcursor {
            let msg;
            // On the DRM path get_cursor_data() may return a snapshot whose id has advanced past the
            // requested `hcursor` (it returns the latest hardware cursor); file it in the cache AND
            // record state.hcursor under the id ACTUALLY served, so a later reappearance of that exact
            // shape dedupes correctly instead of being suppressed. Everything below is fully
            // gated on the drm feature, so the drm-off build stays byte-identical to upstream.
            #[cfg(all(target_os = "linux", feature = "drm"))]
            let mut drm_served_id = hcursor;
            if let Some(cached) = state.cached_cursor_data.get(&hcursor) {
                super::log::trace!("Cursor data cached, hcursor: {}", hcursor);
                msg = cached.clone();
            } else {
                let mut data = crate::get_cursor_data(hcursor)?;
                // File the shape under the id ACTUALLY served, not the one requested. Deliberately a
                // NEW name rather than shadowing `hcursor`: the insert below reads as the requested
                // id everywhere else in this function, and a cfg-gated shadow would make the two
                // builds disagree about what that line means.
                #[cfg(all(target_os = "linux", feature = "drm"))]
                let served_id = data.id;
                #[cfg(all(target_os = "linux", feature = "drm"))]
                {
                    drm_served_id = served_id;
                }
                #[cfg(all(target_os = "linux", feature = "drm"))]
                let cache_key = served_id;
                #[cfg(not(all(target_os = "linux", feature = "drm")))]
                let cache_key = hcursor;
                data.colors = hbb_common::compress::compress(&data.colors[..]).into();
                let mut tmp = Message::new();
                tmp.set_cursor_data(data);
                msg = Arc::new(tmp);
                // A DRM cursor id is derived from the shape's pixels plus geometry, so an animated
                // pointer mints a new id on every shape change and this map would grow for the life
                // of the service, each entry pinning a compressed cursor message. (Upstream's X11
                // ids come from a small set of XFixes serials, so the map is effectively bounded
                // there -- which is why the ceiling is gated and the stock build stays untouched.)
                // Past the ceiling, drop the map and start over: the next request for any evicted
                // shape just recompresses it, and the ceiling comfortably covers every static shape
                // plus a generous animation window.
                #[cfg(all(target_os = "linux", feature = "drm"))]
                {
                    const CURSOR_CACHE_MAX: usize = 64;
                    if state.cached_cursor_data.len() >= CURSOR_CACHE_MAX {
                        state.cached_cursor_data.clear();
                    }
                }
                state.cached_cursor_data.insert(cache_key, msg.clone());
                super::log::trace!("Cursor data updated, hcursor: {}", cache_key);
            }
            #[cfg(not(all(target_os = "linux", feature = "drm")))]
            {
                state.hcursor = hcursor;
            }
            #[cfg(all(target_os = "linux", feature = "drm"))]
            {
                state.hcursor = drm_served_id;
            }
            sp.send_shared(msg.clone());
            state.cursor_data = msg;
        }
    }
    sp.snapshot(|sps| {
        sps.send_shared(state.cursor_data.clone());
        Ok(())
    })?;
    Ok(())
}

pub(super) fn run_window_focus(sp: EmptyExtraFieldService, state: &mut StateWindowFocus) -> ResultType<()> {
    let displays = super::display_service::get_sync_displays();
    if displays.len() <= 1 {
        return Ok(());
    }
    let disp_idx = crate::get_focused_display(displays);
    if let Some(disp_idx) = disp_idx.map(|id| id as i32) {
        if state.is_changed(disp_idx) {
            let mut misc = Misc::new();
            misc.set_follow_current_display(disp_idx as i32);
            let mut msg_out = Message::new();
            msg_out.set_misc(misc);
            sp.send(msg_out);
        }
        state.display_idx = disp_idx;
    }
    Ok(())
}
