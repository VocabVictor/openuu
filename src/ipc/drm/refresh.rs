use super::*;

/// The SINGLE writer of DRM_DISPLAY_CACHE (+ DRM_DISPLAY_GENERATION), off the caller's thread and
/// SINGLE-FLIGHT: a request arriving during a run coalesces into exactly one follow-up.
pub(super) fn schedule_drm_cache_refresh() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static RUNNING: AtomicBool = AtomicBool::new(false);
    static PENDING: AtomicBool = AtomicBool::new(false);
    // Ownership of RUNNING, released on every exit incl. unwind and failed spawn; re-taken mid-loop.
    struct RefreshSlot(bool);
    impl RefreshSlot {
        fn release(&mut self) {
            if self.0 {
                self.0 = false;
                RUNNING.store(false, Ordering::Release);
            }
        }
        fn retake(&mut self) -> bool {
            self.0 = !RUNNING.swap(true, Ordering::AcqRel);
            self.0
        }
    }
    impl Drop for RefreshSlot {
        fn drop(&mut self) {
            self.release();
        }
    }
    // Announce a refresh is wanted before trying to run, so an active worker is guaranteed to see it.
    PENDING.store(true, Ordering::Release);
    if RUNNING.swap(true, Ordering::AcqRel) {
        return; // a worker is already active; it will observe PENDING and refresh again
    }
    let mut slot = RefreshSlot(true);
    let spawned = std::thread::Builder::new()
        .name("drm-cache-refresh".into())
        .spawn(move || loop {
            PENDING.store(false, Ordering::Release);
            let fresh = std::panic::catch_unwind(drm_enumerate_all_displays)
                .unwrap_or_else(|_| {
                    log::error!("drm: display enumeration panicked; treating as no displays");
                    (Vec::new(), Vec::new())
                })
                .0;
            let changed = {
                let mut cache = match DRM_DISPLAY_CACHE.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                if *cache != fresh {
                    *cache = fresh;
                    true
                } else {
                    false
                }
            };
            if changed {
                DRM_DISPLAY_GENERATION.fetch_add(1, Ordering::Release);
                log::info!("drm: display cache refreshed (topology changed)");
            }
            // Exit only if no request arrived during this enumeration. The re-check after releasing
            // the slot closes the lost-wakeup window (a request that set PENDING just before it).
            if !PENDING.load(Ordering::Acquire) {
                slot.release();
                if !PENDING.load(Ordering::Acquire) {
                    break;
                }
                if !slot.retake() {
                    break; // another caller re-acquired the slot; it will handle the pending refresh
                }
            }
        });
    if let Err(err) = spawned {
        log::error!("drm: could not spawn the display-cache refresh worker: {err}");
    }
}

pub(super) fn uevent_is_drm_change(msg: &[u8]) -> bool {
    let mut is_drm = false;
    let mut is_change = false;
    for rec in msg.split(|&b| b == 0) {
        if rec == b"SUBSYSTEM=drm" {
            is_drm = true;
        } else if rec == b"ACTION=change" || rec == b"HOTPLUG=1" {
            is_change = true;
        }
    }
    is_drm && is_change
}

/// Refresh the display cache on DRM hotplug uevents (raw NETLINK_KOBJECT_UEVENT, no libudev).
pub(super) fn drm_udev_listener() {
    use hbb_common::libc;

    let sock = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC,
            libc::NETLINK_KOBJECT_UEVENT,
        )
    };
    if sock < 0 {
        log::info!(
            "drm: udev uevent socket unavailable ({}); hotplug refresh disabled",
            std::io::Error::last_os_error()
        );
        return;
    }
    let _owned = unsafe { OwnedFd::from_raw_fd(sock) };
    let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
    addr.nl_family = libc::AF_NETLINK as u16;
    // Group 1 = kernel-originated uevents (udev re-broadcasts on group 2); pid 0 => kernel assigns.
    addr.nl_groups = 1;
    let rc = unsafe {
        libc::bind(
            sock,
            &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
        )
    };
    if rc < 0 {
        log::info!(
            "drm: udev uevent bind failed ({}); hotplug refresh disabled",
            std::io::Error::last_os_error()
        );
        return;
    }
    log::info!("drm: udev DRM-uevent listener started");
    let mut buf = [0u8; 8192];
    loop {
        // recvmsg, not recv: a local process could UNICAST a spoofed uevent to this root listener.
        let mut src: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        let mut iov = libc::iovec {
            iov_base: buf.as_mut_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };
        let mut mhdr: libc::msghdr = unsafe { std::mem::zeroed() };
        mhdr.msg_name = &mut src as *mut libc::sockaddr_nl as *mut libc::c_void;
        mhdr.msg_namelen = std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        mhdr.msg_iov = &mut iov;
        mhdr.msg_iovlen = 1;
        let n = unsafe { libc::recvmsg(sock, &mut mhdr, 0) };
        if n <= 0 {
            let err = std::io::Error::last_os_error();
            if n < 0 && err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            log::info!("drm: udev uevent recv ended ({err}); hotplug refresh stopped");
            break;
        }
        if (mhdr.msg_namelen as usize) < std::mem::size_of::<libc::sockaddr_nl>()
            || src.nl_pid != 0
            || src.nl_groups == 0
        {
            continue;
        }
        if !uevent_is_drm_change(&buf[..n as usize]) {
            continue;
        }
        schedule_drm_cache_refresh();
    }
}

pub(super) fn drm_prewarm() {
    // Re-ask, bounded: `get_display_server()` falls back to "x11" when it cannot tell (measured:
    // "x11" 0.8 s into a boot on a Wayland host). `is_x11_for_drm()` is that path minus the
    // greeter blind spot, which a login screen never leaves.
    pub(super) const PREWARM_SESSION_RECHECK: std::time::Duration = std::time::Duration::from_secs(2);
    pub(super) const PREWARM_SESSION_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
    let waited = std::time::Instant::now();
    while crate::platform::linux::is_x11_for_drm() {
        if waited.elapsed() >= PREWARM_SESSION_BUDGET {
            log::info!(
                "drm: session still reads as X11 after {:?}; skipping the pre-warm \
                 (the _drm listener still runs)",
                PREWARM_SESSION_BUDGET
            );
            return;
        }
        std::thread::sleep(PREWARM_SESSION_RECHECK);
    }
    let t = std::time::Instant::now();
    schedule_drm_cache_refresh();
    match scrap::drm_reader::DrmReader::open(None, 0) {
        Some(mut r) => {
            // grab_desc(), not grab(): exports an fd without loading libEGL into the root service.
            if let Ok((fd, _desc)) = r.grab_desc() {
                drop(fd); // close the warm-up fd; we only wanted to prime the device/import path
            }
            log::info!("drm: pre-warm framebuffer primed in {:?}", t.elapsed());
        }
        None => log::info!("drm: pre-warm skipped (no reader; cache refresh requested)"),
    }
}
