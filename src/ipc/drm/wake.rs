use super::*;

/// Connectors a wake did NOT bring back. SELF-REFUTING: an entry later seen DRIVEN is removed.
#[cfg(feature = "drm-wake")]
pub(super) static DRM_WAKE_HOPELESS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

#[cfg(feature = "drm-wake")]
pub(super) fn drm_wakeable_undriven(displays: &[DrmDisplayInfo], undriven: &[String]) -> Vec<String> {
    let mut hopeless = DRM_WAKE_HOPELESS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !hopeless.is_empty() {
        hopeless.retain(|id| {
            let driven_now = displays
                .iter()
                .any(|d| format!("{}:{}", d.device, d.name) == *id);
            if driven_now {
                log::info!("drm: {id} is scanning out after all; treating it as wakeable again");
            }
            !driven_now
        });
    }
    undriven
        .iter()
        .filter(|id| !hopeless.iter().any(|h| h == *id))
        .cloned()
        .collect()
}

#[cfg(feature = "drm-wake")]
pub(super) static DRM_LAST_WAKE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[cfg(feature = "drm-wake")]
pub(super) static DRM_WAKE_UNAVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Wake config key; `enable-` is load-bearing: an absent value reads as `!= "N"`, so it defaults ON.
#[cfg(feature = "drm-wake")]
pub(super) const OPTION_ENABLE_DRM_DISPLAY_WAKE: &str = "enable-drm-display-wake";

#[cfg(feature = "drm-wake")]
pub(super) const DRM_WAKE_MIN_GAP: std::time::Duration = std::time::Duration::from_secs(20);
#[cfg(feature = "drm-wake")]
pub(super) const DRM_WAKE_DEVICE_SETTLE: std::time::Duration = std::time::Duration::from_millis(400);
#[cfg(feature = "drm-wake")]
pub(super) const DRM_WAKE_RECHECK_TOTAL: std::time::Duration = std::time::Duration::from_secs(3);
#[cfg(feature = "drm-wake")]
pub(super) const DRM_WAKE_SETTLE_WINDOW: std::time::Duration = std::time::Duration::from_secs(5);

/// Seconds since service start, monotonic: SystemTime would let a clock step re-open the wake gate.
#[cfg(feature = "drm-wake")]
pub(super) fn drm_wake_clock_secs() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs()
}

/// Look like user activity so the compositor re-enables an idle-DISABLED connector (until it does,
/// nothing scans out). Measured on a T2 greeter: one relative move restored a 2880x1800 scanout.
#[cfg(feature = "drm-wake")]
pub(super) fn drm_wake_displays(reason: &str) -> bool {
    use std::sync::atomic::Ordering;

    if DRM_WAKE_UNAVAILABLE.load(Ordering::Relaxed) {
        return false;
    }
    let now = drm_wake_clock_secs();
    loop {
        let last = DRM_LAST_WAKE.load(Ordering::Acquire);
        if last != 0 && now.saturating_sub(last) < DRM_WAKE_MIN_GAP.as_secs() {
            log::debug!(
                "drm: not waking displays ({reason}): a wake {}s ago is still recent",
                now.saturating_sub(last)
            );
            return false;
        }
        if DRM_LAST_WAKE
            .compare_exchange(last, now.max(1), Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            break;
        }
    }

    // It has to look like a MOUSE: libinput ignores a device with a single relative axis and no
    // buttons. Measured: REL_X + REL_Y + BTN_LEFT woke the panel; REL_X alone did not.
    let mut axes = evdev::AttributeSet::<evdev::RelativeAxisType>::new();
    axes.insert(evdev::RelativeAxisType::REL_X);
    axes.insert(evdev::RelativeAxisType::REL_Y);
    let mut keys = evdev::AttributeSet::<evdev::Key>::new();
    keys.insert(evdev::Key::BTN_LEFT);
    let built = evdev::uinput::VirtualDeviceBuilder::new()
        .and_then(|b| b.name("RustDesk DRM display wake").with_relative_axes(&axes))
        .and_then(|b| b.with_keys(&keys))
        .and_then(|b| b.build());
    let mut dev = match built {
        Ok(d) => d,
        Err(err) => {
            DRM_WAKE_UNAVAILABLE.store(true, Ordering::Relaxed);
            log::warn!(
                "drm: cannot wake displays ({reason}): no uinput device ({err}). A compositor that \
                 disabled its outputs will keep them disabled, so there is no scanout to capture \
                 until something else generates input. Note input injection needs uinput too, so \
                 this session cannot control the host either."
            );
            return false;
        }
    };

    // A FRESH uinput device is not bound yet; events written before udev binds it are lost. Measured
    // back to back: with this pause the panel went `disabled -> enabled`, without it it did not.
    std::thread::sleep(DRM_WAKE_DEVICE_SETTLE);

    // +1 then -1: activity with zero net displacement. emit() appends the SYN_REPORT itself.
    let step = |v: i32| {
        evdev::InputEvent::new(
            evdev::EventType::RELATIVE,
            evdev::RelativeAxisType::REL_X.0,
            v,
        )
    };
    let ok = dev.emit(&[step(1)]).and_then(|_| {
        std::thread::sleep(std::time::Duration::from_millis(120));
        dev.emit(&[step(-1)])
    });
    if let Err(err) = ok {
        log::warn!("drm: display wake ({reason}) failed to emit: {err}");
        return false;
    }
    log::info!("drm: no display was scanning out ({reason}); asked the compositor to wake up");
    true
}

#[cfg(not(feature = "drm-wake"))]
pub(super) fn drm_enumerate_settled(reason: &str) -> Vec<DrmDisplayInfo> {
    let (displays, undriven) = drm_enumerate_all_displays();
    if !undriven.is_empty() {
        log::debug!(
            "drm: {} connected display(s) have no CRTC ({reason}); this build has no display wake",
            undriven.len()
        );
    }
    displays
}

/// Wake build: wake an undriven display and WAIT for the settled topology. The wait applies to every
/// handshake whose wake may still be in flight, not only the one whose attempt won the rate limit.
#[cfg(feature = "drm-wake")]
pub(super) fn drm_enumerate_settled(reason: &str) -> Vec<DrmDisplayInfo> {
    use std::sync::atomic::Ordering;

    let (displays, undriven) = drm_enumerate_all_displays();
    if !hbb_common::config::Config::get_bool_option(OPTION_ENABLE_DRM_DISPLAY_WAKE) {
        if !undriven.is_empty() {
            log::info!(
                "drm: {} connected display(s) have no CRTC ({reason}), but the display wake is \
                 disabled by configuration ({OPTION_ENABLE_DRM_DISPLAY_WAKE}=N)",
                undriven.len()
            );
        }
        return displays;
    }
    let wakeable = drm_wakeable_undriven(&displays, &undriven);
    if wakeable.is_empty() {
        return displays;
    }
    let fired = drm_wake_displays(&format!(
        "{reason} and {n} connected display(s) had no CRTC",
        n = wakeable.len()
    ));
    if !fired {
        if DRM_WAKE_UNAVAILABLE.load(Ordering::Relaxed) {
            return displays;
        }
        let last = DRM_LAST_WAKE.load(Ordering::Acquire);
        if last == 0
            || drm_wake_clock_secs().saturating_sub(last) > DRM_WAKE_SETTLE_WINDOW.as_secs()
        {
            return displays;
        }
    }
    let before_len = displays.len();
    let deadline = std::time::Instant::now() + DRM_WAKE_RECHECK_TOTAL;
    let mut cur = displays;
    let mut cur_wakeable = wakeable;
    while !cur_wakeable.is_empty() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(300));
        let (next, next_undriven) = drm_enumerate_all_displays();
        cur_wakeable = drm_wakeable_undriven(&next, &next_undriven);
        cur = next;
    }
    if cur.len() > before_len {
        log::info!(
            "drm: {} display(s) came back after the wake ({} -> {}{})",
            cur.len() - before_len,
            before_len,
            cur.len(),
            if cur_wakeable.is_empty() {
                String::new()
            } else {
                format!(", {} still undriven", cur_wakeable.len())
            }
        );
        schedule_drm_cache_refresh();
    }
    if fired && !cur_wakeable.is_empty() {
        // Only the handshake that FIRED latches; a loser's baseline was taken mid-transition.
        let mut hopeless = DRM_WAKE_HOPELESS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for id in &cur_wakeable {
            if !hopeless.iter().any(|h| h == id) {
                hopeless.push(id.clone());
            }
        }
        log::info!(
            "drm: the wake did not bring back {list}; not asking again for {these} until {it_is} \
             seen scanning out",
            list = cur_wakeable.join(", "),
            these = if cur_wakeable.len() == 1 { "it" } else { "them" },
            it_is = if cur_wakeable.len() == 1 { "it is" } else { "they are" },
        );
    }
    cur
}
