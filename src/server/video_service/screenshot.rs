use super::*;

pub fn set_take_screenshot(source: VideoSource, display_idx: usize, sid: String, tx: Sender) {
    SCREENSHOTS.lock().unwrap().insert(
        (source, display_idx),
        Screenshot {
            sid,
            tx,
            restore_vram: false,
        },
    );
}

// We need to this function, because the `stride` may be larger than `width * 4`.
pub(super) fn get_rgba_from_pixelbuf<'a>(pixbuf: &scrap::PixelBuffer<'a>) -> ResultType<Vec<u8>> {
    let w = pixbuf.width();
    let h = pixbuf.height();
    let stride = pixbuf.stride();
    let Some(s) = stride.get(0) else {
        bail!("Invalid pixel buf stride.")
    };

    if *s == w * 4 {
        let mut rgba = vec![];
        scrap::convert(pixbuf, scrap::Pixfmt::RGBA, &mut rgba)?;
        Ok(rgba)
    } else {
        let bgra = pixbuf.data();
        let mut bit_flipped = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let i = s * y + 4 * x;
                bit_flipped.extend_from_slice(&[bgra[i + 2], bgra[i + 1], bgra[i], bgra[i + 3]]);
            }
        }
        Ok(bit_flipped)
    }
}

pub(super) fn handle_screenshot(screenshot: Screenshot, msg: String, w: usize, h: usize, data: Vec<u8>) {
    let mut response = ScreenshotResponse::new();
    response.sid = screenshot.sid;
    if msg.is_empty() {
        if data.is_empty() {
            response.msg = "Failed to take screenshot, please try again later.".to_owned();
        } else {
            fn encode_png(width: usize, height: usize, rgba: Vec<u8>) -> ResultType<Vec<u8>> {
                let mut png = Vec::new();
                let mut encoder =
                    repng::Options::smallest(width as _, height as _).build(&mut png)?;
                encoder.write(&rgba)?;
                encoder.finish()?;
                Ok(png)
            }
            match encode_png(w as _, h as _, data) {
                Ok(png) => {
                    response.data = png.into();
                }
                Err(e) => {
                    response.msg = format!("Error encoding png: {}", e);
                }
            }
        }
    } else {
        response.msg = msg;
    }
    let mut msg_out = Message::new();
    msg_out.set_screenshot_response(response);
    if let Err(e) = screenshot
        .tx
        .send((hbb_common::tokio::time::Instant::now(), Arc::new(msg_out)))
    {
        log::error!("Failed to send screenshot, {}", e);
    }
}
