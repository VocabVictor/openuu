use super::*;

impl<T: InvokeUiSession> Remote<T> {
    pub(super) async fn handle_file_response(&mut self, fr: FileResponse, peer: &mut Stream) {
        match fr.union {
            Some(file_response::Union::EmptyDirs(res)) => {
                self.handler.update_empty_dirs(res);
            }
            Some(file_response::Union::Dir(fd)) => {
                #[cfg(windows)]
                let entries = fd.entries.to_vec();
                #[cfg(not(windows))]
                let mut entries = fd.entries.to_vec();
                #[cfg(not(windows))]
                {
                    if self.handler.peer_platform() == "Windows" {
                        fs::transform_windows_path(&mut entries);
                    }
                }
                // We cannot call cancel_transfer_job/handle_job_status while holding
                // a mutable borrow from fs::get_job(&mut self.write_jobs), so defer
                // the error handling until after the borrow scope ends.
                let mut set_files_err = None;
                if let Some(job) = fs::get_job(fd.id, &mut self.write_jobs) {
                    log::info!("job set_files: {:?}", entries);
                    if let Err(err) = job.set_files(entries) {
                        set_files_err = Some(err.to_string());
                    } else {
                        job.set_finished_size_on_resume();
                        self.handler.update_folder_files(
                            fd.id,
                            job.files(),
                            fd.path,
                            false,
                            false,
                        );
                    }
                } else if let Some(job) = self.remove_jobs.get_mut(&fd.id) {
                    // Intentionally keep raw entries here:
                    // - remote remove flow executes deletions on peer side;
                    // - local remove flow is populated from local get_recursive_files().
                    job.files = entries;
                    self.handler
                        .update_folder_files(fd.id, &job.files, fd.path, false, false);
                } else {
                    self.handler
                        .update_folder_files(fd.id, &entries, fd.path, false, false);
                }
                if let Some(err) = set_files_err {
                    log::warn!(
                        "Rejected unsafe file list from remote peer for job {}: {}",
                        fd.id,
                        err
                    );
                    self.cancel_transfer_job(fd.id, peer).await;
                    self.handle_job_status(fd.id, -1, Some(err));
                }
            }
            Some(file_response::Union::Digest(digest)) => {
                self.handle_file_digest(digest, peer).await;
            }
            Some(file_response::Union::Block(block)) => {
                if let Some(job) = fs::get_job(block.id, &mut self.write_jobs) {
                    if let Err(_err) = job.write(block).await {
                        // to-do: add "skip" for writing job
                    }
                    if job.r#type == fs::JobType::Generic {
                        self.update_jobs_status();
                    }
                }
            }
            Some(file_response::Union::Done(d)) => {
                let mut err: Option<String> = None;
                let mut job_type = fs::JobType::Generic;
                if let Some(job) = fs::remove_job(d.id, &mut self.write_jobs) {
                    job.modify_time();
                    err = job.job_error();
                    job_type = job.r#type;
                }
                match job_type {
                    fs::JobType::Generic => {
                        self.handle_job_status(d.id, d.file_num, err);
                    }
                    fs::JobType::Printer => {}
                }
            }
            Some(file_response::Union::Error(e)) => {
                let job_type = fs::remove_job(e.id, &mut self.write_jobs)
                    .or_else(|| fs::remove_job(e.id, &mut self.read_jobs))
                    .map(|j| j.r#type)
                    .unwrap_or(fs::JobType::Generic);
                match job_type {
                    fs::JobType::Generic => {
                        self.handle_job_status(e.id, e.file_num, Some(e.error));
                    }
                    fs::JobType::Printer => {
                        log::debug!("Discarded obsolete transfer job {}", e.id);
                    }
                }
            }
            _ => {}
        }
    }

    async fn handle_file_digest(&mut self, digest: FileTransferDigest, peer: &mut Stream) {
            if digest.is_upload {
                if let Some(job) = fs::get_job(digest.id, &mut self.read_jobs) {
                    if let Some(file) = job.files().get(digest.file_num as usize) {
                        if let fs::DataSource::FilePath(p) = &job.data_source {
                            let read_path =
                                get_string(&fs::TransferJob::join(p, &file.name));
                            let mut overwrite_strategy =
                                job.default_overwrite_strategy();
                            let mut offset = 0;
                            if digest.is_identical && job.is_resume {
                                if digest.transferred_size > 0 {
                                    overwrite_strategy = Some(true);
                                    offset = digest.transferred_size as _;
                                }
                            }
                            if let Some(overwrite) = overwrite_strategy {
                                let req = FileTransferSendConfirmRequest {
                                    id: digest.id,
                                    file_num: digest.file_num,
                                    union: Some(if overwrite {
                                        file_transfer_send_confirm_request::Union::OffsetBlk(offset)
                                    } else {
                                        file_transfer_send_confirm_request::Union::Skip(
                                            true,
                                        )
                                    }),
                                    ..Default::default()
                                };
                                job.confirm(&req).await;
                                let msg = new_send_confirm(req);
                                allow_err!(peer.send(&msg).await);
                            } else {
                                self.handler.override_file_confirm(
                                    digest.id,
                                    digest.file_num,
                                    read_path,
                                    true,
                                    digest.is_identical,
                                );
                            }
                        }
                    }
                }
            } else {
                if let Some(job) = fs::get_job(digest.id, &mut self.write_jobs) {
                    if let Some(file) = job.files().get(digest.file_num as usize) {
                        if let fs::DataSource::FilePath(p) = &job.data_source {
                            let write_path =
                                get_string(&fs::TransferJob::join(p, &file.name));
                            job.set_file_digest(digest.file_num, digest.file_size, digest.last_modified);
                            let peer_ver = self.handler.lc.read().unwrap().version;
                            let is_support_resume =
                                crate::is_support_file_transfer_resume_num(
                                    peer_ver,
                                );
                            match fs::is_write_need_confirmation(
                                is_support_resume && job.is_resume,
                                &write_path,
                                &digest,
                            ) {
                                Ok(res) => match res {
                                    DigestCheckResult::IsSame => {
                                        let req = FileTransferSendConfirmRequest {
                                            id: digest.id,
                                            file_num: digest.file_num,
                                            union: Some(file_transfer_send_confirm_request::Union::Skip(true)),
                                            ..Default::default()
                                        };
                                        job.confirm(&req).await;
                                        let msg = new_send_confirm(req);
                                        allow_err!(peer.send(&msg).await);
                                    }
                                    DigestCheckResult::NeedConfirm(digest) => {
                                        let mut overwrite_strategy =
                                            job.default_overwrite_strategy();
                                        let mut offset = 0;
                                        if digest.is_identical
                                            && job.is_resume
                                            && digest.transferred_size > 0
                                        {
                                            overwrite_strategy = Some(true);
                                            offset = digest.transferred_size as _;
                                        }
                                        if let Some(overwrite) = overwrite_strategy
                                        {
                                            let req =
                                                FileTransferSendConfirmRequest {
                                                    id: digest.id,
                                                    file_num: digest.file_num,
                                                    union: Some(if overwrite {
                                                        file_transfer_send_confirm_request::Union::OffsetBlk(offset)
                                                    } else {
                                                        file_transfer_send_confirm_request::Union::Skip(true)
                                                    }),
                                                    ..Default::default()
                                                };
                                            job.confirm(&req).await;
                                            let msg = new_send_confirm(req);
                                            allow_err!(peer.send(&msg).await);
                                        } else {
                                            self.handler.override_file_confirm(
                                                digest.id,
                                                digest.file_num,
                                                write_path,
                                                false,
                                                digest.is_identical,
                                            );
                                        }
                                    }
                                    DigestCheckResult::NoSuchFile => {
                                        let req = FileTransferSendConfirmRequest {
                                        id: digest.id,
                                        file_num: digest.file_num,
                                        union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                                        ..Default::default()
                                    };
                                        job.confirm(&req).await;
                                        let msg = new_send_confirm(req);
                                        allow_err!(peer.send(&msg).await);
                                    }
                                },
                                Err(err) => {
                                    println!("error receiving digest: {}", err);
                                }
                            }
                        }
                    }
                }
            }
    }

    pub(super) async fn handle_file_action(&mut self, action: FileAction) {
        match action.union {
            Some(file_action::Union::Send(_s)) => match _s.file_type.enum_value() {
                #[cfg(target_os = "windows")]
                Ok(file_transfer_send_request::FileType::Printer) => {}
                _ => {}
            },
            Some(file_action::Union::SendConfirm(c)) => {
                if let Some(job) = fs::get_job(c.id, &mut self.read_jobs) {
                    job.confirm(&c).await;
                }
        }
        _ => {}
        }
    }
}
