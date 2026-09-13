use super::*;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct HwCodecConfig {
    #[serde(default)]
    pub signature: u64,
    /// Boot the adapters were probed in: DXGI adapter LUIDs are assigned per
    /// boot, so a cache from an earlier boot names adapters that no longer
    /// exist and every VRAM decode context in it fails to match.
    #[serde(default)]
    pub boot: u64,
    #[serde(default)]
    pub ram_encode: Vec<CodecInfo>,
    #[serde(default)]
    pub ram_decode: Vec<CodecInfo>,
    #[cfg(feature = "vram")]
    #[serde(default)]
    pub vram_encode: Vec<hwcodec::vram::FeatureContext>,
    #[cfg(feature = "vram")]
    #[serde(default)]
    pub vram_decode: Vec<hwcodec::vram::DecodeContext>,
}

// HwCodecConfig2 is used to store the config in json format,
// confy can't serde HwCodecConfig successfully if the non-first struct Vec is empty due to old toml version.
// struct T { a: Vec<A>, b: Vec<String>} will fail if b is empty, but struct T { a: Vec<String>, b: Vec<String>} is ok.
#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub(super) struct HwCodecConfig2 {
    #[serde(default)]
    pub config: String,
}

lazy_static::lazy_static! {
    /// Hardware encoders that failed while running, and when. Failing is not the same as
    /// being absent: a driver reset, an encode session another program took, a mode
    /// change mid-frame. What the machine can do is in the config and stays there; this
    /// says what not to ask for at the moment, and forgetting it after a while is what
    /// keeps a bad minute from costing hardware encoding until the process is restarted.
    static ref FAILED: std::sync::Mutex<
        std::collections::HashMap<String, std::time::Instant>,
    > = Default::default();
}

/// How long a failure is held against an encoder.
const FORGET_FAILURE_AFTER: std::time::Duration = std::time::Duration::from_secs(600);

/// Whether a failure recorded this long ago is still held against the encoder.
fn failure_holds(since: std::time::Duration) -> bool {
    since < FORGET_FAILURE_AFTER
}

/// Names a RAM encoder for the failure registry.
pub fn ram_encoder_id(name: &str) -> String {
    format!("ram:{name}")
}

/// Names a VRAM encoder for the failure registry: a format on one driver on one adapter.
#[cfg(feature = "vram")]
pub fn vram_encoder_id(f: &hwcodec::vram::FeatureContext) -> String {
    format!("vram:{:?}:{:?}:{}", f.data_format, f.driver, f.luid)
}

// ipc server process start check process once, other process get from ipc server once
// install: --server start check process, check process send to --server,  ui get from --server
// portable: ui start check process, check process send to ui
// sciter and unilink: get from ipc server
impl HwCodecConfig {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn set(config: String) {
        let config = serde_json::from_str(&config).unwrap_or_default();
        log::info!("set hwcodec config");
        log::debug!("{config:?}");
        #[cfg(any(windows, target_os = "macos"))]
        hbb_common::config::common_store(
            &HwCodecConfig2 {
                config: serde_json::to_string_pretty(&config).unwrap_or_default(),
            },
            "_hwcodec",
        );
        *CONFIG.lock().unwrap() = Some(config);
        *CONFIG_SET_BY_IPC.lock().unwrap() = true;
    }

    pub fn get() -> HwCodecConfig {
        #[cfg(target_os = "android")]
        {
            let info = crate::android::ffi::get_codec_info();
            log::info!("all codec info: {info:?}");
            struct T {
                name_prefix: &'static str,
                data_format: DataFormat,
            }
            let ts = vec![
                T {
                    name_prefix: "h264",
                    data_format: DataFormat::H264,
                },
                T {
                    name_prefix: "hevc",
                    data_format: DataFormat::H265,
                },
            ];
            let mut e = vec![];
            if let Some(info) = info {
                ts.iter().for_each(|t| {
                    let codecs: Vec<_> = info
                        .codecs
                        .iter()
                        .filter(|c| {
                            c.is_encoder
                                && c.mime_type.as_str() == get_mime_type(t.data_format)
                                && c.nv12
                                && c.hw == Some(true) //only use hardware codec
                        })
                        .collect();
                    let screen_wh = std::cmp::max(info.w, info.h);
                    let mut best = None;
                    if let Some(codec) = codecs
                        .iter()
                        .find(|c| c.max_width >= screen_wh && c.max_height >= screen_wh)
                    {
                        best = Some(codec.name.clone());
                    } else {
                        // find the max resolution
                        let mut max_area = 0;
                        for codec in codecs.iter() {
                            if codec.max_width * codec.max_height > max_area {
                                best = Some(codec.name.clone());
                                max_area = codec.max_width * codec.max_height;
                            }
                        }
                    }
                    if let Some(best) = best {
                        e.push(CodecInfo {
                            name: format!("{}_mediacodec", t.name_prefix),
                            mc_name: Some(best),
                            format: t.data_format,
                            hwdevice: hwcodec::ffmpeg::AVHWDeviceType::AV_HWDEVICE_TYPE_NONE,
                            priority: 0,
                        });
                    }
                });
            }
            log::debug!("e: {e:?}");
            HwCodecConfig {
                ram_encode: e,
                ..Default::default()
            }
        }
        #[cfg(any(windows, target_os = "macos"))]
        {
            let config = CONFIG.lock().unwrap().clone();
            match config {
                Some(c) => c,
                None => {
                    log::info!("try load cached hwcodec config");
                    let c = hbb_common::config::common_load::<HwCodecConfig2>("_hwcodec");
                    let c: HwCodecConfig = serde_json::from_str(&c.config).unwrap_or_default();
                    let new_signature = hwcodec::common::get_gpu_signature();
                    if c.signature == new_signature && c.boot == boot_stamp() {
                        log::debug!("load cached hwcodec config: {c:?}");
                        *CONFIG.lock().unwrap() = Some(c.clone());
                        c
                    } else {
                        log::info!(
                            "gpu signature or boot changed, {} -> {}, boot {} -> {}",
                            c.signature,
                            new_signature,
                            c.boot,
                            boot_stamp()
                        );
                        HwCodecConfig::default()
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        {
            CONFIG.lock().unwrap().clone().unwrap_or_default()
        }
        #[cfg(target_os = "ios")]
        {
            HwCodecConfig::default()
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn get_set_value() -> Option<HwCodecConfig> {
        let set = CONFIG_SET_BY_IPC.lock().unwrap().clone();
        if set {
            CONFIG.lock().unwrap().clone()
        } else {
            None
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub fn already_set() -> bool {
        CONFIG_SET_BY_IPC.lock().unwrap().clone()
    }

    /// One encoder has just failed to do its job. Only that one is put aside.
    pub fn note_failed(id: String) {
        log::info!("hwcodec: {id} failed, not asking it again for a while");
        FAILED
            .lock()
            .unwrap()
            .insert(id, std::time::Instant::now());
        crate::codec::Encoder::update(crate::codec::EncodingUpdate::Check);
    }

    /// Whether an encoder failed recently enough to still be skipped.
    pub fn recently_failed(id: &str) -> bool {
        let mut failed = FAILED.lock().unwrap();
        failed.retain(|_, at| failure_holds(at.elapsed()));
        failed.contains_key(id)
    }

    pub fn clear(vram: bool, encode: bool) {
        log::info!("clear hwcodec config, vram: {vram}, encode: {encode}");
        #[cfg(target_os = "android")]
        crate::android::ffi::clear_codec_info();
        #[cfg(not(target_os = "android"))]
        {
            let mut c = CONFIG.lock().unwrap();
            if let Some(c) = c.as_mut() {
                if vram {
                    #[cfg(feature = "vram")]
                    if encode {
                        c.vram_encode = vec![];
                    } else {
                        c.vram_decode = vec![];
                    }
                } else {
                    if encode {
                        c.ram_encode = vec![];
                    } else {
                        c.ram_decode = vec![];
                    }
                }
            }
        }
        crate::codec::Encoder::update(crate::codec::EncodingUpdate::Check);
    }
}

/// Seconds since the epoch at which this boot started, rounded to a minute so
/// two probes in the same boot agree; 0 where LUIDs do not change per boot.
pub fn boot_stamp() -> u64 {
    #[cfg(windows)]
    {
        extern "system" {
            fn GetTickCount64() -> u64;
        }
        let up = unsafe { GetTickCount64() } / 1000;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        (now.saturating_sub(up) / 60) * 60
    }
    #[cfg(not(windows))]
    {
        0
    }
}

#[cfg(test)]
mod boot_stamp_tests {
    use super::boot_stamp;

    #[test]
    fn the_stamp_is_stable_within_a_boot() {
        let a = boot_stamp();
        std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(a, boot_stamp());
        if cfg!(windows) {
            assert!(a > 1_600_000_000, "{a}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_encoder_that_failed_is_the_only_one_put_aside() {
        let failed = ram_encoder_id("h264_test_vendor");
        let other = ram_encoder_id("hevc_test_vendor");
        HwCodecConfig::note_failed(failed.clone());
        assert!(HwCodecConfig::recently_failed(&failed));
        assert!(
            !HwCodecConfig::recently_failed(&other),
            "one encoder failing said nothing about the other"
        );
    }

    #[test]
    fn an_encoder_nothing_is_known_about_is_not_skipped() {
        assert!(!HwCodecConfig::recently_failed(&ram_encoder_id("never_seen")));
    }

    /// The failure is held for a while and then forgotten, so a machine that had a bad
    /// minute is not left on software encoding until it is restarted.
    #[test]
    fn a_failure_is_held_for_a_while_and_then_forgotten() {
        assert!(failure_holds(std::time::Duration::ZERO));
        assert!(failure_holds(FORGET_FAILURE_AFTER - std::time::Duration::from_secs(1)));
        assert!(!failure_holds(FORGET_FAILURE_AFTER));
        assert!(!failure_holds(FORGET_FAILURE_AFTER * 2));
    }

    #[test]
    fn each_codec_and_adapter_is_named_apart() {
        assert_ne!(ram_encoder_id("h264_nvenc"), ram_encoder_id("hevc_nvenc"));
        assert!(ram_encoder_id("h264_nvenc").starts_with("ram:"));
    }
}
