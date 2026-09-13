use super::*;

impl Connection {
    pub(super) async fn on_message(&mut self, msg: Message) -> bool {
        if let Some(message::Union::Misc(misc)) = &msg.union {
            // Move the CloseReason forward, as this message needs to be received when unauthorized, especially for kcp.
            if let Some(misc::Union::CloseReason(s)) = &misc.union {
                log::info!("receive close reason: {}", s);
                self.on_close("Peer close", true).await;
                raii::AuthedConnID::check_remove_session(self.inner.id(), self.session_key());
                return false;
            }
        }
        if self.authorized {
            if matches!(msg.union.as_ref(), Some(message::Union::LoginRequest(_))) {
                return true;
            }
            if let Some(message) = self.authorized_scope_violation(&msg) {
                return self.handle_authorized_scope_violation(message).await;
            }
        }
        // After handling CloseReason messages, proceed to process other message types
        if let Some(message::Union::LoginRequest(lr)) = msg.union {
            return self.handle_login_request(lr).await;
        } else if let Some(message::Union::Auth2fa(tfa)) = msg.union {
            return self.handle_auth_2fa(tfa).await;
        } else if let Some(message::Union::TestDelay(t)) = msg.union {
            self.handle_test_delay(t);
        } else if let Some(message::Union::SwitchSidesResponse(s)) = msg.union {
            return self.handle_switch_sides_response(s).await;
        } else if self.authorized {
            if self.port_forward_socket.is_some() {
                return true;
            }
            match msg.union {
                Some(message::Union::MouseEvent(me)) => self.handle_mouse_event(me),
                Some(message::Union::PointerDeviceEvent(pde)) => self.handle_pointer_device_event(pde),
                #[cfg(any(target_os = "ios"))]
                Some(message::Union::KeyEvent(..)) => {}
                #[cfg(any(target_os = "android"))]
                Some(message::Union::KeyEvent(me)) => self.handle_key_event(me),
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                Some(message::Union::KeyEvent(me)) => self.handle_key_event(me),
                Some(message::Union::Clipboard(cb)) => self.handle_clipboard(cb),
                Some(message::Union::MultiClipboards(_mcb)) => self.handle_multi_clipboards(_mcb),
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                Some(message::Union::Cliprdr(clip)) => self.handle_cliprdr(clip).await,
                Some(message::Union::FileAction(fa)) => {
                    if self.file_transfer.is_some() {
                        if self.delayed_read_dir.is_some() {
                            if let Some(file_action::Union::ReadDir(rd)) = fa.union {
                                self.delayed_read_dir = Some((rd.path, rd.include_hidden));
                            }
                            return true;
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
                                return true;
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
                                    return true;
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
                                    return true;
                                }
                            }
                        }
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
                                    JobType::Printer => return true,
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
                Some(message::Union::FileResponse(fr)) => match fr.union {
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
                },
                Some(message::Union::Misc(misc)) => return self.handle_misc(misc).await,
                Some(message::Union::AudioFrame(frame)) => self.handle_audio_frame(frame),
                Some(message::Union::VoiceCallRequest(request)) => self.handle_voice_call_request(request).await,
                Some(message::Union::VoiceCallResponse(_response)) => {
                    // TODO: Maybe we can do a voice call from cm directly.
                }
                Some(message::Union::ScreenshotRequest(request)) => self.handle_screenshot_request(request),
                Some(message::Union::PortForwardChannel(ch)) => self.handle_port_forward_channel(ch),
                Some(message::Union::TerminalAction(action)) => self.handle_terminal_action_msg(action).await,
                _ => {}
            }
        }
        true
    }
}
