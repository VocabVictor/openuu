use super::*;

impl TransferJob {
    pub fn set_overwrite_strategy(&mut self, overwrite_strategy: Option<bool>) {
        self.default_overwrite_strategy = overwrite_strategy;
    }

    pub fn default_overwrite_strategy(&self) -> Option<bool> {
        self.default_overwrite_strategy
    }

    pub fn set_file_confirmed(&mut self, file_confirmed: bool) {
        log::info!("id: {}, file_confirmed: {}", self.id, file_confirmed);
        self.file_confirmed = file_confirmed;
        self.file_skipped = false;
    }

    pub fn set_file_is_waiting(&mut self, file_is_waiting: bool) {
        self.file_is_waiting = file_is_waiting;
    }

    #[inline]
    pub fn file_is_waiting(&self) -> bool {
        self.file_is_waiting
    }

    #[inline]
    pub fn file_confirmed(&self) -> bool {
        self.file_confirmed
    }

    /// Indicating whether the last file is skipped
    #[inline]
    pub fn file_skipped(&self) -> bool {
        self.file_skipped
    }

    /// Indicating whether the whole task is skipped
    #[inline]
    pub fn job_skipped(&self) -> bool {
        self.file_skipped() && self.files.len() == 1
    }

    /// Check whether the job is completed after `read` returns `None`
    /// This is a helper function which gives additional lifecycle when the job reads `None`.
    /// If returns `true`, it means we can delete the job automatically. `False` otherwise.
    ///
    /// [`Note`]
    /// Conditions:
    /// 1. Files are not waiting for confirmation by peers.
    #[inline]
    pub fn job_completed(&self) -> bool {
        // has no error, Condition 2
        !self.enable_overwrite_detection || (!self.file_confirmed && !self.file_is_waiting)
    }

    /// Get job error message, useful for getting status when job had finished
    pub fn job_error(&self) -> Option<String> {
        if self.job_skipped() {
            return Some("skipped".to_string());
        }
        None
    }

    pub fn set_file_skipped(&mut self) -> bool {
        log::debug!("skip file {} in job {}", self.file_num, self.id);
        self.data_stream.take();
        self.set_file_confirmed(false);
        self.set_file_is_waiting(false);
        self.file_num += 1;
        self.file_skipped = true;
        true
    }

    pub(super) async fn set_stream_offset(&mut self, file_num: usize, offset: u64) {
        if file_num >= self.files.len() {
            return;
        }
        if let DataSource::FilePath(p) = &self.data_source {
            let entry = &self.files[file_num];
            let Some(path) = self.resolve_entry_path(p, &entry.name) else {
                return;
            };
            let file_path = get_string(&path);
            let download_path = format!("{}.download", &file_path);
            let digest_path = format!("{}.digest", &file_path);

            let mut f = if Path::new(&download_path).exists() && Path::new(&digest_path).exists() {
                // If both download and digest files exist, seek (writer) to the offset
                // NOTE: same as write path: best-effort symlink validation happened earlier,
                // but this reopen remains TOCTOU-prone by design for now.
                match OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&download_path)
                    .await
                {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("Failed to open file {}: {}", download_path, e);
                        return;
                    }
                }
            } else if Path::new(&file_path).exists() {
                // If `file_path` exists, seek (reader) to the offset
                match File::open(&file_path).await {
                    Ok(f) => f,
                    Err(e) => {
                        log::warn!("Failed to open file {}: {}", file_path, e);
                        return;
                    }
                }
            } else {
                log::warn!(
                    "File {} not found, cannot seek to offset {}",
                    file_path,
                    offset
                );
                return;
            };
            if f.seek(std::io::SeekFrom::Start(offset)).await.is_ok() {
                self.data_stream = Some(DataStream::FileStream(f));
                self.transferred += offset;
                self.finished_size += offset;
            }
        }
    }

    pub async fn confirm(&mut self, r: &FileTransferSendConfirmRequest) -> bool {
        if r.id != self.id || (r.file_num != self.file_num && !self.prefetched.contains(&r.file_num)) { return false; }
        self.confirmation_window = (r.confirmation_window as usize).min(16);
        if r.file_num > self.file_num && self.prefetched.contains(&r.file_num) {
            self.confirmations.insert(r.file_num, r.clone());
            return true;
        }
        if self.file_num() != r.file_num {
            // This branch will always be hit if:
            // 1. `confirm()` is called in `ui_cm_interface.rs`
            // 2. Not resuming
            //
            // It is ok. Because `confirm()` in `ui_cm_interface.rs` is only used for resuming.
            log::info!("file num truncated, ignoring");
        } else {
            match r.union {
                Some(file_transfer_send_confirm_request::Union::Skip(s)) => {
                    if s {
                        self.set_file_skipped();
                    } else {
                        self.set_file_confirmed(true);
                    }
                }
                Some(file_transfer_send_confirm_request::Union::OffsetBlk(offset)) => {
                    self.set_file_confirmed(true);
                    // If offset is greater than 0, we need to seek to the offset
                    if offset > 0 {
                        self.set_stream_offset(r.file_num as usize, offset as u64)
                            .await;
                    }
                }
                _ => {}
            }
        }
        true
    }

    #[inline]
    pub fn gen_meta(&self) -> TransferJobMeta {
        TransferJobMeta {
            id: self.id,
            remote: self.remote.to_string(),
            to: self.data_source.to_meta(),
            file_num: self.file_num,
            show_hidden: self.show_hidden,
            is_remote: self.is_remote,
        }
    }
}
