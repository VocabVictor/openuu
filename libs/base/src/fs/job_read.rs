use super::*;

impl TransferJob {
    pub async fn prefetch_digests(&mut self) -> ResultType<Vec<FileTransferDigest>> {
        let mut result = Vec::new();
        if self.paused || self.is_resume || !self.enable_overwrite_detection || self.confirmation_window == 0 { return Ok(result); }
        let current = self.file_num;
        self.prefetched.retain(|number| *number >= current);
        self.confirmations.retain(|number, _| *number >= current);
        let DataSource::FilePath(root) = &self.data_source else { return Ok(result); };
        let start = self.file_num.max(0) as usize + 1;
        for number in start..(start + self.confirmation_window).min(self.files.len()) {
            if self.prefetched.contains(&(number as i32)) { continue; }
            let meta = match tokio::fs::metadata(Self::join(root, &self.files[number].name)).await {
                Ok(meta) => meta,
                Err(_) => break, // Report source errors in normal file order.
            };
            if meta.len() > 64 * 1024 { break; }
            let modified = match meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()) {
                Some(time) => time.as_secs(),
                None => break,
            };
            self.prefetched.insert(number as i32);
            result.push(FileTransferDigest { id: self.id, file_num: number as i32,
                file_size: meta.len(), last_modified: modified, ..Default::default() });
        }
        Ok(result)
    }

    /// Initialize data stream for CM (Connection Manager) scenario.
    /// Returns digest info (last_modified, file_size) if overwrite detection is enabled,
    /// so caller can send it via IPC instead of network stream.
    /// Returns Ok(None) if job is done or already initialized.
    pub async fn init_data_stream_for_cm(&mut self) -> ResultType<Option<(u64, u64)>> {
        loop {
            if self.open_data_stream().await? { return Ok(None); }
            if let Some(confirm) = self.confirmations.remove(&self.file_num) {
                let previous = self.file_num;
                self.confirm(&confirm).await;
                if self.file_num != previous { continue; }
            }
            break;
        }
        // For overwrite detection, return digest info instead of sending via stream
        if self.r#type == JobType::Generic
            && self.enable_overwrite_detection
            && !self.file_confirmed()
            && !self.file_is_waiting()
        {
            self.set_file_is_waiting(true);
            if self.prefetched.contains(&self.file_num) { return Ok(None); }
            let digest = self.get_current_digest().await?;
            return Ok(Some(digest));
        }
        Ok(None)
    }

    pub async fn read(&mut self) -> ResultType<Option<FileTransferBlock>> {
        if self.r#type == JobType::Generic {
            if self.enable_overwrite_detection && !self.file_confirmed() {
                return Ok(None);
            }
        }

        let file_num = self.file_num as usize;
        let name = match &self.data_source {
            DataSource::FilePath(p) => {
                if file_num >= self.files.len() {
                    self.data_stream.take();
                    return Ok(None);
                };
                if self.files.len() == 1 && self.files[file_num].name.is_empty() {
                    p.file_name()
                        .map(|p| p.to_str().unwrap_or(""))
                        .unwrap_or("")
                } else {
                    &self.files[file_num].name
                }
            }
            DataSource::MemoryCursor(..) => "",
        };
        const BUF_SIZE: usize = 128 * 1024;
        // Small files should not allocate and zero a full transfer block.
        // A stale size only changes the chunk size; reads still continue to EOF.
        let buffer_size = if matches!(self.data_source, DataSource::FilePath(_)) {
            self.files[file_num].size.min(BUF_SIZE as u64).max(4096) as usize
        } else {
            BUF_SIZE
        };
        let mut buf: Vec<u8> = vec![0; buffer_size];
        let mut compressed = false;
        let mut offset: usize = 0;
        loop {
            match self
                .data_stream
                .as_mut()
                .ok_or(anyhow!("data stream is None"))?
                .read(&mut buf[offset..])
                .await
            {
                Err(err) => {
                    self.file_num += 1;
                    self.data_stream = None;
                    self.file_confirmed = false;
                    self.file_is_waiting = false;
                    return Err(err.into());
                }
                Ok(n) => {
                    offset += n;
                    if n == 0 || offset == buffer_size {
                        break;
                    }
                }
            }
        }
        unsafe { buf.set_len(offset) };
        if offset == 0 {
            if matches!(self.data_source, DataSource::MemoryCursor(_)) {
                self.data_stream.take();
                return Ok(None);
            }
            self.file_num += 1;
            self.data_stream = None;
            self.file_confirmed = false;
            self.file_is_waiting = false;
        } else {
            self.finished_size += offset as u64;
            if matches!(self.data_source, DataSource::FilePath(_)) && !is_compressed_file(name) {
                if let Some(tmp) = self.compression.encode(&buf) {
                    buf = tmp;
                    compressed = true;
                }
            }
            self.transferred += buf.len() as u64;
        }
        Ok(Some(FileTransferBlock {
            id: self.id,
            file_num: file_num as _,
            data: buf.into(),
            compressed,
            ..Default::default()
        }))
    }

    // Only for generic job and file stream
    pub(super) async fn send_current_digest(&mut self, stream: &mut Stream) -> ResultType<()> {
        let (last_modified, file_size) = self.get_current_digest().await?;
        let mut msg = Message::new();
        let mut resp = FileResponse::new();
        resp.set_digest(FileTransferDigest {
            id: self.id,
            file_num: self.file_num,
            last_modified,
            file_size,
            is_resume: self.is_resume,
            ..Default::default()
        });
        msg.set_file_response(resp);
        stream.send(&msg).await?;
        log::info!(
            "id: {}, file_num: {}, digest message is sent. waiting for confirm. msg: {:?}",
            self.id,
            self.file_num,
            msg
        );
        Ok(())
    }
}
