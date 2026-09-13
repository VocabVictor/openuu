use super::*;

/// Capture producer in the ROOT `--service`: one task per consumer, reader on a worker thread.
#[tokio::main(flavor = "current_thread")]
pub async fn start_drm() {
    match new_drm_listener() {
        Ok(mut incoming) => {
            if let Err(err) = std::thread::Builder::new()
                .name("drm-prewarm".into())
                .spawn(drm_prewarm)
            {
                log::warn!("drm: could not spawn the pre-warm thread ({err}); skipping the warmup");
            }
            if let Err(err) = std::thread::Builder::new()
                .name("drm-udev".into())
                .spawn(drm_udev_listener)
            {
                log::warn!(
                    "drm: could not spawn the udev listener ({err}); a mid-session topology change \
                     will not be pushed, and consumers pick it up on their next handshake"
                );
            }
            loop {
                match incoming.next().await {
                    Some(Ok(stream)) => {
                        tokio::spawn(async move {
                            if let Err(err) = handle_drm_conn(Connection::new(stream)).await {
                                log::info!("drm ipc connection ended: {}", err);
                            }
                        });
                    }
                    Some(Err(err)) => log::error!("Couldn't get drm client: {:?}", err),
                    None => {
                        log::error!("drm ipc listener stream ended; stopping drm producer");
                        break;
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start drm ipc server: {}", err);
        }
    }
}

pub(super) const MAX_DRM_CONNS: usize = 8;

pub(super) fn drm_conn_admitted(prev_count: usize) -> bool {
    prev_count < MAX_DRM_CONNS
}

pub(super) const MAX_DRM_AUTH_IN_FLIGHT: usize = 4;

pub(super) fn drm_auth_admitted(prev_in_flight: usize) -> bool {
    prev_in_flight < MAX_DRM_AUTH_IN_FLIGHT
}

pub(super) fn drm_peer_authorized(peer_uid: Option<u32>, active_uid: Option<u32>) -> bool {
    match peer_uid {
        Some(0) => true,
        Some(uid) => active_uid == Some(uid),
        None => false,
    }
}
