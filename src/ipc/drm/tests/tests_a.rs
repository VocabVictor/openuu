use super::*;

// Added to the wire later: an older peer's message must still decode.
#[test]
pub(super) fn drm_display_info_decodes_without_render_node() {
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

pub(super) fn pipe() -> (OwnedFd, OwnedFd) {
    let mut fds = [0 as libc::c_int; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0, "pipe() failed");
    unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) }
}

pub(super) unsafe fn send_with_fds(sock: libc::c_int, data: &[u8], fds: &[libc::c_int]) -> isize {
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
pub(super) async fn roundtrip_msg_no_fd() {
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
pub(super) async fn roundtrip_msg_with_fd_identity() {
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
pub(super) async fn roundtrip_raw_body() {
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
pub(super) async fn rejects_oversized_length_prefix() {
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
pub(super) async fn a_body_that_never_arrives_times_out() {
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
pub(super) async fn a_dripping_peer_cannot_re_arm_the_send_deadline() {
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
pub(super) async fn surplus_fds_keep_only_the_first() {
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
