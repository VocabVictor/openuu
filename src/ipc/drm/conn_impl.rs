use super::*;

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

    pub(super) async fn next_raw_into_unbounded(&mut self, out: &mut Vec<u8>) -> ResultType<()> {
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
