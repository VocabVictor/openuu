// The DRM/KMS capture half of the `_drm` IPC channel: types, root-service producer, framing.

use super::ipc_auth::active_uid_cached;
use super::*;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

mod channel_types;
pub use channel_types::*;
mod display_cache;
use display_cache::*;
mod wake;
use wake::*;
mod refresh;
use refresh::*;
mod start;
pub use start::*;
mod conn_handler;
use conn_handler::*;
mod capture_worker;
use capture_worker::*;
mod framing;
pub(crate) use framing::*;

impl DrmConn {
    pub fn new(stream: tokio::net::UnixStream) -> Self {
        Self {
            stream,
            read_buf: Vec::new(),
            consumed: false,
        }
    }

    pub async fn send_msg(&mut self, data: &Data, fd: Option<BorrowedFd<'_>>) -> ResultType<()> {
        let payload = serde_json::to_vec(data)?;
        let pass_fd = fd.map(|f| f.as_raw_fd());
        drm_send_frame(&self.stream, &payload, pass_fd).await
    }

    pub async fn send_frame_ack(&self) -> ResultType<()> {
        let deadline =
            tokio::time::Instant::now() + std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS);
        loop {
            match tokio::time::timeout_at(deadline, self.stream.writable()).await {
                Ok(r) => r?,
                Err(_) => bail!(
                    "drm: _drm frame-ack was not accepted within {DRM_SEND_TIMEOUT_MS}ms; closing"
                ),
            }
            match self.stream.try_write(&[1u8]) {
                Ok(n) if n > 0 => return Ok(()),
                Ok(_) => bail!("drm: _drm frame-ack write returned 0 (peer closed)"),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }

    pub fn drain_frame_acks(&self, credit: &mut i32, max: i32) -> ResultType<()> {
        let mut buf = [0u8; 64];
        // BOUNDED: "until WouldBlock" is the peer's promise; a continuous writer would pin us.
        const MAX_ACK_READS: usize = 64;
        for _ in 0..MAX_ACK_READS {
            match self.stream.try_read(&mut buf) {
                Ok(0) => bail!("drm: _drm frame-ack peer closed"),
                Ok(n) => {
                    *credit = (*credit + n as i32).min(max);
                    if *credit >= max {
                        return Ok(());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e.into()),
            }
        }
        Ok(())
    }

    pub async fn wait_readable(&self) -> ResultType<()> {
        self.stream.readable().await?;
        Ok(())
    }

    pub async fn recv_msg(&mut self) -> ResultType<(Data, Option<OwnedFd>)> {
        self.consumed = false;
        let mut prefix = [0u8; 4];
        let fd = drm_read_full(&self.stream, &mut prefix, true, &mut self.consumed).await?;
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_JSON_BYTES {
            bail!("drm: message length {len} exceeds cap {MAX_DRM_JSON_BYTES}");
        }
        if self.read_buf.len() < len {
            self.read_buf.resize(len, 0);
        }
        drm_read_full(&self.stream, &mut self.read_buf[..len], false, &mut self.consumed).await?;
        let data: Data = serde_json::from_slice(&self.read_buf[..len])?;
        Ok((data, fd))
    }

    /// Cancel-safe timeout wrapper around `recv_msg`. `None` = nothing consumed, so re-polling is
    /// safe; past the first byte the frame is committed and an overrun is a hard error.
    pub async fn recv_msg_timeout2(
        &mut self,
        ms_timeout: u64,
    ) -> Option<ResultType<(Data, Option<OwnedFd>)>> {
        let ready = timeout(ms_timeout, self.stream.readable()).await;
        match ready {
            Err(_) => None, // no frame started: clean boundary, caller re-checks `stop`
            Ok(Err(e)) => Some(Err(e.into())),
            Ok(Ok(())) => match timeout(ms_timeout, self.recv_msg()).await {
                Ok(res) => Some(res),
                Err(_) if self.consumed => Some(Err(anyhow::anyhow!(
                    "drm: frame body stalled past {ms_timeout}ms after first byte; closing"
                ))),
                Err(_) => None,
            },
        }
    }

    pub async fn send_raw(&mut self, data: Bytes) -> ResultType<()> {
        drm_send_frame(&self.stream, &data, None).await
    }

    pub async fn next_raw_into(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
        match timeout(DRM_BODY_TIMEOUT_MS, self.next_raw_into_unbounded(out)).await {
            Ok(res) => res,
            Err(_) => bail!(
                "drm: raw body did not arrive within {DRM_BODY_TIMEOUT_MS}ms of its header; closing"
            ),
        }
    }

    async fn next_raw_into_unbounded(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
        let mut prefix = [0u8; 4];
        if drm_read_full(&self.stream, &mut prefix, true, &mut self.consumed)
            .await?
            .is_some()
        {
            log::warn!("drm: unexpected fd on a raw-body frame; dropping");
        }
        let len = u32::from_be_bytes(prefix) as usize;
        if len > MAX_DRM_RAW_BYTES {
            bail!("drm: raw body length {len} exceeds cap {MAX_DRM_RAW_BYTES}");
        }
        out.resize(len, 0);
        drm_read_full(&self.stream, &mut out[..], false, &mut self.consumed).await?;
        Ok(())
    }
}

#[cfg(test)]
mod drm_conn_tests {
    use super::*;
    use hbb_common::libc;
    use hbb_common::tokio::{self, io::AsyncWriteExt};
    use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

    // Added to the wire later: an older peer's message must still decode.
    #[test]
    fn drm_display_info_decodes_without_render_node() {
        let legacy = r#"{"name":"DP-1","crtc_id":386,"x":0,"y":0,
                         "width":3840,"height":2160,"active":true}"#;
        let info: DrmDisplayInfo =
            serde_json::from_str(legacy).expect("a pre-render_node payload must still decode");
        assert_eq!(info.name, "DP-1");
        assert_eq!(info.crtc_id, 386);
        assert!(info.render_node.is_empty(), "missing node; the consumer auto-selects only where there is one render node");
        assert!(info.device.is_empty(), "missing device means auto-detect");

        let current = DrmDisplayInfo {
            name: "DP-1".to_owned(),
            crtc_id: 386,
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
            active: true,
            render_node: "/dev/dri/renderD129".to_owned(),
            device: "/dev/dri/card2".to_owned(),
        };
        let wire = serde_json::to_vec(&current).unwrap();
        let back: DrmDisplayInfo = serde_json::from_slice(&wire).unwrap();
        assert_eq!(back, current);
    }

    fn pipe() -> (OwnedFd, OwnedFd) {
        let mut fds = [0 as libc::c_int; 2];
        assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe() failed");
        unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
    }

    unsafe fn send_with_fds(sock: libc::c_int, data: &[u8], fds: &[libc::c_int]) -> isize {
        let mut iov = libc::iovec {
            iov_base: data.as_ptr() as *mut libc::c_void,
            iov_len: data.len(),
        };
        let fdbytes = fds.len() * std::mem::size_of::<libc::c_int>();
        let space = libc::CMSG_SPACE(fdbytes as u32) as usize;
        let mut cbuf = vec![0u8; space];
        let mut msg: libc::msghdr = std::mem::zeroed();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = cbuf.as_mut_ptr() as *mut libc::c_void;
        msg.msg_controllen = space as _;
        let cmsg = libc::CMSG_FIRSTHDR(&msg);
        (*cmsg).cmsg_level = libc::SOL_SOCKET;
        (*cmsg).cmsg_type = libc::SCM_RIGHTS;
        (*cmsg).cmsg_len = libc::CMSG_LEN(fdbytes as u32) as _;
        std::ptr::copy_nonoverlapping(fds.as_ptr() as *const u8, libc::CMSG_DATA(cmsg), fdbytes);
        libc::sendmsg(sock, &msg, 0)
    }

    #[tokio::test]
    async fn roundtrip_msg_no_fd() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut tx = DrmConn::new(a);
        let mut rx = DrmConn::new(b);
        tx.send_msg(&Data::DrmFrame { width: 1920, height: 1080 }, None)
            .await
            .unwrap();
        let (data, fd) = rx.recv_msg().await.unwrap();
        assert!(matches!(
            data,
            Data::DrmFrame {
                width: 1920,
                height: 1080
            }
        ));
        assert!(fd.is_none(), "no fd was sent, none must be reported");
    }

    #[tokio::test]
    async fn roundtrip_msg_with_fd_identity() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut tx = DrmConn::new(a);
        let mut rx = DrmConn::new(b);
        let (rd, wr) = pipe();
        tx.send_msg(&Data::DrmFrame { width: 4, height: 4 }, Some(rd.as_fd()))
            .await
            .unwrap();
        let (_data, fd) = rx.recv_msg().await.unwrap();
        let recv_fd = fd.expect("an fd was attached, it must be received");
        let sentinel = [0xABu8];
        assert_eq!(
            unsafe { libc::write(wr.as_raw_fd(), sentinel.as_ptr() as *const libc::c_void, 1) },
            1
        );
        let mut got = [0u8; 1];
        assert_eq!(
            unsafe { libc::read(recv_fd.as_raw_fd(), got.as_mut_ptr() as *mut libc::c_void, 1) },
            1
        );
        assert_eq!(got[0], 0xAB, "received fd must be the same pipe");
    }

    #[tokio::test]
    async fn roundtrip_raw_body() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut tx = DrmConn::new(a);
        let mut rx = DrmConn::new(b);
        let body = Bytes::from(vec![7u8; 5000]);
        tx.send_raw(body.clone()).await.unwrap();
        let mut got = Vec::new();
        rx.next_raw_into(&mut got).await.unwrap();
        assert_eq!(&got[..], &body[..]);
        let short = Bytes::from(vec![9u8; 10]);
        tx.send_raw(short.clone()).await.unwrap();
        rx.next_raw_into(&mut got).await.unwrap();
        assert_eq!(&got[..], &short[..]);
    }

    #[tokio::test]
    async fn rejects_oversized_length_prefix() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        let bogus = (MAX_DRM_JSON_BYTES as u32 + 1).to_be_bytes();
        a.write_all(&bogus).await.unwrap();
        let err = rx
            .recv_msg()
            .await
            .err()
            .expect("a length past the cap must be rejected");
        assert!(
            err.to_string().contains("exceeds cap"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn a_body_that_never_arrives_times_out() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        a.write_all(&10u32.to_be_bytes()).await.unwrap();
        let mut got = Vec::new();
        let err = rx
            .next_raw_into(&mut got)
            .await
            .err()
            .expect("a body that never arrives must time out");
        assert!(
            err.to_string().contains("did not arrive"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    async fn a_dripping_peer_cannot_re_arm_the_send_deadline() {
        use tokio::io::AsyncReadExt;
        let (mut reader, writer) = tokio::net::UnixStream::pair().unwrap();
        let payload = vec![0u8; 32 * 1024 * 1024];
        // Measured: 1 KiB drains do not re-assert POLLOUT; 64 KiB does, which separates the forms.
        let drip = tokio::spawn(async move {
            let mut sink = vec![0u8; 64 * 1024];
            loop {
                if reader.read(&mut sink).await.unwrap_or(0) == 0 {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        });
        let started = std::time::Instant::now();
        let outcome = tokio::time::timeout(
            std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS * 4),
            drm_write_all(&writer, &payload, None),
        )
        .await;
        drip.abort();
        let inner = outcome.expect(
            "the send deadline did not fire: the budget is being re-armed per readiness wait",
        );
        let err = inner.err().expect("a dripping peer must not complete the write");
        assert!(
            err.to_string().contains("did not accept the remaining"),
            "unexpected error: {err}"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_millis(DRM_SEND_TIMEOUT_MS * 3),
            "took {:?}, which is not the send deadline firing",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn surplus_fds_keep_only_the_first() {
        let (mut a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        let (rd, wr) = pipe();
        let (rd2, _wr2) = pipe();
        let payload = serde_json::to_vec(&Data::DrmFrame {
            width: 8,
            height: 8,
        })
        .unwrap();
        let prefix = (payload.len() as u32).to_be_bytes();
        let n = unsafe { send_with_fds(a.as_raw_fd(), &prefix, &[rd.as_raw_fd(), rd2.as_raw_fd()]) };
        assert!(n >= 0, "sendmsg failed: {}", std::io::Error::last_os_error());
        a.write_all(&payload).await.unwrap();
        let (data, fd) = rx.recv_msg().await.unwrap();
        assert!(matches!(
            data,
            Data::DrmFrame {
                width: 8,
                height: 8
            }
        ));
        let kept = fd.expect("the first surplus fd must be kept");
        let sentinel = [0x5Au8];
        assert_eq!(
            unsafe { libc::write(wr.as_raw_fd(), sentinel.as_ptr() as *const libc::c_void, 1) },
            1
        );
        let mut got = [0u8; 1];
        assert_eq!(
            unsafe { libc::read(kept.as_raw_fd(), got.as_mut_ptr() as *mut libc::c_void, 1) },
            1
        );
        assert_eq!(got[0], 0x5A, "the kept fd must be the FIRST one sent");
    }

    // 16 fds need CMSG_LEN(64)=80 > the 64-byte DRM_CMSG_CAP, so the kernel sets MSG_CTRUNC.
    #[tokio::test]
    async fn rejects_truncated_control_message() {
        let (a, b) = tokio::net::UnixStream::pair().unwrap();
        let mut rx = DrmConn::new(b);
        let (rd, _wr) = pipe();
        let dups: Vec<OwnedFd> = (0..16).map(|_| rd.try_clone().unwrap()).collect();
        let fds: Vec<libc::c_int> = dups.iter().map(|f| f.as_raw_fd()).collect();
        let prefix = 0u32.to_be_bytes(); // the fds ride the prefix read; CTRUNC fires before any body
        let n = unsafe { send_with_fds(a.as_raw_fd(), &prefix, &fds) };
        assert!(n >= 0, "sendmsg failed: {}", std::io::Error::last_os_error());
        let err = rx
            .recv_msg()
            .await
            .err()
            .expect("a truncated control message must be rejected");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("truncat") || msg.contains("ctrunc"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn peer_uid_from_fd_reads_socket_peer() {
        let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
        let euid = unsafe { libc::geteuid() };
        assert_eq!(peer_uid_from_fd(a.as_raw_fd()), Some(euid));
    }

    #[test]
    fn drm_peer_authorized_matrix() {
        assert!(drm_peer_authorized(Some(0), Some(1000)));
        assert!(drm_peer_authorized(Some(0), None));
        assert!(drm_peer_authorized(Some(1000), Some(1000)));
        assert!(!drm_peer_authorized(Some(1000), Some(1001)));
        assert!(!drm_peer_authorized(Some(1000), None));
        assert!(!drm_peer_authorized(None, Some(1000)));
        assert!(!drm_peer_authorized(None, None));
    }

    #[test]
    fn accept_time_exe_match_accepts_only_our_own_executable() {
        let me = std::process::id();
        assert!(
            super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(me), "_drm").is_ok(),
            "the test process must match its own executable"
        );

        let mut other = std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .expect("/bin/sleep should be spawnable in the test environment");
        // Until the child finishes exec'ing, /proc/<pid>/exe still points at OUR binary.
        let ours = std::fs::read_link(format!("/proc/{me}/exe")).ok();
        let peer_link = format!("/proc/{}/exe", other.id());
        let mut exec_done = false;
        for _ in 0..200 {
            match std::fs::read_link(&peer_link) {
                Ok(p) if Some(&p) != ours.as_ref() => {
                    exec_done = true;
                    break;
                }
                _ => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        }
        let res = if exec_done {
            super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(other.id()), "_drm")
        } else {
            Err(anyhow::anyhow!("child never exec'd; nothing was tested"))
        };
        let _ = other.kill();
        let _ = other.wait();
        assert!(exec_done, "the spawned child never exec'd, so the negative case was not exercised");
        assert!(
            res.is_err(),
            "a peer running another executable must be rejected, got {res:?}"
        );

        assert!(super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(None, "_drm").is_err());
    }

    #[test]
    fn drm_conn_admission_bound() {
        assert!(drm_conn_admitted(0));
        assert!(drm_conn_admitted(MAX_DRM_CONNS - 1)); // last admitted slot
        assert!(!drm_conn_admitted(MAX_DRM_CONNS)); // cap reached -> rejected
        assert!(!drm_conn_admitted(MAX_DRM_CONNS + 5)); // over cap -> rejected
    }

    #[test]
    fn drm_auth_admission_bound() {
        assert!(drm_auth_admitted(0));
        assert!(drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT - 1)); // last admitted slot
        assert!(!drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT)); // cap reached -> rejected
        assert!(!drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT + 5)); // over cap -> rejected
        assert!(
            MAX_DRM_AUTH_IN_FLIGHT <= MAX_DRM_CONNS,
            "the pre-auth bound must not be looser than the connection cap"
        );
    }
}
