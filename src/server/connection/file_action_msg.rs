use super::*;

impl Connection {
    /// Dispatch an accepted `FileAction` to the file service and the cm.
    pub(super) async fn handle_file_action_kind(&mut self, fa: FileAction) {
        match fa.union {
            Some(file_action::Union::ReadEmptyDirs(rd)) => {
                self.read_empty_dirs(&rd.path, rd.include_hidden);
            }
            Some(file_action::Union::ReadDir(rd)) => {
                self.read_dir(&rd.path, rd.include_hidden);
            }
            Some(file_action::Union::AllFiles(f)) => {
                if crate::common::need_fs_cm_send_files() {
                    self.send_fs(ipc::FS::ReadAllFiles {
                        path: f.path,
                        id: f.id,
                        include_hidden: f.include_hidden,
                        conn_id: self.inner.id(),
                    });
                } else {
                    match fs::get_recursive_files(&f.path, f.include_hidden) {
                        Err(err) => {
                            log::error!(
                                "Failed to get recursive files for {}: {}",
                                f.path,
                                err
                            );
                            self.send(fs::new_error(f.id, err, -1)).await;
                        }
                        Ok(files) => {
                            if let Err(msg) =
                                crate::ui_cm_interface::check_file_count_limit(
                                    files.len(),
                                )
                            {
                                self.send(fs::new_error(f.id, msg, -1)).await;
                            } else {
                                self.send(fs::new_dir(f.id, f.path, files)).await;
                            }
                        }
                    }
                }
            }
            Some(file_action::Union::Send(s)) => {
                // server to client
                let id = s.id;
                let path = s.path.clone();
                let job_type = JobType::from_proto(s.file_type);
                match job_type {
                    JobType::Generic => {
                        let od = can_enable_overwrite_detection(
                            get_version_number(&self.lr.version),
                        );
                        if crate::common::need_fs_cm_send_files() {
                            // Delegate file reading to CM on Windows
                            self.cm_read_job_ids.insert(id);
                            self.send_fs(ipc::FS::ReadFile {
                                path,
                                id,
                                file_num: s.file_num,
                                include_hidden: s.include_hidden,
                                conn_id: self.inner.id(),
                                overwrite_detection: od,
                            });
                        } else {
                            // Handle file reading in Connection on non-Windows
                            let data_source =
                                fs::DataSource::FilePath(PathBuf::from(&path));
                            self.create_and_start_read_job(
                                id,
                                job_type,
                                data_source,
                                s.file_num,
                                s.include_hidden,
                                od,
                                path,
                                true, // check file count limit
                            )
                            .await;
                        }
                    }
                    JobType::Printer => return,
                }
                self.file_transferred = true;
            }
            Some(file_action::Union::Receive(r)) => {
                // client to server
                // note: 1.1.10 introduced identical file detection, which breaks original logic of send/recv files
                // whenever got send/recv request, check peer version to ensure old version of rustdesk
                let od = can_enable_overwrite_detection(get_version_number(
                    &self.lr.version,
                ));
                self.send_fs(ipc::FS::NewWrite {
                    path: r.path.clone(),
                    id: r.id,
                    file_num: r.file_num,
                    files: r
                        .files
                        .to_vec()
                        .drain(..)
                        .map(|f| (f.name, f.modified_time))
                        .collect(),
                    overwrite_detection: od,
                    total_size: r.total_size,
                    conn_id: self.inner.id(),
                });
                self.post_file_audit(
                    FileAuditType::RemoteReceive,
                    &r.path,
                    Self::get_files_for_audit(fs::JobType::Generic, r.files),
                    json!({}),
                );
                self.file_transferred = true;
            }
            Some(file_action::Union::RemoveDir(d)) => {
                self.send_fs(ipc::FS::RemoveDir {
                    path: d.path.clone(),
                    id: d.id,
                    recursive: d.recursive,
                });
                self.file_remove_log_control.on_remove_dir(d);
            }
            Some(file_action::Union::RemoveFile(f)) => {
                self.send_fs(ipc::FS::RemoveFile {
                    path: f.path.clone(),
                    id: f.id,
                    file_num: f.file_num,
                });
                self.file_remove_log_control.on_remove_file(f);
            }
            Some(file_action::Union::Create(c)) => {
                self.send_fs(ipc::FS::CreateDir {
                    path: c.path.clone(),
                    id: c.id,
                });
                self.send_to_cm(ipc::Data::FileTransferLog((
                    "create_dir".to_string(),
                    serde_json::to_string(&FileActionLog {
                        id: c.id,
                        conn_id: self.inner.id(),
                        path: c.path,
                        dir: true,
                    })
                    .unwrap_or_default(),
                )));
            }
            Some(file_action::Union::Cancel(c)) => {
                self.send_fs(ipc::FS::CancelWrite { id: c.id });
                let _ = self.cm_read_job_ids.remove(&c.id);
                self.send_fs(ipc::FS::CancelRead {
                    id: c.id,
                    conn_id: self.inner.id(),
                });
                if let Some(job) = fs::remove_job(c.id, &mut self.read_jobs) {
                    self.send_to_cm(ipc::Data::FileTransferLog((
                        "transfer".to_string(),
                        fs::serialize_transfer_job(&job, false, true, ""),
                    )));
                }
            }
            Some(file_action::Union::Pause(p)) => {
                if let Some(job) = fs::get_job(p.id, &mut self.read_jobs) {
                    job.paused = p.paused;
                } else if self.cm_read_job_ids.contains(&p.id) {
                    self.send_fs(ipc::FS::PauseRead { id: p.id, paused: p.paused });
                }
            }
            Some(file_action::Union::SendConfirm(r)) => {
                if let Some(job) = fs::get_job(r.id, &mut self.read_jobs) {
                    job.confirm(&r).await;
                } else if self.cm_read_job_ids.contains(&r.id) {
                    // Forward to CM for CM-read jobs
                    self.send_fs(ipc::FS::SendConfirmForRead {
                        confirmation_window: r.confirmation_window,
                        id: r.id,
                        file_num: r.file_num,
                        skip: r.skip(),
                        offset_blk: r.offset_blk(),
                        conn_id: self.inner.id(),
                    });
                } else {
                    if let Ok(sc) = r.write_to_bytes() {
                        self.send_fs(ipc::FS::SendConfirm(sc));
                    }
                }
            }
            Some(file_action::Union::Rename(r)) => {
                self.send_fs(ipc::FS::Rename {
                    id: r.id,
                    path: r.path.clone(),
                    new_name: r.new_name.clone(),
                });
                self.send_to_cm(ipc::Data::FileTransferLog((
                    "rename".to_string(),
                    serde_json::to_string(&FileRenameLog {
                        conn_id: self.inner.id(),
                        path: r.path,
                        new_name: r.new_name,
                    })
                    .unwrap_or_default(),
                )));
            }
            _ => {}
        }
    }
}
