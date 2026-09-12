use super::*;

pub fn check_available_hwcodec() -> String {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    hwcodec::common::setup_parent_death_signal();
    let ctx = EncodeContext {
        name: String::from(""),
        mc_name: None,
        width: 1280,
        height: 720,
        pixfmt: DEFAULT_PIXFMT,
        align: HW_STRIDE_ALIGN as _,
        kbs: 1000,
        fps: DEFAULT_FPS,
        gop: DEFAULT_GOP,
        quality: DEFAULT_HW_QUALITY,
        rc: RC_CBR,
        q: -1,
        thread_count: 4,
    };
    #[cfg(feature = "vram")]
    let vram = crate::vram::check_available_vram();
    #[cfg(feature = "vram")]
    let vram_string = vram.2;
    #[cfg(not(feature = "vram"))]
    let vram_string = "".to_owned();
    let c = HwCodecConfig {
        ram_encode: Encoder::available_encoders(ctx, Some(vram_string)),
        ram_decode: Decoder::available_decoders(),
        #[cfg(feature = "vram")]
        vram_encode: vram.0,
        #[cfg(feature = "vram")]
        vram_decode: vram.1,
        signature: hwcodec::common::get_gpu_signature(),
    };
    log::debug!("{c:?}");
    serde_json::to_string(&c).unwrap_or_default()
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn start_check_process() {
    if !enable_hwcodec_option() || HwCodecConfig::already_set() {
        return;
    }
    use hbb_common::allow_err;
    use std::sync::Once;
    let f = || {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(_) = exe.file_name().to_owned() {
                let arg = "--check-hwcodec-config";
                if let Ok(mut child) = std::process::Command::new(exe).arg(arg).spawn() {
                    #[cfg(windows)]
                    hwcodec::common::child_exit_when_parent_exit(child.id());
                    // wait up to 30 seconds, it maybe slow on windows startup for poorly performing machines
                    for _ in 0..30 {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        if let Ok(Some(_)) = child.try_wait() {
                            break;
                        }
                    }
                    allow_err!(child.kill());
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            log::info!("Check hwcodec config, exit with: {status}")
                        }
                        Ok(None) => {
                            log::info!(
                                "Check hwcodec config, status not ready yet, let's really wait"
                            );
                            let res = child.wait();
                            log::info!("Check hwcodec config, wait result: {res:?}");
                        }
                        Err(e) => {
                            log::error!("Check hwcodec config, error attempting to wait: {e}")
                        }
                    }
                }
            }
        };
    };
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(f);
    });
}
