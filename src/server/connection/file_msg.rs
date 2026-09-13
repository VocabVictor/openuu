use super::*;

impl Connection {
    pub(super) async fn handle_file_action(&mut self, fa: FileAction) {
        if self.file_transfer.is_some() {
            if self.delayed_read_dir.is_some() {
                if let Some(file_action::Union::ReadDir(rd)) = fa.union {
                    self.delayed_read_dir = Some((rd.path, rd.include_hidden));
                }
                return;
            }
            if crate::get_builtin_option(keys::OPTION_ONE_WAY_FILE_TRANSFER) == "Y" {
                let mut job_id = None;
                match &fa.union {
                    Some(file_action::Union::Send(s)) => {
                        job_id = Some(s.id);
                    }
                    Some(file_action::Union::RemoveFile(rf)) => {
                        job_id = Some(rf.id);
                    }
                    Some(file_action::Union::Rename(r)) => {
                        job_id = Some(r.id);
                    }
                    Some(file_action::Union::Create(c)) => {
                        job_id = Some(c.id);
                    }
                    Some(file_action::Union::RemoveDir(rd)) => {
                        job_id = Some(rd.id);
                    }
                    _ => {}
                }
                if let Some(job_id) = job_id {
                    self.send(fs::new_error(job_id, "one-way-file-transfer-tip", 0))
                        .await;
                    return;
                }
            }
            // Android is scoped-storage only: reject any peer supplied path that
            // escapes the app workspace before it reaches the filesystem.
            #[cfg(target_os = "android")]
            {
                // (path, job id, allow empty) of the peer supplied path this action
                // operates on.
                let checked: Option<(&str, i32, bool)> = match &fa.union {
                    Some(file_action::Union::ReadEmptyDirs(rd)) => {
                        Some((rd.path.as_str(), -1, false))
                    }
                    Some(file_action::Union::ReadDir(rd)) => {
                        Some((rd.path.as_str(), 0, true))
                    }
                    Some(file_action::Union::AllFiles(f)) => {
                        Some((f.path.as_str(), f.id, false))
                    }
                    Some(file_action::Union::Send(s)) => {
                        if JobType::from_proto(s.file_type) == JobType::Generic {
                            Some((s.path.as_str(), s.id, false))
                        } else {
                            None
                        }
                    }
                    Some(file_action::Union::Receive(r)) => {
                        Some((r.path.as_str(), r.id, false))
                    }
                    Some(file_action::Union::RemoveDir(d)) => {
                        Some((d.path.as_str(), d.id, false))
                    }
                    Some(file_action::Union::RemoveFile(f)) => {
                        Some((f.path.as_str(), f.id, false))
                    }
                    Some(file_action::Union::Create(c)) => {
                        Some((c.path.as_str(), c.id, false))
                    }
                    Some(file_action::Union::Rename(r)) => {
                        Some((r.path.as_str(), r.id, false))
                    }
                    _ => None,
                };
                if let Some((path, job_id, allow_empty)) = checked {
                    if !crate::common::is_peer_path_allowed(path, allow_empty) {
                        log::warn!(
                            "Reject file action outside the app workspace: {}",
                            path
                        );
                        if job_id >= 0 {
                            self.send(fs::new_error(job_id, "Permission denied", -1))
                                .await;
                        }
                        return;
                    }
                }
                if let Some(file_action::Union::Rename(r)) = &fa.union {
                    let destination = std::path::Path::new(&r.path)
                        .parent()
                        .map(|parent| parent.join(&r.new_name));
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
                        self.send(fs::new_error(r.id, "Permission denied", -1))
                            .await;
                        return;
                    }
                }
            }
            self.handle_file_action_kind(fa).await;
        }
    }

    pub(super) fn handle_file_response(&mut self, fr: FileResponse) {
        match fr.union {
            Some(file_response::Union::Block(block)) => {
                self.send_fs(ipc::FS::WriteBlock {
                    id: block.id,
                    file_num: block.file_num,
                    data: block.data,
                    compressed: block.compressed,
                });
            }
            Some(file_response::Union::Done(d)) => {
                self.send_fs(ipc::FS::WriteDone {
                    id: d.id,
                    file_num: d.file_num,
                });
            }
            Some(file_response::Union::Digest(d)) => self.send_fs(ipc::FS::CheckDigest {
                id: d.id,
                file_num: d.file_num,
                file_size: d.file_size,
                last_modified: d.last_modified,
                is_upload: true,
                is_resume: d.is_resume,
            }),
            Some(file_response::Union::Error(e)) => {
                self.send_fs(ipc::FS::WriteError {
                    id: e.id,
                    file_num: e.file_num,
                    err: e.error,
                });
            }
            _ => {}
        }
    }
}
