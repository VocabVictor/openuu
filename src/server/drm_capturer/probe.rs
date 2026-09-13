use super::*;

pub(super) enum ProbeState {
    Unknown,
    Unavailable(Instant),
    Available(Instant, Vec<DrmDisplayInfo>),
}

pub(super) static DRM_STATE: Mutex<ProbeState> = Mutex::new(ProbeState::Unknown);
pub(super) const NEGATIVE_TTL: Duration = Duration::from_secs(30);
pub(super) const POSITIVE_TTL: Duration = Duration::from_secs(15);

/// Runs on a throwaway thread: a nested `#[tokio::main]` panics if called from inside a runtime.
pub(super) fn query_displays() -> ResultType<Vec<DrmDisplayInfo>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("drm-query".into())
        .spawn(move || {
            let _ = tx.send(query_displays_async());
        })
        .map_err(|err| anyhow!("could not spawn the drm display query thread: {err}"))?;
    rx.recv_timeout(Duration::from_millis(HANDSHAKE_WAIT_MS))
        .map_err(|_| anyhow!("drm display query timed out"))?
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn query_displays_async() -> ResultType<Vec<DrmDisplayInfo>> {
    query_displays_inner().await
}

pub(super) async fn query_displays_inner() -> ResultType<Vec<DrmDisplayInfo>> {
    let mut conn = connect_drm(DRM_CONNECT_TIMEOUT_MS).await?;
    match conn.recv_msg_timeout2(DISPLAY_LIST_TIMEOUT_MS).await {
        Some(Ok((Data::DrmDisplayList(v), _fd))) => Ok(v),
        Some(Ok((other, _fd))) => Err(anyhow!("expected DrmDisplayList, got {:?}", other)),
        Some(Err(err)) => Err(err),
        None => Err(anyhow!("timed out waiting for DrmDisplayList")),
    }
}

pub(super) static DRM_PROBE_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) const DRM_PROBE_MAX_FAILURES: u32 = 5;
pub(super) static DRM_REFRESH_FAILURES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(super) const DRM_REFRESH_MAX_FAILURES: u32 = 3;
// Single-flight, so is_available() never calls query_displays() (~4s of IPC) holding DRM_STATE.
pub(super) static DRM_PROBE_IN_FLIGHT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Advanced by every publish, so a slow UNLOCKED probe can tell a newer verdict landed meanwhile.
pub(super) static DRM_STATE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// EVERY verdict change to DRM_STATE goes through here so the generation stays truthful; the TTL
    /// restamp in `refresh_available_async` is the one direct write.
#[inline]
pub(super) fn publish_probe_state(st: &mut ProbeState, next: ProbeState) {
    *st = next;
    DRM_STATE_GEN.fetch_add(1, Ordering::Release);
}

/// Releases DRM_PROBE_IN_FLIGHT on EVERY exit; a leaked release wedges all future probes.
pub(super) struct ProbeInFlightGuard;
impl Drop for ProbeInFlightGuard {
    fn drop(&mut self) {
        DRM_PROBE_IN_FLIGHT.store(false, Ordering::Release);
    }
}

/// Ownership of `UINPUT_REFRESH_BUSY`, released on every exit. It is handed back and re-taken
/// mid-loop, so releasing on drop unconditionally would clear a flag a REPLACEMENT worker owns.
pub(super) struct UinputRefreshGuard(pub(super) bool);
impl UinputRefreshGuard {
    pub(super) fn release(&mut self) {
        if self.0 {
            self.0 = false;
            UINPUT_REFRESH_BUSY.store(false, Ordering::Release);
        }
    }
    pub(super) fn retake(&mut self) -> bool {
        self.0 = !UINPUT_REFRESH_BUSY.swap(true, Ordering::AcqRel);
        self.0
    }
}
impl Drop for UinputRefreshGuard {
    fn drop(&mut self) {
        self.release();
    }
}
