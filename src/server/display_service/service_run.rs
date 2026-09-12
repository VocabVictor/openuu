use super::*;

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), run);
    svc.sp
}

pub(super) fn displays_to_msg(displays: Vec<DisplayInfo>) -> Message {
    let mut pi = PeerInfo {
        ..Default::default()
    };
    pi.displays = displays.clone();

    #[cfg(windows)]
    if crate::platform::is_installed() {
        let m = crate::virtual_display_manager::get_platform_additions();
        pi.platform_additions = serde_json::to_string(&m).unwrap_or_default();
    }

    // current_display should not be used in server.
    // It is set to 0 for compatibility with old clients.
    pi.current_display = 0;
    let mut msg_out = Message::new();
    msg_out.set_peer_info(pi);
    msg_out
}

pub(super) fn check_get_displays_changed_msg() -> Option<Message> {
    #[cfg(target_os = "linux")]
    {
        if !is_x11() {
            // On the DRM/KMS capture path the PipeWire enumeration (which is what feeds
            // `SYNC_DISPLAYS` via `check_update_displays`) is bypassed, so populate the sync list
            // from the DRM display list here. Without this the display service broadcasts an empty
            // list that overwrites the login peer-info displays and the client shows "No displays".
            #[cfg(feature = "drm")]
            if super::drm_capturer::is_available_cached() {
                let synced = !SYNC_DISPLAYS.lock().unwrap().displays.is_empty();
                let stamped_before = scrap::wayland::display::wayland_failure_stamped();
                // With nothing published yet, even the unaugmented DRM list beats the empty
                // broadcast below; with a synced layout, a suppressed turn keeps it instead.
                if !synced || !scrap::wayland::display::wayland_lookup_suppressed() {
                    if let Some(displays) = super::drm_capturer::get_display_infos() {
                        // A first failure keeps the synced layout for one backoff; only a
                        // failure that persists across one replaces it with the DRM stack.
                        if !synced
                            || stamped_before
                            || !scrap::wayland::display::wayland_lookup_suppressed()
                        {
                            SYNC_DISPLAYS.lock().unwrap().check_changed(&displays);
                        }
                    }
                }
            }
            return get_displays_msg();
        }
    }
    check_update_displays(&try_get_displays().ok()?);
    get_displays_msg()
}

pub fn check_displays_changed() -> ResultType<()> {
    #[cfg(target_os = "linux")]
    {
        // Currently, wayland need to call wayland::clear() before call Display::all(), otherwise it will cause
        // block, or even crash here, https://github.com/rustdesk/rustdesk/blob/0bb4d43e9ea9d9dfb9c46c8d27d1a97cd0ad6bea/libs/scrap/src/wayland/pipewire.rs#L235
        if !is_x11() {
            return Ok(());
        }
    }
    check_update_displays(&try_get_displays()?);
    Ok(())
}

pub(super) fn get_displays_msg() -> Option<Message> {
    let displays = SYNC_DISPLAYS.lock().unwrap().get_update_sync_displays()?;
    Some(displays_to_msg(displays))
}

pub(super) fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    while sp.ok() {
        sp.snapshot(|sps| {
            if !TEMP_IGNORE_DISPLAYS_CHANGED.load(Ordering::Relaxed) {
                if sps.has_subscribes() {
                    SYNC_DISPLAYS.lock().unwrap().is_synced = false;
                    bail!("new subscriber");
                }
            }
            Ok(())
        })?;

        if let Some(msg_out) = check_get_displays_changed_msg() {
            sp.send(msg_out);
            log::info!("Displays changed");
        }

        #[cfg(target_os = "linux")]
        if sp.has_subscribes() {
            refresh_wayland_uinput_rect_if_changed();
        }

        std::thread::sleep(Duration::from_millis(300));
    }

    Ok(())
}
