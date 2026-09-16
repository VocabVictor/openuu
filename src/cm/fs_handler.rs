use super::*;

#[cfg(not(any(target_os = "ios")))]
pub(super) async fn handle_fs(
    fs: ipc::FS,
    write_jobs: &mut Vec<fs::TransferJob>,
    read_jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
    tx_log: Option<&UnboundedSender<String>>,
    _conn_id: i32,
) {
    // Android is scoped-storage only, so every peer supplied path has to stay inside the
    // app workspace. This is the filesystem boundary, keep it enforced here even though
    // `Connection` rejects out-of-workspace requests earlier as well.
    #[cfg(target_os = "android")]
    {
        // (path, job id, file num, allow empty) of the peer supplied path this message
        // acts on.
        let checked: Option<(&str, i32, i32, bool)> = match &fs {
            ipc::FS::ReadEmptyDirs { dir, .. } => Some((dir.as_str(), -1, -1, false)),
            ipc::FS::ReadDir { dir, .. } => Some((dir.as_str(), -1, -1, true)),
            ipc::FS::RemoveDir { path, id, .. } | ipc::FS::CreateDir { path, id } => {
                Some((path.as_str(), *id, 0, false))
            }
            ipc::FS::Rename { path, id, .. } => Some((path.as_str(), *id, 0, false)),
            ipc::FS::RemoveFile { path, id, file_num } => {
                Some((path.as_str(), *id, *file_num, false))
            }
            ipc::FS::ReadAllFiles { path, id, .. } => Some((path.as_str(), *id, -1, false)),
            ipc::FS::NewWrite {
                path, id, file_num, ..
            }
            | ipc::FS::ReadFile {
                path, id, file_num, ..
            } => Some((path.as_str(), *id, *file_num, false)),
            _ => None,
        };
        if let Some((path, id, file_num, allow_empty)) = checked {
            if !crate::common::is_peer_path_allowed(path, allow_empty) {
                log::warn!("Reject file operation outside the app workspace: {}", path);
                if id >= 0 {
                    send_raw(fs::new_error(id, "Permission denied", file_num), tx);
                }
                return;
            }
        }
        if let ipc::FS::Rename { path, new_name, id } = &fs {
            let destination = std::path::Path::new(path)
                .parent()
                .map(|parent| parent.join(new_name));
            let allowed = destination
                .as_deref()
                .and_then(std::path::Path::to_str)
                .map_or(false, |path| {
                    crate::common::is_peer_path_allowed(path, false)
                });
            if !allowed {
                log::warn!(
                    "Reject rename destination outside the app workspace: {:?}",
                    destination
                );
                send_raw(fs::new_error(*id, "Permission denied", 0), tx);
                return;
            }
        }
    }
    match fs {
        ipc::FS::ReadEmptyDirs {
            dir,
            include_hidden,
        } => {
            read_empty_dirs(&dir, include_hidden, tx).await;
        }
        ipc::FS::ReadDir {
            dir,
            include_hidden,
        } => {
            read_dir(&dir, include_hidden, tx).await;
        }
        ipc::FS::RemoveDir {
            path,
            id,
            recursive,
        } => {
            remove_dir(path, id, recursive, tx).await;
        }
        ipc::FS::RemoveFile { path, id, file_num } => {
            remove_file(path, id, file_num, tx).await;
        }
        ipc::FS::CreateDir { path, id } => {
            create_dir(path, id, tx).await;
        }
        ipc::FS::NewWrite {
            path,
            id,
            file_num,
            mut files,
            overwrite_detection,
            total_size,
            conn_id,
        } => {
            // Convert files to FileEntry
            let file_entries: Vec<FileEntry> = files
                .drain(..)
                .map(|f| FileEntry {
                    name: f.0,
                    modified_time: f.1,
                    ..Default::default()
                })
                .collect();

            // cm has no show_hidden context
            // dummy remote, show_hidden, is_remote
            let mut job = fs::TransferJob::new_write(
                id,
                fs::JobType::Generic,
                "".to_string(),
                fs::DataSource::FilePath(PathBuf::from(&path)),
                file_num,
                false,
                false,
                overwrite_detection,
            );
            if let Err(e) = job.set_files(file_entries) {
                log::warn!("Reject unsafe transfer file list for {}: {}", path, e);
                send_raw(fs::new_error(id, e, file_num), tx);
                return;
            }
            job.total_size = total_size;
            job.conn_id = conn_id;
            write_jobs.push(job);
        }
        ipc::FS::CancelWrite { id } => {
            if let Some(job) = fs::remove_job(id, write_jobs) {
                job.remove_download_file();
                if let Some(tx) = tx_log {
                    if let Err(e) = tx.send(serialize_transfer_job(&job, false, true, "")) {
                        log::error!("error sending transfer job log via IPC: {}", e);
                    }
                }
            }
        }
        ipc::FS::WriteDone { id, file_num } => {
            if let Some(job) = fs::remove_job(id, write_jobs) {
                job.modify_time();
                send_raw(fs::new_done(id, file_num), tx);
                tx_log.map(|tx| tx.send(serialize_transfer_job(&job, true, false, "")));
            }
        }
        ipc::FS::WriteError { id, file_num, err } => {
            if let Some(job) = fs::remove_job(id, write_jobs) {
                tx_log.map(|tx| tx.send(serialize_transfer_job(&job, false, false, &err)));
                send_raw(fs::new_error(job.id(), err, file_num), tx);
            }
        }
        ipc::FS::WriteBlock {
            id,
            file_num,
            data,
            compressed,
        } => {
            if let Some(job) = fs::get_job(id, write_jobs) {
                if let Err(err) = job
                    .write(FileTransferBlock {
                        id,
                        file_num,
                        data,
                        compressed,
                        ..Default::default()
                    })
                    .await
                {
                    send_raw(fs::new_error(id, err, file_num), &tx);
                }
            }
        }
        ipc::FS::CheckDigest {
            id,
            file_num,
            file_size,
            last_modified,
            is_upload,
            is_resume,
        } => {
            if let Some(job) = fs::get_job(id, write_jobs) {
                let mut req = FileTransferSendConfirmRequest {
                    id,
                    file_num,
                    union: Some(file_transfer_send_confirm_request::Union::OffsetBlk(0)),
                    ..Default::default()
                };
                let digest = FileTransferDigest {
                    id,
                    file_num,
                    last_modified,
                    file_size,
                    ..Default::default()
                };
                if let Some(file) = job.files().get(file_num as usize) {
                    if let fs::DataSource::FilePath(p) = &job.data_source {
                        let path = get_string(&fs::TransferJob::join(p, &file.name));
                        match is_write_need_confirmation(is_resume, &path, &digest) {
                            Ok(digest_result) => {
                                job.set_file_digest(file_num, file_size, last_modified);
                                match digest_result {
                                    DigestCheckResult::IsSame => {
                                        req.set_skip(true);
                                        let msg_out = new_send_confirm(req);
                                        send_raw(msg_out, &tx);
                                    }
                                    DigestCheckResult::NeedConfirm(mut digest) => {
                                        // upload to server, but server has the same file, request
                                        digest.is_upload = is_upload;
                                        let mut msg_out = Message::new();
                                        let mut fr = FileResponse::new();
                                        fr.set_digest(digest);
                                        msg_out.set_file_response(fr);
                                        send_raw(msg_out, &tx);
                                    }
                                    DigestCheckResult::NoSuchFile => {
                                        let msg_out = new_send_confirm(req);
                                        send_raw(msg_out, &tx);
                                    }
                                }
                            }
                            Err(err) => {
                                send_raw(fs::new_error(id, err, file_num), &tx);
                            }
                        }
                    }
                }
            }
        }
        ipc::FS::SendConfirm(bytes) => {
            if let Ok(r) = FileTransferSendConfirmRequest::parse_from_bytes(&bytes) {
                if let Some(job) = fs::get_job(r.id, write_jobs) {
                    job.confirm(&r).await;
                }
            }
        }
        ipc::FS::Rename { id, path, new_name } => {
            rename_file(path, new_name, id, tx).await;
        }
        ipc::FS::ReadFile {
            path,
            id,
            file_num,
            include_hidden,
            conn_id,
            overwrite_detection,
        } => {
            start_read_job(
                path,
                file_num,
                include_hidden,
                id,
                conn_id,
                overwrite_detection,
                read_jobs,
                tx,
            )
            .await;
        }
        // Cancel an ongoing read job (file transfer from server to client).
        // Note: This only cancels jobs in `read_jobs`. It does NOT cancel `ReadAllFiles`
        // operations, which are one-shot directory scans that complete quickly and don't
        // have persistent job tracking.
        ipc::FS::PauseRead { id, paused } => {
            if let Some(job) = fs::get_job(id, read_jobs) {
                job.paused = paused;
            }
        }
        ipc::FS::CancelRead { id, conn_id: _ } => {
            if let Some(job) = fs::remove_job(id, read_jobs) {
                if let Some(tx) = tx_log {
                    if let Err(e) = tx.send(serialize_transfer_job(&job, false, true, "")) {
                        log::error!("error sending transfer job log via IPC: {}", e);
                    }
                }
            }
        }
        ipc::FS::SendConfirmForRead {
            id,
            file_num,
            skip,
            offset_blk,
            confirmation_window,
            conn_id: _,
        } => {
            if let Some(job) = fs::get_job(id, read_jobs) {
                let req = FileTransferSendConfirmRequest {
                    id,
                    file_num,
                    confirmation_window,
                    union: if skip {
                        Some(file_transfer_send_confirm_request::Union::Skip(true))
                    } else {
                        Some(file_transfer_send_confirm_request::Union::OffsetBlk(
                            offset_blk,
                        ))
                    },
                    ..Default::default()
                };
                job.confirm(&req).await;
            }
        }
        // Recursively list all files in a directory.
        // This is a one-shot operation that cannot be cancelled via CancelRead.
        // The operation typically completes quickly as it only reads directory metadata,
        // not file contents. File count is limited by `check_file_count_limit()`.
        ipc::FS::ReadAllFiles {
            path,
            id,
            include_hidden,
            conn_id,
        } => {
            read_all_files(path, include_hidden, id, conn_id, tx).await;
        }
        _ => {}
    }
}
