use super::*;

// mac key input must be run in main thread, otherwise crash on >= osx 10.15
#[cfg(target_os = "macos")]
lazy_static::lazy_static! {
    pub(super) static ref QUEUE: Queue = Queue::main();
}

#[cfg(target_os = "macos")]
pub(super) struct VirtualInputState {
    virtual_input: VirtualInput,
    capslock_down: bool,
}

#[cfg(target_os = "macos")]
impl VirtualInputState {
    pub(super) fn new() -> Option<Self> {
        VirtualInput::new(
            CGEventSourceStateID::CombinedSessionState,
            // Note: `CGEventTapLocation::Session` will be affected by the mouse events.
            // When we're simulating key events, then move the physical mouse, the key events will be affected.
            // It looks like https://github.com/rustdesk/rustdesk/issues/9729#issuecomment-2432306822
            // 1. Press "Command" key in RustDesk
            // 2. Move the physical mouse
            // 3. Press "V" key in RustDesk
            // Then the controlled side just prints "v" instead of pasting.
            //
            // Changing `CGEventTapLocation::Session` to `CGEventTapLocation::HID` fixes it.
            // But we do not consider this as a bug, because it's not a common case,
            // we consider only RustDesk operates the controlled side.
            //
            // https://developer.apple.com/documentation/coregraphics/cgeventtaplocation/
            CGEventTapLocation::Session,
        )
        .map(|virtual_input| Self {
            virtual_input,
            capslock_down: false,
        })
        .ok()
    }

    #[inline]
    pub(super) fn simulate(&self, event_type: &EventType) -> ResultType<()> {
        Ok(self.virtual_input.simulate(&event_type)?)
    }
}

#[cfg(target_os = "macos")]
pub(super) static mut VIRTUAL_INPUT_MTX: Mutex<()> = Mutex::new(());
#[cfg(target_os = "macos")]
pub(super) static mut VIRTUAL_INPUT_STATE: Option<VirtualInputState> = None;

// First call set_uinput() will create keyboard and mouse clients.
// The clients are ipc connections that must live shorter than tokio runtime.
// Thus this function must not be called in a temporary runtime.
#[cfg(target_os = "linux")]
pub async fn setup_uinput(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    // Keyboard and mouse both open /dev/uinput
    // TODO: Make sure there's no race
    set_uinput_resolution(minx, maxx, miny, maxy).await?;

    let keyboard = super::uinput::client::UInputKeyboard::new().await?;
    log::info!("UInput keyboard created");
    let mouse = super::uinput::client::UInputMouse::new().await?;
    log::info!("UInput mouse created");

    let mut en = ENIGO.lock().unwrap();
    // enigo guessed x11 once at construction, which is what a Wayland greeter reads as, and
    // then routes the devices installed below to a null xdo that drops everything silently.
    // Reaching here means `wayland_use_uinput()` was true, so this states a fact.
    en.set_is_x11(false);
    // One lock for both, so there is no window where the keyboard is custom and the mouse is not.
    en.set_custom_keyboard(Box::new(keyboard));
    en.set_custom_mouse(Box::new(mouse));
    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn setup_rdp_input() -> ResultType<(), Box<dyn std::error::Error>> {
    let mut en = ENIGO.lock()?;
    // Same as `setup_uinput`: the caller is gated on `wayland_use_rdp_input()`.
    en.set_is_x11(false);
    let rdp_info_lock = RDP_SESSION_INFO.lock()?;
    let rdp_info = rdp_info_lock.as_ref().ok_or("RDP session is None")?;

    let keyboard = RdpInputKeyboard::new(rdp_info.conn.clone(), rdp_info.session.clone())?;
    en.set_custom_keyboard(Box::new(keyboard));
    log::info!("RdpInput keyboard created");

    if let Some(stream) = rdp_info.streams.clone().into_iter().next() {
        let resolution = rdp_info
            .resolution
            .lock()
            .unwrap()
            .unwrap_or(stream.get_size());
        let mouse = RdpInputMouse::new(
            rdp_info.conn.clone(),
            rdp_info.session.clone(),
            stream,
            resolution,
        )?;
        en.set_custom_mouse(Box::new(mouse));
        log::info!("RdpInput mouse created");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
pub async fn update_mouse_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    set_uinput_resolution(minx, maxx, miny, maxy).await?;

    // Confirm the device adopted the new range before the caller caches it.
    // spawn_blocking because ENIGO is a std Mutex and send_refresh blocks on IPC.
    tokio::task::spawn_blocking(move || {
        if let Some(mouse) = ENIGO.lock().unwrap().get_custom_mouse() {
            if let Some(mouse) = mouse
                .as_mut_any()
                .downcast_mut::<super::uinput::client::UInputMouse>()
            {
                return mouse.send_refresh();
            }
            bail!("failed to downcast custom mouse to UInputMouse");
        }
        // No custom mouse: nothing to refresh.
        Ok(())
    })
    .await?
}

#[cfg(target_os = "linux")]
pub(super) async fn set_uinput_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    super::uinput::client::set_resolution(minx, maxx, miny, maxy).await
}
