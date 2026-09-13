use super::*;

#[tokio::main(flavor = "current_thread")]
pub(in crate::server) async fn ensure_inited() -> ResultType<()> {
    // DRM/KMS capture (opt-in): the root service owns the reader and the capturer self-inits over
    // IPC, so there is no PipeWire recorder to initialize here. But we still must set the uinput
    // desktop rect (check_init does this on the PipeWire path, and the DRM path skips check_init).
    #[cfg(feature = "drm")]
    if super::super::drm_capturer::is_available_cached() {
        update_uinput_resolution().await;
        return Ok(());
    }
    check_init().await
}

pub(in crate::server) fn is_inited() -> Option<Message> {
    if is_x11() {
        None
    } else {
        #[cfg(feature = "drm")]
        if super::super::drm_capturer::is_available_cached() {
            return None;
        }
        if CAP_DISPLAY_INFO.read().unwrap().is_empty() {
            let mut msg_out = Message::new();
            let res = MessageBox {
                msgtype: "nook-nocancel-hasclose".to_owned(),
                title: "Wayland".to_owned(),
                text: "Please Select the screen to be shared(Operate on the peer side).".to_owned(),
                link: "".to_owned(),
                ..Default::default()
            };
            msg_out.set_message_box(res);
            Some(msg_out)
        } else {
            None
        }
    }
}

pub(in crate::server) async fn check_init() -> ResultType<()> {
    if !is_x11() {
        if CAP_DISPLAY_INFO.read().unwrap().is_empty() {
            if crate::input_service::wayland_use_uinput() {
                // The cached layout may predate compositor changes made while no session
                // was active, https://github.com/rustdesk/rustdesk/issues/15601
                scrap::wayland::display::clear_wayland_displays_cache();
                if let Some((minx, maxx, miny, maxy)) =
                    scrap::wayland::display::get_desktop_rect_for_uinput()
                {
                    log::info!(
                        "update mouse resolution: ({}, {}), ({}, {})",
                        minx,
                        maxx,
                        miny,
                        maxy
                    );
                    // Bound the IPC wait like the periodic refresh does, so a hung
                    // response can't stall session init.
                    match timeout(
                        3_000,
                        input_service::update_mouse_resolution(minx, maxx, miny, maxy),
                    )
                    .await
                    {
                        Ok(Ok(())) => {
                            super::super::display_service::set_wayland_uinput_rect((
                                minx, maxx, miny, maxy,
                            ));
                            // Snapshot the per-display layout the client's coordinates
                            // will be based on, so the mouse path can correct them if
                            // the compositor moves a monitor mid-session.
                            super::super::display_service::set_wayland_layout_baseline(
                                scrap::wayland::display::get_display_rects_for_uinput(),
                            );
                        }
                        Ok(Err(err)) => log::error!("Failed to update mouse resolution: {}", err),
                        Err(err) => log::error!("Failed to update mouse resolution: {}", err),
                    }
                } else {
                    log::warn!("Failed to get desktop rect for uinput");
                }
            }

            let mut lock = CAP_DISPLAY_INFO.write().unwrap();
            if lock.is_empty() {
                // Check if PipeWire is already initialized to prevent duplicate recorder creation
                if *PIPEWIRE_INITIALIZED.read().unwrap() {
                    log::warn!("wayland_diag: Preventing duplicate PipeWire initialization");
                    return Ok(());
                }

                let mut all = Display::all()?;
                log::debug!("Initializing displays with fill_displays()");
                {
                    let temp_mouse_move_handle = input_service::TemporaryMouseMoveHandle::new();
                    let move_mouse_to = |x, y| temp_mouse_move_handle.move_mouse_to(x, y);
                    fill_displays(move_mouse_to, crate::get_cursor_pos, &mut all)
                        .map_err(map_staged_err)?;
                }
                log::debug!("Attempting to fix logical size with try_fix_logical_size()");
                try_fix_logical_size(&mut all);
                *PIPEWIRE_INITIALIZED.write().unwrap() = true;
                let num = all.len();
                let primary = super::super::display_service::get_primary_2(&all);
                let mut displays = super::super::display_service::update_sync_displays(&all);
                for display in displays.iter_mut() {
                    display.cursor_embedded = is_cursor_embedded();
                }

                let mut rects: Vec<((i32, i32), usize, usize)> = Vec::new();
                for d in &all {
                    rects.push((d.origin(), d.width(), d.height()));
                }

                log::debug!(
                    "#displays={}, primary={}, rects: {:?}, cpus={}/{}",
                    num,
                    primary,
                    rects,
                    num_cpus::get_physical(),
                    num_cpus::get()
                );

                // Create individual CapDisplayInfo for each display with its own capturer
                for (idx, display) in all.into_iter().enumerate() {
                    // No `with_context` here: the peer is shown `format!("{}", err)`, which
                    // renders only the outermost layer, and the mapped reason is the inner one.
                    let capturer = Box::into_raw(Box::new(Capturer::new(display)?));
                    let capturer = CapturerPtr(capturer);

                    let cap_display_info = Box::into_raw(Box::new(CapDisplayInfo {
                        rects: rects.clone(),
                        displays: displays.clone(),
                        num,
                        primary,
                        current: idx,
                        capturer,
                    }));

                    lock.insert(idx, cap_display_info as u64);
                }
            }
        }
    }
    Ok(())
}
