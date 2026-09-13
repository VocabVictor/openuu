use super::*;

// Translation keys, so the key itself is the English text: an older peer that has never heard
// of them falls back to displaying the key and still reads as a sentence.
pub(super) const WAYLAND_DECLINED: &str = "The screen sharing request was declined on the remote device";
pub(super) const WAYLAND_TIMED_OUT: &str = "The screen sharing request timed out on the remote device";
pub(super) const WAYLAND_NO_SESSION: &str = "OpenUU cannot reach the desktop session on the remote device, check that a desktop session is running and that OpenUU can use it";
pub(super) const WAYLAND_UNSUPPORTED: &str = "The desktop portal on the remote device is missing a capability needed for screen sharing or remote control, its backend may not be installed";
pub(super) const WAYLAND_PIPEWIRE_HANDOVER: &str = "Screen sharing was approved on the remote device, but the PipeWire connection could not be opened";
pub(super) const WAYLAND_ENDED: &str =
    "The screen sharing request ended without completing on the remote device";
// The remedy the message it replaces used to carry, minus the link: this is the outcome
// rustdesk/rustdesk#8600 is about.
pub(super) const WAYLAND_NO_USABLE_SCREEN: &str = "OpenUU could not obtain a usable screen from the XDG Desktop Portal, the PipeWire library may be too old";
pub(super) const WAYLAND_GST_UNAVAILABLE: &str =
    "OpenUU could not load a GStreamer component needed for screen capture ({})";

pub(super) const WAYLAND_STAGE_TAG: &str = "wl-stage:";

// `translate()` on the peer strips the braces itself, so what goes on the wire is the key
// with the detail still *inside* the placeholder.
pub(super) fn with_detail(key: &str, detail: &str) -> String {
    key.replace("{}", &format!("{{{}}}", detail))
}

pub(super) fn is_ubuntu_before_21() -> bool {
    DISTRO.name.to_uppercase() == "Ubuntu".to_uppercase() && DISTRO.version_id < "21".to_owned()
}

/// Maps a `<stage>:<kind>:<detail>` tag from the portal handshake, see
/// `scrap::wayland::pipewire`, onto what to tell the peer. Everything the capture loop reports
/// carries no tag and keeps the legacy substring heuristics above.
pub(super) fn staged_message(tag: &str, ubuntu_before_21: bool) -> String {
    let mut parts = tag.splitn(3, ':');
    let stage = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    let detail = parts.next().unwrap_or_default().trim();

    // An outcome that says something about the machine is what the Ubuntu branch was written
    // for, so that branch keeps it. An outcome that says what a person did is a fact no distro
    // check can improve on.
    let of_the_machine = |msg: &str| {
        if ubuntu_before_21 {
            SCRAP_UBUNTU_HIGHER_REQUIRED.to_owned()
        } else {
            msg.to_owned()
        }
    };

    match (stage, kind) {
        (_, "declined") => WAYLAND_DECLINED.to_owned(),
        (_, "ended") => WAYLAND_ENDED.to_owned(),
        (_, "no-response") => WAYLAND_TIMED_OUT.to_owned(),
        ("streams", _) => of_the_machine(WAYLAND_NO_USABLE_SCREEN),
        ("gst-plugin", _) => of_the_machine(&with_detail(WAYLAND_GST_UNAVAILABLE, detail)),
        // The bus the portal lives on was never reached, so the portal has not been asked
        // anything yet and telling anyone to restart it would be a guess.
        ("session-bus", _) => of_the_machine(WAYLAND_NO_SESSION),
        // The portal answered `Start`, so the request was granted and the only thing left
        // was handing over the PipeWire connection. Whatever went wrong, it is not the
        // portal being unavailable -- it had just answered.
        ("open-pipewire-remote", _) => of_the_machine(WAYLAND_PIPEWIRE_HANDOVER),
        // The portal is there and answering; it just does not implement what was called,
        // which restarting it cannot fix.
        (_, "unsupported") => of_the_machine(WAYLAND_UNSUPPORTED),
        // Everything else is the portal not delivering, which is what this key already says --
        // and unlike a message of our own it carries the `systemctl --user restart` remedy.
        _ => of_the_machine(SCRAP_XDP_PORTAL_UNAVAILABLE),
    }
}
