use super::*;

pub(super) struct XdoLib {
    pub(super) _lib: Library,
    pub(super) xdo_new: FnXdoNew,
    pub(super) xdo_new_with_opened_display: Option<FnXdoNewWithOpenedDisplay>,
    pub(super) xdo_free: FnXdoFree,
    pub(super) xdo_send_keysequence_window: FnXdoSendKeysequenceWindow,
    pub(super) xdo_send_keysequence_window_down: Option<FnXdoSendKeysequenceWindowDown>,
    pub(super) xdo_send_keysequence_window_up: Option<FnXdoSendKeysequenceWindowUp>,
    pub(super) xdo_enter_text_window: Option<FnXdoEnterTextWindow>,
    pub(super) xdo_click_window: Option<FnXdoClickWindow>,
    pub(super) xdo_mouse_down: Option<FnXdoMouseDown>,
    pub(super) xdo_mouse_up: Option<FnXdoMouseUp>,
    pub(super) xdo_move_mouse: Option<FnXdoMoveMouse>,
    pub(super) xdo_move_mouse_relative: Option<FnXdoMoveMouseRelative>,
    pub(super) xdo_move_mouse_relative_to_window: Option<FnXdoMoveMouseRelativeToWindow>,
    pub(super) xdo_get_mouse_location: Option<FnXdoGetMouseLocation>,
    pub(super) xdo_get_mouse_location2: Option<FnXdoGetMouseLocation2>,
    pub(super) xdo_get_active_window: Option<FnXdoGetActiveWindow>,
    pub(super) xdo_get_focused_window: Option<FnXdoGetFocusedWindow>,
    pub(super) xdo_get_focused_window_sane: Option<FnXdoGetFocusedWindowSane>,
    pub(super) xdo_get_window_location: Option<FnXdoGetWindowLocation>,
    pub(super) xdo_get_window_size: Option<FnXdoGetWindowSize>,
    pub(super) xdo_get_input_state: Option<FnXdoGetInputState>,
    pub(super) xdo_activate_window: Option<FnXdoActivateWindow>,
    pub(super) xdo_wait_for_mouse_move_from: Option<FnXdoWaitForMouseMoveFrom>,
    pub(super) xdo_wait_for_mouse_move_to: Option<FnXdoWaitForMouseMoveTo>,
    pub(super) xdo_set_window_class: Option<FnXdoSetWindowClass>,
    pub(super) xdo_search_windows: Option<FnXdoSearchWindows>,
}

impl XdoLib {
    pub(super) fn load() -> Option<Self> {
        // https://github.com/rustdesk/rustdesk/issues/13711
        const LIB_NAMES: [&str; 3] = ["libxdo.so.4", "libxdo.so.3", "libxdo.so"];

        unsafe {
            let (lib, lib_name) = LIB_NAMES
                .iter()
                .find_map(|name| Library::new(name).ok().map(|lib| (lib, *name)))?;

            log::info!("libxdo-sys Loaded {}", lib_name);

            let xdo_new: FnXdoNew = *lib.get(b"xdo_new").ok()?;
            let xdo_free: FnXdoFree = *lib.get(b"xdo_free").ok()?;
            let xdo_send_keysequence_window: FnXdoSendKeysequenceWindow =
                *lib.get(b"xdo_send_keysequence_window").ok()?;

            let xdo_new_with_opened_display = lib
                .get(b"xdo_new_with_opened_display")
                .ok()
                .map(|s: Symbol<FnXdoNewWithOpenedDisplay>| *s);
            let xdo_send_keysequence_window_down = lib
                .get(b"xdo_send_keysequence_window_down")
                .ok()
                .map(|s: Symbol<FnXdoSendKeysequenceWindowDown>| *s);
            let xdo_send_keysequence_window_up = lib
                .get(b"xdo_send_keysequence_window_up")
                .ok()
                .map(|s: Symbol<FnXdoSendKeysequenceWindowUp>| *s);
            let xdo_enter_text_window = lib
                .get(b"xdo_enter_text_window")
                .ok()
                .map(|s: Symbol<FnXdoEnterTextWindow>| *s);
            let xdo_click_window = lib
                .get(b"xdo_click_window")
                .ok()
                .map(|s: Symbol<FnXdoClickWindow>| *s);
            let xdo_mouse_down = lib
                .get(b"xdo_mouse_down")
                .ok()
                .map(|s: Symbol<FnXdoMouseDown>| *s);
            let xdo_mouse_up = lib
                .get(b"xdo_mouse_up")
                .ok()
                .map(|s: Symbol<FnXdoMouseUp>| *s);
            let xdo_move_mouse = lib
                .get(b"xdo_move_mouse")
                .ok()
                .map(|s: Symbol<FnXdoMoveMouse>| *s);
            let xdo_move_mouse_relative = lib
                .get(b"xdo_move_mouse_relative")
                .ok()
                .map(|s: Symbol<FnXdoMoveMouseRelative>| *s);
            let xdo_move_mouse_relative_to_window = lib
                .get(b"xdo_move_mouse_relative_to_window")
                .ok()
                .map(|s: Symbol<FnXdoMoveMouseRelativeToWindow>| *s);
            let xdo_get_mouse_location = lib
                .get(b"xdo_get_mouse_location")
                .ok()
                .map(|s: Symbol<FnXdoGetMouseLocation>| *s);
            let xdo_get_mouse_location2 = lib
                .get(b"xdo_get_mouse_location2")
                .ok()
                .map(|s: Symbol<FnXdoGetMouseLocation2>| *s);
            let xdo_get_active_window = lib
                .get(b"xdo_get_active_window")
                .ok()
                .map(|s: Symbol<FnXdoGetActiveWindow>| *s);
            let xdo_get_focused_window = lib
                .get(b"xdo_get_focused_window")
                .ok()
                .map(|s: Symbol<FnXdoGetFocusedWindow>| *s);
            let xdo_get_focused_window_sane = lib
                .get(b"xdo_get_focused_window_sane")
                .ok()
                .map(|s: Symbol<FnXdoGetFocusedWindowSane>| *s);
            let xdo_get_window_location = lib
                .get(b"xdo_get_window_location")
                .ok()
                .map(|s: Symbol<FnXdoGetWindowLocation>| *s);
            let xdo_get_window_size = lib
                .get(b"xdo_get_window_size")
                .ok()
                .map(|s: Symbol<FnXdoGetWindowSize>| *s);
            let xdo_get_input_state = lib
                .get(b"xdo_get_input_state")
                .ok()
                .map(|s: Symbol<FnXdoGetInputState>| *s);
            let xdo_activate_window = lib
                .get(b"xdo_activate_window")
                .ok()
                .map(|s: Symbol<FnXdoActivateWindow>| *s);
            let xdo_wait_for_mouse_move_from = lib
                .get(b"xdo_wait_for_mouse_move_from")
                .ok()
                .map(|s: Symbol<FnXdoWaitForMouseMoveFrom>| *s);
            let xdo_wait_for_mouse_move_to = lib
                .get(b"xdo_wait_for_mouse_move_to")
                .ok()
                .map(|s: Symbol<FnXdoWaitForMouseMoveTo>| *s);
            let xdo_set_window_class = lib
                .get(b"xdo_set_window_class")
                .ok()
                .map(|s: Symbol<FnXdoSetWindowClass>| *s);
            let xdo_search_windows = lib
                .get(b"xdo_search_windows")
                .ok()
                .map(|s: Symbol<FnXdoSearchWindows>| *s);

            Some(Self {
                _lib: lib,
                xdo_new,
                xdo_new_with_opened_display,
                xdo_free,
                xdo_send_keysequence_window,
                xdo_send_keysequence_window_down,
                xdo_send_keysequence_window_up,
                xdo_enter_text_window,
                xdo_click_window,
                xdo_mouse_down,
                xdo_mouse_up,
                xdo_move_mouse,
                xdo_move_mouse_relative,
                xdo_move_mouse_relative_to_window,
                xdo_get_mouse_location,
                xdo_get_mouse_location2,
                xdo_get_active_window,
                xdo_get_focused_window,
                xdo_get_focused_window_sane,
                xdo_get_window_location,
                xdo_get_window_size,
                xdo_get_input_state,
                xdo_activate_window,
                xdo_wait_for_mouse_move_from,
                xdo_wait_for_mouse_move_to,
                xdo_set_window_class,
                xdo_search_windows,
            })
        }
    }
}

pub(super) static XDO_LIB: OnceLock<Option<XdoLib>> = OnceLock::new();

pub(super) fn get_lib() -> Option<&'static XdoLib> {
    XDO_LIB
        .get_or_init(|| {
            let lib = XdoLib::load();
            if lib.is_none() {
                log::info!("libxdo-sys libxdo not found, xdo functions will be disabled");
            }
            lib
        })
        .as_ref()
}
