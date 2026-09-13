use super::*;

pub fn init() {
    set_map_err(map_err_scrap);
}

pub(in crate::server) fn increment_active_display_count() -> usize {
    let mut count = ACTIVE_DISPLAY_COUNT.write().unwrap();
    *count += 1;
    *count
}

pub(in crate::server) fn decrement_active_display_count() -> usize {
    let mut count = ACTIVE_DISPLAY_COUNT.write().unwrap();
    if *count > 0 {
        *count -= 1;
    }
    *count
}

pub(super) fn map_err_scrap(err: String) -> io::Error {
    // to-do: Handle error better, do not restart server
    // Reached by the capture loop, which is what this crude self-heal was for. A no-reply
    // during the portal handshake is tagged below and is reported instead of exiting: at
    // login there is someone waiting to be told, and a portal that is slow to activate is
    // not a reason to take the service down.
    if err.starts_with("Did not receive a reply") {
        log::error!("Fatal pipewire error, {}", &err);
        std::process::exit(-1);
    }

    if let Some(tag) = err.strip_prefix(WAYLAND_STAGE_TAG) {
        log_staged_once(&err);
        return io::Error::new(
            io::ErrorKind::Other,
            staged_message(tag, is_ubuntu_before_21()),
        );
    }

    if DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() {
        if DISTRO.version_id < "21".to_owned() {
            io::Error::new(io::ErrorKind::Other, SCRAP_UBUNTU_HIGHER_REQUIRED)
        } else {
            try_log(&err);
            io::Error::new(io::ErrorKind::Other, err)
        }
    } else {
        try_log(&err);
        let err_lower = err.to_ascii_lowercase();
        if err_lower.contains("org.freedesktop.portal")
            || err_lower.contains("dbus")
            || err_lower.contains("d-bus")
        {
            // The portal D-Bus interface is unreachable. This typically means
            // xdg-desktop-portal has crashed... for more info, see: Issue #12897
            io::Error::new(io::ErrorKind::Other, SCRAP_XDP_PORTAL_UNAVAILABLE)
        } else if err_lower.contains("pipewire") {
            io::Error::new(io::ErrorKind::Other, SCRAP_OTHER_VERSION_OR_X11_REQUIRED)
        } else {
            io::Error::new(io::ErrorKind::Other, SCRAP_X11_REQUIRED)
        }
    }
}

/// `Display::all` and `Capturer::new` reach the peer through `map_err_scrap`, but
/// `fill_displays` opens a portal session of its own and returns its error straight up, so a
/// tag has to be resolved here or it lands in the login dialog verbatim.
pub(super) fn map_staged_err(err: anyhow::Error) -> anyhow::Error {
    let text = err.to_string();
    match text.strip_prefix(WAYLAND_STAGE_TAG) {
        Some(tag) => {
            log_staged_once(&text);
            anyhow::anyhow!(staged_message(tag, is_ubuntu_before_21()))
        }
        None => err,
    }
}

// The video service retries about once a second, so a wedged portal would otherwise write a
// line a second forever. Repeat the message only when the cause changes, or after long
// enough that a reader would want to see the fault is still there.
pub(super) const STAGE_ERR_REPEAT: std::time::Duration = std::time::Duration::from_secs(600);

pub(super) fn log_staged_once(err: &str) {
    let now = std::time::Instant::now();
    let mut last = LAST_STAGE_ERR.lock().unwrap();
    let repeat = match last.as_ref() {
        Some((seen, at)) => seen != err || now.duration_since(*at) >= STAGE_ERR_REPEAT,
        None => true,
    };
    if repeat {
        log::error!("Wayland portal handshake failed: {}", err);
        *last = Some((err.to_owned(), now));
    }
}

pub(super) fn try_log(err: &String) {
    let mut lock_count = LOG_SCRAP_COUNT.lock().unwrap();
    if *lock_count >= 1000000 {
        return;
    }
    if *lock_count % 10000 == 0 {
        log::error!("Failed scrap {}", err);
    }
    *lock_count += 1;
}
