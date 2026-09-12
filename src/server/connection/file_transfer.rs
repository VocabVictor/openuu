use super::*;

impl Connection {
    pub(super) async fn handle_read_job_init_result(
        &mut self,
        id: i32,
        _file_num: i32,
        _include_hidden: bool,
        result: Result<Vec<u8>, String>,
    ) {
        // Check if this response is still expected (not stale/cancelled)
        if !self.cm_read_job_ids.contains(&id) {
            log::warn!(
                "Received ReadJobInitResult for unknown or stale job id={}, ignoring",
                id
            );
            return;
        }

        match result {
            Err(error) => {
                self.cm_read_job_ids.remove(&id);
                self.send(fs::new_error(id, error, 0)).await;
            }
            Ok(dir_bytes) => {
                // Deserialize FileDirectory from protobuf bytes
                let dir = match FileDirectory::parse_from_bytes(&dir_bytes) {
                    Ok(d) => d,
                    Err(e) => {
                        log::error!("Failed to parse FileDirectory: {}", e);
                        self.cm_read_job_ids.remove(&id);
                        self.send(fs::new_error(id, "internal error".to_string(), 0))
                            .await;
                        return;
                    }
                };

                let path_str = dir.path.clone();
                let file_entries: Vec<FileEntry> = dir.entries.into();

                // Send file directory to client
                self.send(fs::new_dir(id, path_str.clone(), file_entries.clone()))
                    .await;

                // Post audit for file transfer
                self.post_file_audit(
                    FileAuditType::RemoteSend,
                    &path_str,
                    Self::get_files_for_audit(fs::JobType::Generic, file_entries),
                    json!({}),
                );

                // CM will handle the actual file reading and send blocks via IPC
                self.file_transferred = true;
            }
        }
    }

    pub(super) async fn handle_file_block_from_cm(
        &mut self,
        id: i32,
        file_num: i32,
        data: bytes::Bytes,
        compressed: bool,
    ) {
        // Check if the job is still valid (not cancelled)
        if !self.cm_read_job_ids.contains(&id) {
            log::debug!(
                "Dropping file block for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward file block to client
        let mut block = FileTransferBlock::new();
        block.id = id;
        block.file_num = file_num;
        block.data = data.to_vec().into();
        block.compressed = compressed;

        let mut msg = Message::new();
        let mut fr = FileResponse::new();
        fr.set_block(block);
        msg.set_file_response(fr);
        self.send(msg).await;
    }

    pub(super) async fn handle_file_read_done(&mut self, id: i32, file_num: i32) {
        // Drop stale completions for cancelled/unknown jobs
        if !self.cm_read_job_ids.remove(&id) {
            log::debug!(
                "Dropping FileReadDone for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward done message to client
        let mut done = FileTransferDone::new();
        done.id = id;
        done.file_num = file_num;

        let mut msg = Message::new();
        let mut fr = FileResponse::new();
        fr.set_done(done);
        msg.set_file_response(fr);
        self.send(msg).await;
    }

    pub(super) async fn handle_file_read_error(&mut self, id: i32, file_num: i32, err: String) {
        // Drop stale errors for cancelled/unknown jobs
        if !self.cm_read_job_ids.remove(&id) {
            log::debug!(
                "Dropping FileReadError for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward error to client
        self.send(fs::new_error(id, err, file_num)).await;
    }

    pub(super) async fn handle_file_digest_from_cm(
        &mut self,
        id: i32,
        file_num: i32,
        last_modified: u64,
        file_size: u64,
        is_resume: bool,
    ) {
        // Check if the job is still valid (not cancelled)
        if !self.cm_read_job_ids.contains(&id) {
            log::debug!(
                "Dropping digest for cancelled/unknown job id={}, file_num={}",
                id,
                file_num
            );
            return;
        }

        // Forward digest to client for overwrite detection
        let mut digest = FileTransferDigest::new();
        digest.id = id;
        digest.file_num = file_num;
        digest.last_modified = last_modified;
        digest.file_size = file_size;
        digest.is_upload = false; // Server sending to client
        digest.is_resume = is_resume;

        let mut msg = Message::new();
        let mut fr = FileResponse::new();
        fr.set_digest(digest);
        msg.set_file_response(fr);
        self.send(msg).await;
    }

    pub(super) async fn process_new_read_job(&mut self, mut job: fs::TransferJob, path: String) {
        let files = job.files().to_owned();
        let job_type = job.r#type;
        self.send(fs::new_dir(job.id, path.clone(), files.clone()))
            .await;
        job.is_remote = true;
        job.conn_id = self.inner.id();
        self.read_jobs.push(job);
        self.file_timer = crate::rustdesk_interval(time::interval(MILLI1));
        let audit_path = path;
        self.post_file_audit(
            FileAuditType::RemoteSend,
            &audit_path,
            Self::get_files_for_audit(job_type, files),
            json!({}),
        );
    }

    pub(super) async fn handle_all_files_result(
        &mut self,
        id: i32,
        path: String,
        result: Result<Vec<u8>, String>,
    ) {
        match result {
            Err(err) => {
                self.send(fs::new_error(id, err, -1)).await;
            }
            Ok(bytes) => {
                // Deserialize FileDirectory from protobuf bytes and send as FileResponse
                match FileDirectory::parse_from_bytes(&bytes) {
                    Ok(fd) => {
                        let mut msg = Message::new();
                        let mut fr = FileResponse::new();
                        fr.set_dir(fd);
                        msg.set_file_response(fr);
                        self.send(msg).await;
                    }
                    Err(e) => {
                        self.send(fs::new_error(
                            id,
                            format!("deserialize failed for {}: {}", path, e),
                            -1,
                        ))
                        .await;
                    }
                }
            }
        }
    }

    pub(super) fn read_empty_dirs(&mut self, dir: &str, include_hidden: bool) {
        let dir = dir.to_string();
        self.send_fs(ipc::FS::ReadEmptyDirs {
            dir,
            include_hidden,
        });
    }

    pub(super) fn read_dir(&mut self, dir: &str, include_hidden: bool) {
        let dir = dir.to_string();
        self.send_fs(ipc::FS::ReadDir {
            dir,
            include_hidden,
        });
    }

    /// Create a new read job and start processing it (Connection-side).
    ///
    /// This is a generic Connection-side read job creation helper used for:
    /// - Generic file transfers on non-Windows platforms
    ///
    /// On Windows, generic file reads are delegated to CM via `start_read_job()` in
    /// `src/ui_cm_interface.rs` for elevated access.
    ///
    /// Both Connection-side and CM-side implementations use `TransferJob::new_read()`
    /// with similar parameters. When modifying job creation logic, ensure both paths
    /// stay in sync.
    pub(super) async fn create_and_start_read_job(
        &mut self,
        id: i32,
        job_type: fs::JobType,
        data_source: fs::DataSource,
        file_num: i32,
        include_hidden: bool,
        overwrite_detection: bool,
        path: String,
        check_file_limit: bool,
    ) {
        match fs::TransferJob::new_read(
            id,
            job_type,
            "".to_string(),
            data_source,
            file_num,
            include_hidden,
            false,
            overwrite_detection,
        ) {
            Err(err) => {
                self.send(fs::new_error(id, err, 0)).await;
            }
            Ok(job) => {
                if check_file_limit {
                    if let Err(msg) =
                        crate::ui_cm_interface::check_file_count_limit(job.files().len())
                    {
                        self.send(fs::new_error(id, msg, -1)).await;
                        return;
                    }
                }
                self.process_new_read_job(job, path).await;
            }
        }
    }
}
