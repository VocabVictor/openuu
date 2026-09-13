use super::*;

pub(super) fn run(vs: VideoService) -> ResultType<()> {
    let mut _raii = Raii::new(vs.idx, vs.sp.name());
    // Wayland only support one video capturer for now. It is ok to call ensure_inited() here.
    //
    // ensure_inited() is needed because clear() may be called.
    // to-do: wayland ensure_inited should pass current display index.
    // But for now, we do not support multi-screen capture on wayland.
    #[cfg(target_os = "linux")]
    super::wayland::ensure_inited()?;
    #[cfg(target_os = "linux")]
    let _wayland_call_on_ret = {
        // Increment active display count when starting
        let _display_count = super::wayland::increment_active_display_count();

        SimpleCallOnReturn {
            b: true,
            f: Box::new(|| {
                // Decrement active display count and only clear if this was the last display
                let remaining_count = super::wayland::decrement_active_display_count();
                if remaining_count == 0 {
                    super::wayland::clear();
                }
            }),
        }
    };

    #[cfg(windows)]
    let last_portable_service_running = crate::portable_service::client::running();
    #[cfg(not(windows))]
    let last_portable_service_running = false;

    let display_idx = vs.idx;
    let sp = vs.sp;
    let mut c = get_capturer(vs.source, display_idx, last_portable_service_running)?;
    #[cfg(windows)]
    if !scrap::codec::enable_directx_capture() && !c.is_gdi() {
        log::info!("disable dxgi with option, fall back to gdi");
        c.set_gdi();
    }
    let mut video_qos = VIDEO_QOS.lock().unwrap();
    let mut spf = video_qos.spf();
    let mut quality = video_qos.ratio();
    let record_incoming = config::option2bool(
        "allow-auto-record-incoming",
        &Config::get_option("allow-auto-record-incoming"),
    );
    let client_record = video_qos.record();
    drop(video_qos);
    let (mut encoder, encoder_cfg, codec_format, use_i444, recorder) = match setup_encoder(
        &c,
        sp.name(),
        quality,
        client_record,
        record_incoming,
        last_portable_service_running,
        vs.source,
        display_idx,
    ) {
        Ok(result) => result,
        Err(err) => {
            log::error!("Failed to create encoder: {err:?}, fallback to VP9");
            Encoder::set_fallback(&EncoderCfg::VPX(VpxEncoderConfig {
                width: c.width as _,
                height: c.height as _,
                quality,
                codec: VpxVideoCodecId::VP9,
                keyframe_interval: None,
            }));
            setup_encoder(
                &c,
                sp.name(),
                quality,
                client_record,
                record_incoming,
                last_portable_service_running,
                vs.source,
                display_idx,
            )?
        }
    };
    #[cfg(feature = "vram")]
    c.set_output_texture(encoder.input_texture());
    #[cfg(target_os = "android")]
    if vs.source.is_monitor() {
        if let Err(e) = check_change_scale(encoder.is_hardware()) {
            try_broadcast_display_changed(&sp, display_idx, &c, true).ok();
            bail!(e);
        }
    }
    // The encoder was created for a fixed fps; start it on the one QoS paces at, since
    // check_qos only reports changes from here on.
    allow_err!(encoder.set_fps(VIDEO_QOS.lock().unwrap().fps()));
    VIDEO_QOS.lock().unwrap().store_bitrate(encoder.bitrate());
    VIDEO_QOS
        .lock()
        .unwrap()
        .set_support_changing_quality(&sp.name(), encoder.support_changing_quality());
    log::info!("initial quality: {quality:?}");

    if sp.is_option_true(OPTION_REFRESH) {
        sp.set_option_bool(OPTION_REFRESH, false);
    }

    let mut frame_controller = VideoFrameController::new(display_idx);

    let start = time::Instant::now();
    let mut last_check_displays = time::Instant::now();
    #[cfg(windows)]
    let mut try_gdi = 1;
    #[cfg(windows)]
    let capture_started = Instant::now();
    // Rebuilds of the duplication since the last capture that worked.
    #[cfg(windows)]
    let mut rebuilds = 0u32;
    #[cfg(windows)]
    log::info!("gdi: {}", c.is_gdi());
    #[cfg(windows)]
    start_uac_elevation_check();

    #[cfg(target_os = "linux")]
    let mut would_block_count = 0u32;
    let mut yuv = Vec::new();
    let mut mid_data = Vec::new();
    let mut repeat_encode_counter = 0;
    let repeat_encode_max = 10;
    let mut encode_fail_counter = 0;
    let mut first_frame = true;
    let capture_width = c.width;
    let capture_height = c.height;
    let (mut second_instant, mut send_counter) = (Instant::now(), 0);
    // Diagnostics only.  `send_counter` counts capture rounds, which is not the
    // number of frames that reached a connection: the encoder's own rate control
    // drops frames when the bitrate cannot carry them.  `wait_max_ms` is how long
    // a round waited for the previous frame to be picked up, so a blocked write
    // shows up here as capture stalling rather than as a slow network.
    let (mut sent_counter, mut wait_max_ms) = (0usize, 0u32);
    // Captures in a row that brought nothing: the screen is not being touched.
    let mut still_captures = 0u32;
    let mut fetch_hold = FetchHold::new();

    while sp.ok() {
        #[cfg(windows)]
        check_uac_switch(c.privacy_mode_id, c._capturer_privacy_mode_id)?;
        check_qos(
            &mut encoder,
            &mut quality,
            &mut spf,
            client_record,
            &mut send_counter,
            &mut sent_counter,
            &mut wait_max_ms,
            &mut second_instant,
            &sp.name(),
        )?;
        if sp.is_option_true(OPTION_REFRESH) {
            if vs.source.is_monitor() {
                let _ = try_broadcast_display_changed(&sp, display_idx, &c, true);
            }
            log::info!("switch to refresh");
            bail!("SWITCH");
        }
        if codec_format != Encoder::negotiated_codec() {
            log::info!(
                "switch due to codec changed, {:?} -> {:?}",
                codec_format,
                Encoder::negotiated_codec()
            );
            bail!("SWITCH");
        }
        #[cfg(windows)]
        if last_portable_service_running != crate::portable_service::client::running() {
            log::info!("switch due to portable service running changed");
            bail!("SWITCH");
        }
        if Encoder::use_i444(&encoder_cfg) != use_i444 {
            log::info!("switch due to i444 changed");
            bail!("SWITCH");
        }
        #[cfg(all(windows, feature = "vram"))]
        if c.is_gdi() && encoder.input_texture() {
            log::info!("changed to gdi when using vram");
            VRamEncoder::set_fallback_gdi(sp.name(), true);
            bail!("SWITCH");
        }
        if vs.source.is_monitor() {
            check_privacy_mode_changed(&sp, display_idx, &c)?;
        }
        #[cfg(windows)]
        {
            if crate::platform::windows::desktop_changed()
                && !crate::portable_service::client::running()
            {
                bail!("Desktop changed");
            }
        }
        let now = time::Instant::now();
        if vs.source.is_monitor() && last_check_displays.elapsed().as_millis() > 1000 {
            last_check_displays = now;
            // This check may be redundant, but it is better to be safe.
            // The previous check in `sp.is_option_true(OPTION_REFRESH)` block may be enough.
            try_broadcast_display_changed(&sp, display_idx, &c, false)?;
        }

        let ack_window = ack_wait_window(spf, VIDEO_QOS.lock().unwrap().rtt_baseline_ms());
        // The previous frame is still with a slow connection: capture again next round
        // rather than queue another encoded frame behind it.
        if !fetch_hold.may_encode(&mut frame_controller, ack_window, || {
            if vs.source.is_monitor() {
                check_privacy_mode_changed(&sp, display_idx, &c)?;
            }
            Ok(())
        })? {
            continue;
        }
        frame_controller.reset();

        let time = now - start;
        let ms = (time.as_secs() * 1000 + time.subsec_millis() as u64) as i64;
        let res = match c.frame(capture_timeout(spf, still_captures)) {
            Ok(frame) => {
                repeat_encode_counter = 0;
                still_captures = 0;
                if frame.valid() {
                    let screenshot_key = (vs.source, display_idx);
                    let screenshot = SCREENSHOTS.lock().unwrap().remove(&screenshot_key);
                    if let Some(mut screenshot) = screenshot {
                        let restore_vram = screenshot.restore_vram;
                        let (msg, w, h, data) = match &frame {
                            scrap::Frame::PixelBuffer(f) => match get_rgba_from_pixelbuf(f) {
                                Ok(rgba) => ("".to_owned(), f.width(), f.height(), rgba),
                                Err(e) => {
                                    let serr = e.to_string();
                                    log::error!(
                                        "Failed to convert the pix format into rgba, {}",
                                        &serr
                                    );
                                    (format!("Convert pixfmt: {}", serr), 0, 0, vec![])
                                }
                            },
                            scrap::Frame::Texture(_) => {
                                if restore_vram {
                                    // Already set one time, just ignore to break infinite loop.
                                    // Though it's unreachable, this branch is kept to avoid infinite loop.
                                    (
                                        "Please change codec and try again.".to_owned(),
                                        0,
                                        0,
                                        vec![],
                                    )
                                } else {
                                    #[cfg(all(windows, feature = "vram"))]
                                    VRamEncoder::set_not_use(sp.name(), true);
                                    screenshot.restore_vram = true;
                                    SCREENSHOTS
                                        .lock()
                                        .unwrap()
                                        .insert(screenshot_key, screenshot);
                                    _raii.try_vram = false;
                                    bail!("SWITCH");
                                }
                            }
                        };
                        std::thread::spawn(move || {
                            handle_screenshot(screenshot, msg, w, h, data);
                        });
                        if restore_vram {
                            bail!("SWITCH");
                        }
                    }

                    let frame = frame.to(encoder.yuvfmt(), &mut yuv, &mut mid_data)?;
                    let send_conn_ids = handle_one_frame(
                        display_idx,
                        &sp,
                        frame,
                        ms,
                        &mut encoder,
                        recorder.clone(),
                        &mut encode_fail_counter,
                        &mut first_frame,
                        capture_width,
                        capture_height,
                    )?;
                    if !send_conn_ids.is_empty() {
                        sent_counter += 1;
                    }
                    frame_controller.set_send(now, send_conn_ids);
                    send_counter += 1;
                }
                #[cfg(windows)]
                {
                    #[cfg(feature = "vram")]
                    if try_gdi == 1 && !c.is_gdi() {
                        VRamEncoder::set_fallback_gdi(sp.name(), false);
                    }
                    try_gdi = 0;
                    rebuilds = 0;
                }
                Ok(())
            }
            Err(err) => Err(err),
        };

        match res {
            Err(ref e) if e.kind() == WouldBlock => {
                still_captures = still_captures.saturating_add(1);
                #[cfg(windows)]
                if try_gdi > 0 && !c.is_gdi() {
                    let waited = capture_started.elapsed();
                    if gdi_fallback::give_up_on_duplication(try_gdi, waited) {
                        c.set_gdi();
                        try_gdi = 0;
                        log::info!("No image in {:?}, fall back to gdi", waited);
                    } else {
                        try_gdi += 1;
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    would_block_count += 1;
                    if !is_x11() {
                        if would_block_count >= 100 {
                            // to-do: Unknown reason for WouldBlock 100 times (seconds = 100 * 1 / fps)
                            // https://github.com/rustdesk/rustdesk/blob/63e6b2f8ab51743e77a151e2b7ff18816f5fa2fb/libs/scrap/src/common/wayland.rs#L81
                            //
                            // Do not reset the capturer for now, as it will cause the prompt to show every few minutes.
                            // https://github.com/rustdesk/rustdesk/issues/4276
                            //
                            // super::wayland::clear();
                            // bail!("Wayland capturer none 100 times, try restart capture");
                        }
                    }
                }
                if !encoder.latency_free() && yuv.len() > 0 {
                    // yun.len() > 0 means the frame is not texture.
                    if repeat_encode_counter < repeat_encode_max {
                        repeat_encode_counter += 1;
                        let send_conn_ids = handle_one_frame(
                            display_idx,
                            &sp,
                            EncodeInput::YUV(&yuv),
                            ms,
                            &mut encoder,
                            recorder.clone(),
                            &mut encode_fail_counter,
                            &mut first_frame,
                            capture_width,
                            capture_height,
                        )?;
                        if !send_conn_ids.is_empty() {
                            sent_counter += 1;
                        }
                        frame_controller.set_send(now, send_conn_ids);
                        send_counter += 1;
                    }
                }
            }
            Err(err) => {
                // This check may be redundant, but it is better to be safe.
                // The previous check in `sp.is_option_true(OPTION_REFRESH)` block may be enough.
                if vs.source.is_monitor() {
                    try_broadcast_display_changed(&sp, display_idx, &c, true)?;
                }

                #[cfg(windows)]
                if !c.is_gdi() {
                    match gdi_fallback::after_capture_error(err.kind(), rebuilds) {
                        gdi_fallback::AfterFailure::Rebuild => {
                            rebuilds += 1;
                            let wait = gdi_fallback::rebuild_backoff(rebuilds - 1);
                            log::info!(
                                "dxgi: {:?}; making the duplication again in {:?} ({rebuilds})",
                                err,
                                wait
                            );
                            std::thread::sleep(wait);
                            c = get_capturer(
                                vs.source,
                                display_idx,
                                last_portable_service_running,
                            )?;
                            if !scrap::codec::enable_directx_capture() && !c.is_gdi() {
                                c.set_gdi();
                            }
                            #[cfg(feature = "vram")]
                            c.set_output_texture(encoder.input_texture());
                            continue;
                        }
                        gdi_fallback::AfterFailure::UseGdi => {
                            c.set_gdi();
                            log::info!("dxgi error, fall back to gdi: {:?}", err);
                            continue;
                        }
                    }
                }
                return Err(err.into());
            }
            _ => {
                #[cfg(target_os = "linux")]
                {
                    would_block_count = 0;
                }
            }
        }

        fetch_hold.after_send(&mut frame_controller, ack_window, || {
            if vs.source.is_monitor() {
                check_privacy_mode_changed(&sp, display_idx, &c)?;
            }
            Ok(())
        })?;
        wait_max_ms = wait_max_ms.max(fetch_hold.last_wait_ms());

        let elapsed = now.elapsed();
        // may need to enable frame(timeout)
        log::trace!("{:?} {:?}", time::Instant::now(), elapsed);
        if elapsed < spf {
            std::thread::sleep(spf - elapsed);
        }
    }

    Ok(())
}
