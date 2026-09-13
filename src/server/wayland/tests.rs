use super::*;

#[test]
fn staged_message_names_the_outcome() {
    let m = |tag| staged_message(tag, false);
    assert_eq!(m("start:declined:"), WAYLAND_DECLINED);
    assert_eq!(m("start:ended:"), WAYLAND_ENDED);
    assert_eq!(m("start:no-response:"), WAYLAND_TIMED_OUT);
    // A restored session shows no picker at all, so a timeout anywhere is a timeout and
    // never a claim about someone not answering.
    assert_eq!(m("create-session:no-response:"), WAYLAND_TIMED_OUT);
    assert_eq!(m("streams:empty:"), WAYLAND_NO_USABLE_SCREEN);
}

#[test]
fn only_a_portal_that_may_be_dead_is_told_to_restart() {
    let m = |tag| staged_message(tag, false);
    // Not reached the bus at all: the portal has not been asked anything yet.
    assert_eq!(
        m("session-bus:dbus:org.freedesktop.DBus.Error.NotSupported"),
        WAYLAND_NO_SESSION
    );
    // Answering, but without an implementation behind the interface that was called --
    // at any stage, not just the first one.
    assert_eq!(
        m("create-session:unsupported:org.freedesktop.DBus.Error.UnknownMethod"),
        WAYLAND_UNSUPPORTED
    );
    assert_eq!(
        m("select-sources:unsupported:org.freedesktop.DBus.Error.UnknownMethod"),
        WAYLAND_UNSUPPORTED
    );
    // Absent or silent, which is what the existing key's remedy is for.
    assert_eq!(
        m("create-session:dbus:org.freedesktop.DBus.Error.ServiceUnknown"),
        SCRAP_XDP_PORTAL_UNAVAILABLE
    );
    // Not this one: `Start` had already been answered, so the portal was alive and the
    // request granted. Saying it may have crashed would walk the diagnosis backwards.
    assert_eq!(
        m("open-pipewire-remote:dbus:org.freedesktop.DBus.Error.Failed"),
        WAYLAND_PIPEWIRE_HANDOVER
    );
    // A tag this build does not know must never fall back to a guess.
    assert_eq!(
        m("some-new-stage:some-new-kind:x"),
        SCRAP_XDP_PORTAL_UNAVAILABLE
    );
    assert_eq!(m(""), SCRAP_XDP_PORTAL_UNAVAILABLE);
}

// The peer resolves a message by replacing its first `{...}` with `{}` and looking that
// up, so a detail-carrying message has to reduce back to its key exactly.
#[test]
fn a_detail_carrying_message_reduces_back_to_its_key() {
    let gst = staged_message("gst-plugin:unavailable:pipewiresrc", false);
    assert_eq!(
        gst,
        "OpenUU could not load a GStreamer component needed for screen capture ({pipewiresrc})"
    );
    let open = gst.find('{').expect("no placeholder");
    let close = gst[open..].find('}').expect("unclosed placeholder") + open;
    assert_eq!(
        format!("{}{{}}{}", &gst[..open], &gst[close + 1..]),
        WAYLAND_GST_UNAVAILABLE
    );
}

#[test]
fn legacy_ubuntu_keeps_its_message_for_machine_faults_only() {
    let m = |tag| staged_message(tag, true);
    assert_eq!(
        m("create-session:dbus:org.freedesktop.DBus.Error.ServiceUnknown"),
        SCRAP_UBUNTU_HIGHER_REQUIRED
    );
    assert_eq!(
        m("create-session:unsupported:org.freedesktop.DBus.Error.UnknownMethod"),
        SCRAP_UBUNTU_HIGHER_REQUIRED
    );
    assert_eq!(
        m("gst-plugin:unavailable:pipewiresrc"),
        SCRAP_UBUNTU_HIGHER_REQUIRED
    );
    assert_eq!(m("streams:empty:"), SCRAP_UBUNTU_HIGHER_REQUIRED);
    assert_eq!(m("session-bus:dbus:"), SCRAP_UBUNTU_HIGHER_REQUIRED);
    assert_eq!(m("start:declined:"), WAYLAND_DECLINED);
    assert_eq!(m("start:ended:"), WAYLAND_ENDED);
    assert_eq!(m("start:no-response:"), WAYLAND_TIMED_OUT);
}
