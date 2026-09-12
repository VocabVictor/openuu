use super::*;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn change_grab_status(state: GrabState, keyboard_mode: &str, session_id: u128) {
    #[cfg(feature = "flutter")]
    if !IS_RDEV_ENABLED.load(Ordering::SeqCst) {
        return;
    }
    // Serialize transitions so a stale `Wait` from a previous owner cannot
    // clobber a fresh `Run` from a different session window.
    let mut release_after_unlock = None;
    #[cfg(target_os = "linux")]
    let mut run_grab_after_unlock = None;
    #[cfg(target_os = "linux")]
    let mut disable_after_unlock = false;
    let mut gs = GRAB_STATE.lock().unwrap();
    match state {
        GrabState::Ready => {}
        GrabState::Run => {
            #[cfg(windows)]
            update_grab_get_key_name(keyboard_mode);

            // Idempotent: if this session already owns the grab, just
            // refresh the debounce timer (proves the session is still
            // actively focused) and skip the actual grab call.
            if gs.owner == Some(session_id) {
                gs.last_grab = Some(std::time::Instant::now());
                // Reset so the next Wait can spawn a fresh deferred-release
                // timer with an up-to-date snapshot of last_grab.
                gs.deferred_pending = false;
                log::debug!(
                    "[grab] Run(0x{:x}): already owner, refresh debounce",
                    session_id
                );
                return;
            }

            log::debug!(
                "[grab] Run(0x{:x}): prev_owner={}, mode={}",
                session_id,
                gs.owner
                    .map_or("none".to_string(), |id| format!("0x{:x}", id)),
                keyboard_mode,
            );

            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
            KEYBOARD_HOOKED.store(true, Ordering::SeqCst);

            #[cfg(target_os = "linux")]
            let had_owner = gs.owner.is_some();
            gs.owner = Some(session_id);
            gs.last_grab = Some(std::time::Instant::now());
            // Invalidate any in-flight deferred release from the previous
            // owner so it cannot suppress a fresh timer for the new owner.
            gs.deferred_pending = false;
            #[cfg(target_os = "linux")]
            {
                run_grab_after_unlock = Some(had_owner);
            }
        }
        GrabState::Wait => {
            // Drop stale `Wait` events that do not correspond to the
            // current grab owner. This prevents a late PointerExit from
            // session A from releasing session B's freshly acquired grab.
            if gs.owner != Some(session_id) {
                log::debug!(
                    "[grab] Wait(0x{:x}): ignored, owner={}",
                    session_id,
                    gs.owner
                        .map_or("none".to_string(), |id| format!("0x{:x}", id)),
                );
                return;
            }

            // Debounce: on Linux/X11, XGrabKeyboard causes a focus-change
            // feedback loop (grab -> PointerExit -> ungrab -> PointerEnter ->
            // grab -> ...). Suppress Wait if the grab was acquired recently
            // by this same session -- it is X11 feedback, not a real leave.
            // A deferred release is scheduled so that a genuine leave within
            // the debounce window is not permanently lost.
            #[cfg(target_os = "linux")]
            if let Some(t) = gs.last_grab {
                let elapsed = t.elapsed().as_millis();
                if elapsed < GRAB_DEBOUNCE_MS {
                    if !gs.deferred_pending {
                        log::debug!(
                            "[grab] Wait(0x{:x}): debounced ({}ms < {}ms), scheduling deferred release",
                            session_id, elapsed, GRAB_DEBOUNCE_MS,
                        );
                        gs.deferred_pending = true;
                        let remaining = (GRAB_DEBOUNCE_MS - elapsed) as u64 + 50;
                        let snapshot = gs.last_grab;
                        let mode = keyboard_mode.to_string();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(remaining));
                            let release_keys = {
                                let mut gs = GRAB_STATE.lock().unwrap();
                                // Release only if no new Run has refreshed the grab since.
                                if gs.owner == Some(session_id) && gs.last_grab == snapshot {
                                    let to_release = take_remote_keys();
                                    gs.deferred_pending = false;
                                    log::debug!(
                                        "[grab] Wait(0x{:x}): deferred release",
                                        session_id
                                    );
                                    KEYBOARD_HOOKED.store(false, Ordering::SeqCst);
                                    gs.owner = None;
                                    gs.last_grab = None;
                                    Some(to_release)
                                } else {
                                    log::debug!(
                                        "[grab] Wait(0x{:x}): deferred release cancelled (grab refreshed)",
                                        session_id,
                                    );
                                    None
                                }
                            };
                            if let Some(to_release) = release_keys {
                                disable_grab_if_released();
                                release_remote_keys_for_events(&mode, to_release);
                            }
                        });
                    } else {
                        log::debug!(
                            "[grab] Wait(0x{:x}): debounced, deferred release already pending",
                            session_id,
                        );
                    }
                    return;
                }
            }

            log::debug!("[grab] Wait(0x{:x}): releasing grab", session_id);

            #[cfg(windows)]
            rdev::set_get_key_unicode(false);

            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
            KEYBOARD_HOOKED.store(false, Ordering::SeqCst);

            gs.owner = None;
            gs.last_grab = None;
            gs.deferred_pending = false;
            release_after_unlock = Some(take_remote_keys());
            #[cfg(target_os = "linux")]
            {
                disable_after_unlock = true;
            }
        }
        GrabState::Exit => {}
    }
    drop(gs);
    #[cfg(target_os = "linux")]
    {
        if disable_after_unlock {
            disable_grab_if_released();
        }
        if let Some(disable_first) = run_grab_after_unlock {
            apply_run_grab_if_owner(session_id, disable_first);
        }
    }
    if let Some(to_release) = release_after_unlock {
        release_remote_keys_for_events(keyboard_mode, to_release);
    }
}
