use super::*;

/// Per-second pipeline diagnostics, off unless `RUSTDESK_QOS_VERBOSE` is set.
/// The default log level is `debug`, so an unconditional line here would land in
/// every user's log file once a second forever.  Nothing enables it implicitly.
pub(crate) fn qos_diag_verbose() -> bool {
    static VERBOSE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *VERBOSE.get_or_init(|| std::env::var("RUSTDESK_QOS_VERBOSE").is_ok())
}

pub(super) fn check_qos(
    encoder: &mut Encoder,
    ratio: &mut f32,
    spf: &mut Duration,
    client_record: bool,
    send_counter: &mut usize,
    sent_counter: &mut usize,
    wait_max_ms: &mut u32,
    second_instant: &mut Instant,
    name: &str,
) -> ResultType<()> {
    let mut video_qos = VIDEO_QOS.lock().unwrap();
    *spf = video_qos.spf();
    if *ratio != video_qos.ratio() {
        *ratio = video_qos.ratio();
        if encoder.support_changing_quality() {
            allow_err!(encoder.set_quality(*ratio));
            video_qos.store_bitrate(encoder.bitrate());
        } else {
            // Now only vaapi doesn't support changing quality
            if !video_qos.in_vbr_state() && !video_qos.latest_quality().is_custom() {
                log::info!("switch to change quality");
                bail!("SWITCH");
            }
        }
    }
    if client_record != video_qos.record() {
        log::info!("switch due to record changed");
        bail!("SWITCH");
    }
    if second_instant.elapsed() > Duration::from_secs(1) {
        *second_instant = Instant::now();
        video_qos.update_display_data(&name, *send_counter);
        // Diagnostics only, joined with `qos_trace` on `t`: the controller's target
        // is not the rate the encoder produced, and neither is the rate the send
        // path accepted.
        if qos_diag_verbose() {
            log::debug!(
                "qos_video t={} display={name} captured={} sent={} wait_max={}",
                hbb_common::get_time(),
                *send_counter,
                *sent_counter,
                *wait_max_ms
            );
        }
        *send_counter = 0;
        *sent_counter = 0;
        *wait_max_ms = 0;
    }
    drop(video_qos);
    Ok(())
}
