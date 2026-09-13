use super::*;

pub(super) fn handle_mouse(mouse: &mut mouce::UInputMouseManager, data: &DataMouse) {
    log::trace!("handle_mouse {:?}", &data);
    match data {
        DataMouse::MoveTo(x, y) => {
            allow_err!(mouse.move_to(*x as _, *y as _))
        }
        DataMouse::MoveRelative(x, y) => {
            allow_err!(mouse.move_relative(*x, *y))
        }
        DataMouse::Down(button) => {
            let btn = match button {
                enigo::MouseButton::Left => mouce::MouseButton::Left,
                enigo::MouseButton::Middle => mouce::MouseButton::Middle,
                enigo::MouseButton::Right => mouce::MouseButton::Right,
                _ => {
                    return;
                }
            };
            allow_err!(mouse.press_button(&btn))
        }
        DataMouse::Up(button) => {
            let btn = match button {
                enigo::MouseButton::Left => mouce::MouseButton::Left,
                enigo::MouseButton::Middle => mouce::MouseButton::Middle,
                enigo::MouseButton::Right => mouce::MouseButton::Right,
                _ => {
                    return;
                }
            };
            allow_err!(mouse.release_button(&btn))
        }
        DataMouse::Click(button) => {
            let btn = match button {
                enigo::MouseButton::Left => mouce::MouseButton::Left,
                enigo::MouseButton::Middle => mouce::MouseButton::Middle,
                enigo::MouseButton::Right => mouce::MouseButton::Right,
                _ => {
                    return;
                }
            };
            allow_err!(mouse.click_button(&btn))
        }
        DataMouse::ScrollX(_length) => {
            // TODO: not supported for now
        }
        DataMouse::ScrollY(length) => {
            let mut length = *length;

            let scroll = if length < 0 {
                mouce::ScrollDirection::Up
            } else {
                mouce::ScrollDirection::Down
            };

            if length < 0 {
                length = -length;
            }

            for _ in 0..length {
                allow_err!(mouse.scroll_wheel(&scroll))
            }
        }
        DataMouse::Refresh => {
            // unreachable!()
        }
    }
}

pub(super) fn spawn_keyboard_handler(mut stream: Connection) {
    log::debug!("spawn_keyboard_handler: new keyboard handler connection");
    tokio::spawn(async move {
        let mut keyboard = match create_uinput_keyboard() {
            Ok(keyboard) => {
                log::debug!("UInput keyboard device created successfully");
                keyboard
            }
            Err(e) => {
                log::error!("Failed to create keyboard {}", e);
                return;
            }
        };
        loop {
            tokio::select! {
                res = stream.next() => {
                    match res {
                        Err(err) => {
                            log::info!("UInput keyboard ipc connection closed: {}", err);
                            break;
                        }
                        Ok(Some(data)) => {
                            match data {
                                Data::Keyboard(data) => {
                                    handle_keyboard(&mut stream, &mut keyboard, &data).await;
                                }
                                _ => {
                                    log::warn!("Unexpected data type in keyboard handler");
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

pub(super) fn spawn_mouse_handler(mut stream: ipc::Connection) {
    let resolution = RESOLUTION.lock().unwrap();
    if resolution.0 .0 == resolution.0 .1 || resolution.1 .0 == resolution.1 .1 {
        return;
    }
    let rng_x = resolution.0.clone();
    let rng_y = resolution.1.clone();
    tokio::spawn(async move {
        log::info!(
            "Create uinput mouce with rng_x: ({}, {}), rng_y: ({}, {})",
            rng_x.0,
            rng_x.1,
            rng_y.0,
            rng_y.1
        );
        let mut mouse = match mouce::UInputMouseManager::new(rng_x, rng_y) {
            Ok(mouse) => mouse,
            Err(e) => {
                log::error!("Failed to create mouse, {}", e);
                return;
            }
        };
        loop {
            tokio::select! {
                res = stream.next() => {
                    match res {
                        Err(err) => {
                            log::info!("UInput mouse ipc connection closed: {}", err);
                            break;
                        }
                        Ok(Some(data)) => {
                            match data {
                                Data::Mouse(data) => {
                                    if let DataMouse::Refresh = data {
                                        let (rng_x, rng_y) = {
                                            let resolution = RESOLUTION.lock().unwrap();
                                            (resolution.0.clone(), resolution.1.clone())
                                        };
                                        log::info!(
                                            "Refresh uinput mouce with rng_x: ({}, {}), rng_y: ({}, {})",
                                            rng_x.0,
                                            rng_x.1,
                                            rng_y.0,
                                            rng_y.1
                                        );
                                        match mouce::UInputMouseManager::new(rng_x, rng_y) {
                                            Ok(m) => {
                                                mouse = m;
                                                // Ack: device adopted the new range.
                                                allow_err!(stream.send(&Data::Empty).await);
                                            }
                                            Err(e) => {
                                                // Keep the current device; withhold the ack
                                                // so the client times out and retries.
                                                log::error!(
                                                    "Failed to recreate uinput mouse, keeping current: {}",
                                                    e
                                                );
                                            }
                                        }
                                    } else {
                                        handle_mouse(&mut mouse, &data);
                                    }
                                }
                                _ => {
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

pub(super) fn spawn_controller_handler(mut stream: ipc::Connection) {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                res = stream.next() => {
                    match res {
                        Err(_err) => {
                            // log::info!("UInput controller ipc connection closed: {}", err);
                            break;
                        }
                        Ok(Some(data)) => {
                            match data {
                                Data::Control(data) => match data {
                                    ipc::DataControl::Resolution{
                                        minx,
                                        maxx,
                                        miny,
                                        maxy,
                                    } => {
                                        *RESOLUTION.lock().unwrap() = ((minx, maxx), (miny, maxy));
                                        allow_err!(stream.send(&Data::Empty).await);
                                    }
                                }
                                _ => {
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    });
}

#[cfg(target_os = "linux")]
pub(super) fn authorize_uinput_peer(postfix: &str, stream: &RawIpcConnection) -> bool {
    if !hbb_common::config::is_service_ipc_postfix(postfix) {
        return true;
    }
    let peer_uid = ipc::peer_uid_from_fd(stream.as_raw_fd());
    let active_uid = crate::platform::linux::get_active_userid_fresh()
        .trim()
        .parse::<u32>()
        .ok();
    let authorized =
        peer_uid.is_some_and(|uid| ipc::is_allowed_service_peer_uid(uid, active_uid));
    if !authorized {
        crate::ipc::log_rejected_uinput_connection(postfix, peer_uid, active_uid);
        return false;
    }
    if let Err(err) =
        ipc::ensure_peer_executable_matches_current_by_fd(stream.as_raw_fd(), postfix)
    {
        log::warn!(
            "Rejected connection on protected uinput ipc channel due to executable mismatch: postfix={}, err={}",
            postfix,
            err
        );
        return false;
    }
    true
}
