use super::*;

// 16 fds need CMSG_LEN(64)=80 > the 64-byte DRM_CMSG_CAP, so the kernel sets MSG_CTRUNC.
#[tokio::test]
pub(super) async fn rejects_truncated_control_message() {
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
pub(super) fn peer_uid_from_fd_reads_socket_peer() {
    let (a, _b) = std::os::unix::net::UnixStream::pair().unwrap();
    let euid = unsafe { libc::geteuid() };
    assert_eq!(peer_uid_from_fd(a.as_raw_fd()), Some(euid));
}

#[test]
pub(super) fn drm_peer_authorized_matrix() {
    assert!(drm_peer_authorized(Some(0), Some(1000)));
    assert!(drm_peer_authorized(Some(0), None));
    assert!(drm_peer_authorized(Some(1000), Some(1000)));
    assert!(!drm_peer_authorized(Some(1000), Some(1001)));
    assert!(!drm_peer_authorized(Some(1000), None));
    assert!(!drm_peer_authorized(None, Some(1000)));
    assert!(!drm_peer_authorized(None, None));
}

#[test]
pub(super) fn accept_time_exe_match_accepts_only_our_own_executable() {
    let me = std::process::id();
    assert!(
        super::super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(me), "_drm").is_ok(),
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
        super::super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(Some(other.id()), "_drm")
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

    assert!(super::super::ipc_auth::ensure_peer_executable_matches_current_by_pid_opt(None, "_drm").is_err());
}

#[test]
pub(super) fn drm_conn_admission_bound() {
    assert!(drm_conn_admitted(0));
    assert!(drm_conn_admitted(MAX_DRM_CONNS - 1)); // last admitted slot
    assert!(!drm_conn_admitted(MAX_DRM_CONNS)); // cap reached -> rejected
    assert!(!drm_conn_admitted(MAX_DRM_CONNS + 5)); // over cap -> rejected
}

#[test]
pub(super) fn drm_auth_admission_bound() {
    assert!(drm_auth_admitted(0));
    assert!(drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT - 1)); // last admitted slot
    assert!(!drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT)); // cap reached -> rejected
    assert!(!drm_auth_admitted(MAX_DRM_AUTH_IN_FLIGHT + 5)); // over cap -> rejected
    assert!(
        MAX_DRM_AUTH_IN_FLIGHT <= MAX_DRM_CONNS,
        "the pre-auth bound must not be looser than the connection cap"
    );
}
