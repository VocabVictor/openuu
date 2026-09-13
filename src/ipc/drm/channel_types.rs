use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct DrmDisplayInfo {
    pub name: String,
    pub crtc_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub active: bool,
    /// Render node of the GPU that EXPORTS this display's scanout; on a multi-GPU host auto-select
    /// can bind a different GPU whose cross-vendor import then fails. Empty when the service cannot
    /// name it: the consumer then auto-selects on a single-render-node host, and forces the CPU
    /// path where there are several.
    #[serde(default)]
    pub render_node: String,
    /// KMS card node (`/dev/dri/card*`) driving this display. crtc_ids are card-local, so the index
    /// alone is ambiguous across cards. Empty = the single auto-detected device.
    #[serde(default)]
    pub device: String,
}

/// Mirrors `scrap::drm_reader::drmtap_dmabuf_desc` except `dma_buf_fd` (never serializes — it rides
/// SCM_RIGHTS ancillary), and adds `buffer_id` (fb_id tagged with a per-connection epoch; no consumer reads it today) and `has_fd`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DmabufDesc {
    pub buffer_id: u64,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    /// KMS framebuffer id — libdrmtap's import-once cache key. 0 disables caching for this frame.
    pub fb_id: u32,
    /// Used entries in `offsets`/`pitches` (1..4); 0 is treated as 1.
    pub num_planes: u32,
    pub offsets: [u32; 4],
    pub pitches: [u32; 4],
    /// DRMTAP_EOTF_* (SDR=0, PQ=2, HLG=3). PQ triggers the HDR->SDR tone-map on convert.
    pub hdr_eotf: u32,
    pub hdr_max_nits: u32,
    /// True: the fd rides this message's SCM_RIGHTS cmsg. False: import-once cache hit for `fb_id`.
    pub has_fd: bool,
}

pub(crate) fn drm_ipc_path() -> String {
    let service_path = Config::ipc_path("_service");
    let dir = std::path::Path::new(&service_path)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("/tmp"));
    dir.join("ipc_drm").to_string_lossy().into_owned()
}

pub(crate) async fn connect_drm(ms_timeout: u64) -> ResultType<DrmConn> {
    use std::os::fd::AsRawFd;
    let path = drm_ipc_path();
    let stream = timeout(ms_timeout, tokio::net::UnixStream::connect(&path)).await??;
    // The producer MUST be root: a non-root peer that won a socket-path race must not be trusted to
    // supply the display list, frames and an arbitrary dma-buf fd.
    if peer_uid_from_fd(stream.as_raw_fd()) != Some(0) {
        bail!("drm: _drm producer is not root; refusing to consume");
    }
    Ok(DrmConn::new(stream))
}

/// Bind the `_drm` listener 0666: connectable by any local uid, authorized in `handle_drm_conn`.
pub(super) fn new_drm_listener() -> ResultType<Incoming> {
    let path = drm_ipc_path();
    let _ = ensure_secure_ipc_parent_dir(&path, "_service")?;
    // NOT `std::fs::remove_file`: `unlink(2)` returns EISDIR against a directory-typed squatter and
    // the bind then fails EADDRINUSE; the fd-based helper picks `AT_REMOVEDIR` (empty dirs only).
    if let Err(err) = remove_ipc_entry_via_secure_parent_fd(&path) {
        log::warn!("drm: could not clear a stale entry at {}: {}", &path, err);
    }
    let mut endpoint = Endpoint::new(path.clone());
    endpoint.set_security_attributes(SecurityAttributes::allow_everyone_create()?);
    let incoming = endpoint.incoming()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666)).map_err(|err| {
        std::fs::remove_file(&path).ok();
        err
    })?;
    log::info!("Started drm ipc server at path: {}", &path);
    Ok(incoming)
}

pub(super) enum DrmProducerMsg {
    /// Enumerated displays, sent once before any frame.
    Displays(Vec<DrmDisplayInfo>),
    /// Zero-copy path: descriptor + scanout fd; the `OwnedFd` is closed once the send has dup'd it.
    Frame {
        desc: DmabufDesc,
        fd: Option<OwnedFd>,
    },
    /// CPU-mapped fallback (packed BGRA): consumer has no convert context (`need_cpu`), or ENOTSUP.
    FrameCpu {
        width: u32,
        height: u32,
        data: Bytes,
    },
    Cursor {
        id: u64,
        width: u32,
        height: u32,
        hotx: i32,
        hoty: i32,
        colors: Vec<u8>,
    },
}

pub(super) struct DrmStopGuard(pub(super) std::sync::Arc<std::sync::atomic::AtomicBool>);
impl Drop for DrmStopGuard {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

pub(super) fn dup_to_drm_conn(stream: &Connection) -> ResultType<DrmConn> {
    let raw = stream.inner.get_ref().as_raw_fd();
    // F_DUPFD_CLOEXEC, not dup(): `dup` never copies close-on-exec, and this process forks (the
    // `loginctl` lookup), so an already-authorized `_drm` socket would leak into children.
    let dup = unsafe { hbb_common::libc::fcntl(raw, hbb_common::libc::F_DUPFD_CLOEXEC, 0) };
    if dup < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: `dup` is a freshly dup'd, owned fd for a connected SOCK_STREAM unix socket.
    let std_stream = unsafe { std::os::unix::net::UnixStream::from_raw_fd(dup) };
    std_stream.set_nonblocking(true)?;
    let tokio_stream = tokio::net::UnixStream::from_std(std_stream)?;
    Ok(DrmConn::new(tokio_stream))
}
