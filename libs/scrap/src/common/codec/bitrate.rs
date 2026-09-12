use super::*;

pub fn base_bitrate(width: u32, height: u32) -> u32 {
    pub(super) const RESOLUTION_PRESETS: &[(u32, u32, u32)] = &[
        (640, 480, 400),     // VGA, 307k pixels
        (800, 600, 500),     // SVGA, 480k pixels
        (1024, 768, 800),    // XGA, 786k pixels
        (1280, 720, 1000),   // 720p, 921k pixels
        (1366, 768, 1100),   // HD, 1049k pixels
        (1440, 900, 1300),   // WXGA+, 1296k pixels
        (1600, 900, 1500),   // HD+, 1440k pixels
        (1920, 1080, 2073),  // 1080p, 2073k pixels
        (2048, 1080, 2200),  // 2K DCI, 2211k pixels
        (2560, 1440, 3000),  // 2K QHD, 3686k pixels
        (3440, 1440, 4000),  // UWQHD, 4953k pixels
        (3840, 2160, 5000),  // 4K UHD, 8294k pixels
        (7680, 4320, 12000), // 8K UHD, 33177k pixels
    ];
    let pixels = width * height;

    let (preset_pixels, preset_bitrate) = RESOLUTION_PRESETS
        .iter()
        .map(|(w, h, bitrate)| (w * h, bitrate))
        .min_by_key(|(preset_pixels, _)| {
            if *preset_pixels >= pixels {
                preset_pixels - pixels
            } else {
                pixels - preset_pixels
            }
        })
        .unwrap_or(((1920 * 1080) as u32, &2073)); // default 1080p

    let bitrate = (*preset_bitrate as f32 * (pixels as f32 / preset_pixels as f32)).round() as u32;

    #[cfg(target_os = "android")]
    {
        let fix = crate::Display::fix_quality() as u32;
        log::debug!("Android screen, fix quality:{}", fix);
        bitrate * fix
    }
    #[cfg(not(target_os = "android"))]
    {
        bitrate
    }
}

pub fn codec_thread_num(limit: usize) -> usize {
    let max: usize = num_cpus::get();
    let mut res;
    let info;
    let mut s = System::new();
    s.refresh_memory();
    let memory = s.available_memory() / 1024 / 1024 / 1024;
    #[cfg(windows)]
    {
        res = 0;
        let percent = base::platform::windows::cpu_uage_one_minute();
        info = format!("cpu usage: {:?}", percent);
        if let Some(pecent) = percent {
            if pecent < 100.0 {
                res = ((100.0 - pecent) * (max as f64) / 200.0).round() as usize;
            }
        }
    }
    #[cfg(not(windows))]
    {
        s.refresh_cpu_usage();
        // https://man7.org/linux/man-pages/man3/getloadavg.3.html
        let avg = s.load_average();
        info = format!("cpu loadavg: {}", avg.one);
        res = (((max as f64) - avg.one) * 0.5).round() as usize;
    }
    res = std::cmp::min(res, max / 2);
    res = std::cmp::min(res, memory as usize / 2);
    //  Use common thread count
    res = match res {
        _ if res >= 64 => 64,
        _ if res >= 32 => 32,
        _ if res >= 16 => 16,
        _ if res >= 8 => 8,
        _ if res >= 4 => 4,
        _ if res >= 2 => 2,
        _ => 1,
    };
    // https://aomedia.googlesource.com/aom/+/refs/heads/main/av1/av1_cx_iface.c#677
    // https://aomedia.googlesource.com/aom/+/refs/heads/main/aom_util/aom_thread.h#26
    // https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vp8/vp8_cx_iface.c#148
    // https://chromium.googlesource.com/webm/libvpx/+/refs/heads/main/vp9/vp9_cx_iface.c#190
    // https://github.com/FFmpeg/FFmpeg/blob/7c16bf0829802534004326c8e65fb6cdbdb634fa/libavcodec/pthread.c#L65
    // https://github.com/FFmpeg/FFmpeg/blob/7c16bf0829802534004326c8e65fb6cdbdb634fa/libavcodec/pthread_internal.h#L26
    // libaom: MAX_NUM_THREADS = 64
    // libvpx: MAX_NUM_THREADS = 64
    // ffmpeg: MAX_AUTO_THREADS = 16
    res = std::cmp::min(res, limit);
    // avoid frequent log
    let log = match THREAD_LOG_TIME.lock().unwrap().clone() {
        Some(instant) => instant.elapsed().as_secs() > 1,
        None => true,
    };
    if log {
        log::info!("cpu num: {max}, {info}, available memory: {memory}G, codec thread: {res}");
        *THREAD_LOG_TIME.lock().unwrap() = Some(Instant::now());
    }
    res
}

pub(super) fn disable_av1() -> bool {
    // aom is very slow for x86 sciter version on windows x64
    // disable it for all 32 bit platforms
    std::mem::size_of::<usize>() == 4
}

#[cfg(not(target_os = "ios"))]
pub fn test_av1() {
    use base::config::keys::OPTION_AV1_TEST;
    use hbb_common::rand::Rng;
    use std::{sync::Once, time::Duration};

    if disable_av1() || !Config::get_option(OPTION_AV1_TEST).is_empty() {
        log::info!("skip test av1");
        return;
    }

    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let f = || {
            let (width, height, quality, keyframe_interval, i444) = (1920, 1080, 1.0, None, false);
            let frame_count = 10;
            let block_size = 300;
            let move_step = 50;
            let generate_fake_data =
                |frame_index: u32, dst_fmt: EncodeYuvFormat| -> ResultType<Vec<u8>> {
                    let mut rng = hbb_common::rand::thread_rng();
                    let mut bgra = vec![0u8; (width * height * 4) as usize];
                    let gradient = frame_index as f32 / frame_count as f32;
                    // floating block
                    let x0 = (frame_index * move_step) % (width - block_size);
                    let y0 = (frame_index * move_step) % (height - block_size);
                    // Fill the block with random colors
                    for y in 0..block_size {
                        for x in 0..block_size {
                            let index = (((y0 + y) * width + x0 + x) * 4) as usize;
                            if index + 3 < bgra.len() {
                                let noise = rng.gen_range(0..255) as f32 / 255.0;
                                let value = (255.0 * gradient + noise * 50.0) as u8;
                                bgra[index] = value;
                                bgra[index + 1] = value;
                                bgra[index + 2] = value;
                                bgra[index + 3] = 255;
                            }
                        }
                    }
                    let dst_stride_y = dst_fmt.stride[0];
                    let dst_stride_uv = dst_fmt.stride[1];
                    let mut dst = vec![0u8; (dst_fmt.h * dst_stride_y * 2) as usize];
                    let dst_y = dst.as_mut_ptr();
                    let dst_u = dst[dst_fmt.u..].as_mut_ptr();
                    let dst_v = dst[dst_fmt.v..].as_mut_ptr();
                    let res = unsafe {
                        crate::ARGBToI420(
                            bgra.as_ptr(),
                            (width * 4) as _,
                            dst_y,
                            dst_stride_y as _,
                            dst_u,
                            dst_stride_uv as _,
                            dst_v,
                            dst_stride_uv as _,
                            width as _,
                            height as _,
                        )
                    };
                    if res != 0 {
                        bail!("ARGBToI420 failed: {}", res);
                    }
                    Ok(dst)
                };
            let Ok(mut av1) = AomEncoder::new(
                EncoderCfg::AOM(AomEncoderConfig {
                    width,
                    height,
                    quality,
                    keyframe_interval,
                }),
                i444,
            ) else {
                return false;
            };
            let mut key_frame_time = Duration::ZERO;
            let mut non_key_frame_time_sum = Duration::ZERO;
            let pts = Instant::now();
            let yuvfmt = av1.yuvfmt();
            for i in 0..frame_count {
                let Ok(yuv) = generate_fake_data(i, yuvfmt.clone()) else {
                    return false;
                };
                let start = Instant::now();
                if av1
                    .encode(pts.elapsed().as_millis() as _, &yuv, super::super::STRIDE_ALIGN)
                    .is_err()
                {
                    log::debug!("av1 encode failed");
                    if i == 0 {
                        return false;
                    }
                }
                if i == 0 {
                    key_frame_time = start.elapsed();
                } else {
                    non_key_frame_time_sum += start.elapsed();
                }
            }
            let non_key_frame_time = non_key_frame_time_sum / (frame_count - 1);
            log::info!(
                "av1 time: key: {:?}, non-key: {:?}, consume: {:?}",
                key_frame_time,
                non_key_frame_time,
                pts.elapsed()
            );
            key_frame_time < Duration::from_millis(90)
                && non_key_frame_time < Duration::from_millis(30)
        };
        std::thread::spawn(move || {
            let v = f();
            Config::set_option(
                OPTION_AV1_TEST.to_string(),
                if v { "Y" } else { "N" }.to_string(),
            );
        });
    });
}
