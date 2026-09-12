use super::*;

pub(super) fn run_capture(shmem: Arc<SharedMemory>) {
    let mut c = None;
    let mut last_current_display = usize::MAX;
    let mut last_timeout_ms: i32 = 33;
    let mut spf = Duration::from_millis(last_timeout_ms as _);
    let mut first_frame_captured = false;
    let mut dxgi_failed_times = 0;
    let mut display_width = 0;
    let mut display_height = 0;
    loop {
        if EXIT.lock().unwrap().clone() {
            break;
        }
        unsafe {
            let para_ptr = shmem.as_ptr().add(ADDR_CAPTURER_PARA);
            let para = para_ptr as *const CapturerPara;
            let recreate = (*para).recreate;
            let current_display = (*para).current_display;
            let timeout_ms = (*para).timeout_ms;
            if c.is_none() {
                let Ok(mut displays) = display_service::try_get_displays() else {
                    log::error!("Failed to get displays");
                    *EXIT.lock().unwrap() = true;
                    return;
                };
                if displays.len() <= current_display {
                    log::error!("Invalid display index:{}", current_display);
                    *EXIT.lock().unwrap() = true;
                    return;
                }
                let display = displays.remove(current_display);
                display_width = display.width();
                display_height = display.height();
                match Capturer::new(display) {
                    Ok(mut v) => {
                        c = {
                            last_current_display = current_display;
                            first_frame_captured = false;
                            if dxgi_failed_times > MAX_DXGI_FAIL_TIME {
                                dxgi_failed_times = 0;
                                v.set_gdi();
                            }
                            utils::set_para(
                                &shmem,
                                CapturerPara {
                                    recreate: false,
                                    current_display: (*para).current_display,
                                    timeout_ms: (*para).timeout_ms,
                                },
                            );
                            Some(v)
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to create gdi capturer: {:?}", e);
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        continue;
                    }
                }
            } else {
                if recreate || current_display != last_current_display {
                    log::info!(
                        "create capturer, display: {} -> {}",
                        last_current_display,
                        current_display,
                    );
                    c = None;
                    continue;
                }
                if timeout_ms != last_timeout_ms
                    && timeout_ms >= 1000 / video_qos::MAX_FPS as i32
                    && timeout_ms <= 1000 / video_qos::MIN_FPS as i32
                {
                    last_timeout_ms = timeout_ms;
                    spf = Duration::from_millis(timeout_ms as _);
                }
            }
            if first_frame_captured {
                if !utils::counter_equal(shmem.as_ptr().add(ADDR_CAPTURE_FRAME_COUNTER)) {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
            }
            match c.as_mut().map(|f| f.frame(spf)) {
                Some(Ok(f)) => match f {
                    Frame::PixelBuffer(f) => {
                        let frame_capacity = shmem.len().saturating_sub(ADDR_CAPTURE_FRAME);
                        if f.data().len() > frame_capacity {
                            log::error!(
                                "Portable service capture frame exceeds shared memory capacity: frame_len={}, capacity={}, shmem_len={}",
                                f.data().len(),
                                frame_capacity,
                                shmem.len()
                            );
                            *EXIT.lock().unwrap() = true;
                            return;
                        }
                        utils::set_frame_info(
                            &shmem,
                            FrameInfo {
                                length: f.data().len(),
                                width: display_width,
                                height: display_height,
                            },
                        );
                        shmem.write(ADDR_CAPTURE_FRAME, f.data());
                        shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                        utils::increase_counter(shmem.as_ptr().add(ADDR_CAPTURE_FRAME_COUNTER));
                        first_frame_captured = true;
                        dxgi_failed_times = 0;
                    }
                    Frame::Texture(_) => {
                        // should not happen
                    }
                },
                Some(Err(e)) => {
                    if crate::platform::windows::desktop_changed() {
                        crate::platform::try_change_desktop();
                        c = None;
                        std::thread::sleep(spf);
                        continue;
                    }
                    if e.kind() != std::io::ErrorKind::WouldBlock {
                        // DXGI_ERROR_INVALID_CALL after each success on Microsoft GPU driver
                        // log::error!("capture frame failed: {:?}", e);
                        if c.as_ref().map(|c| c.is_gdi()) == Some(false) {
                            // nog gdi
                            dxgi_failed_times += 1;
                        }
                        if dxgi_failed_times > MAX_DXGI_FAIL_TIME {
                            c = None;
                            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(FALSE));
                            std::thread::sleep(spf);
                        }
                    } else {
                        shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                    }
                }
                _ => {
                    println!("unreachable!");
                }
            }
        }
    }
}
