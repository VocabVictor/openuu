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
                #[allow(unused_mut)]
                Some(message::Union::MouseEvent(mut me)) => {
                    if self.is_authed_view_camera_conn() {
                        return true;
                    }
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    if let Err(e) = call_main_service_pointer_input("mouse", me.mask, me.x, me.y) {
                        log::debug!("call_main_service_pointer_input fail:{}", e);
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if self.peer_keyboard_enabled() {
                        if is_left_up(&me) {
                            CLICK_TIME.store(get_time(), Ordering::SeqCst);
                        } else {
                            MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);
                        }
                        #[cfg(target_os = "macos")]
                        self.retina.on_mouse_event(&mut me, self.display_idx);
                        self.input_mouse(
                            me,
                            self.inner.id(),
                            self.lr.my_name.clone(),
                            self.peer_argb,
                            true,
                            self.show_my_cursor,
                        );
                    } else if self.show_my_cursor {
                        #[cfg(target_os = "macos")]
                        self.retina.on_mouse_event(&mut me, self.display_idx);
                        self.input_mouse(
                            me,
                            self.inner.id(),
                            self.lr.my_name.clone(),
                            self.peer_argb,
                            false,
                            true,
                        );
                    }
                    self.update_auto_disconnect_timer();
                }
                Some(message::Union::PointerDeviceEvent(pde)) => {
                    if self.is_authed_view_camera_conn() {
                        return true;
                    }
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    if let Err(e) = match pde.union {
                        Some(pointer_device_event::Union::TouchEvent(touch)) => match touch.union {
                            Some(touch_event::Union::PanStart(pan_start)) => {
                                call_main_service_pointer_input(
                                    "touch",
                                    4,
                                    pan_start.x,
                                    pan_start.y,
                                )
                            }
                            Some(touch_event::Union::PanUpdate(pan_update)) => {
                                call_main_service_pointer_input(
                                    "touch",
                                    5,
                                    pan_update.x,
                                    pan_update.y,
                                )
                            }
                            Some(touch_event::Union::PanEnd(pan_end)) => {
                                call_main_service_pointer_input("touch", 6, pan_end.x, pan_end.y)
                            }
                            _ => Ok(()),
                        },
                        _ => Ok(()),
                    } {
                        log::debug!("call_main_service_pointer_input fail:{}", e);
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if self.peer_keyboard_enabled() {
                        MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);
                        self.input_pointer(pde, self.inner.id());
                    }
                    self.update_auto_disconnect_timer();
                }
                #[cfg(any(target_os = "ios"))]
                Some(message::Union::KeyEvent(..)) => {}
                #[cfg(any(target_os = "android"))]
                Some(message::Union::KeyEvent(mut me)) => {
                    if self.is_authed_view_camera_conn() {
                        return true;
                    }
                    let key = match me.mode.enum_value() {
                        Ok(KeyboardMode::Map) => {
                            Some(crate::keyboard::keycode_to_rdev_key(me.chr()))
                        }
                        Ok(KeyboardMode::Translate) => {
                            if let Some(key_event::Union::Chr(code)) = me.union {
                                Some(crate::keyboard::keycode_to_rdev_key(code & 0x0000FFFF))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    }
                    .filter(crate::keyboard::is_modifier);

                    let is_press =
                        (me.press || me.down) && !(crate::is_modifier(&me) || key.is_some());

                    if let Some(key) = key {
                        if is_press {
                            self.pressed_modifiers.insert(key);
                        } else {
                            self.pressed_modifiers.remove(&key);
                        }
                    }

                    let mut modifiers = vec![];

                    for key in self.pressed_modifiers.iter() {
                        if let Some(control_key) = map_key_to_control_key(key) {
                            modifiers.push(EnumOrUnknown::new(control_key));
                        }
                    }

                    me.modifiers = modifiers;

                    let encode_result = me.write_to_bytes();

                    match encode_result {
                        Ok(data) => {
                            let result = call_main_service_key_event(&data);
                            if let Err(e) = result {
                                log::debug!("call_main_service_key_event fail: {}", e);
                            }
                        }
                        Err(e) => {
                            log::debug!("encode key event fail: {}", e);
                        }
                    }
                }
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                Some(message::Union::KeyEvent(me)) => {
                    if self.is_authed_view_camera_conn() {
                        return true;
                    }
                    if self.peer_keyboard_enabled() {
                        if is_enter(&me) {
                            CLICK_TIME.store(get_time(), Ordering::SeqCst);
                        }
                        // https://github.com/rustdesk/rustdesk/issues/8633
                        MOUSE_MOVE_TIME.store(get_time(), Ordering::SeqCst);

                        let key = match me.mode.enum_value() {
                            Ok(KeyboardMode::Map) => {
                                Some(crate::keyboard::keycode_to_rdev_key(me.chr()))
                            }
                            Ok(KeyboardMode::Translate) => {
                                if let Some(key_event::Union::Chr(code)) = me.union {
                                    Some(crate::keyboard::keycode_to_rdev_key(code & 0x0000FFFF))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        }
                        .filter(crate::keyboard::is_modifier);

                        // handle all down as press
                        // fix unexpected repeating key on remote linux, seems also fix abnormal alt/shift, which
                        // make sure all key are released
                        // https://github.com/rustdesk/rustdesk/issues/6793
                        let is_press = if cfg!(target_os = "linux") {
                            (me.press || me.down) && !(crate::is_modifier(&me) || key.is_some())
                        } else {
                            me.press
                        };

                        if let Some(key) = key {
                            if is_press {
                                self.pressed_modifiers.insert(key);
                            } else {
                                self.pressed_modifiers.remove(&key);
                            }
                        }

                        if is_press {
                            match me.union {
                                Some(key_event::Union::Unicode(_))
                                | Some(key_event::Union::Seq(_)) => {
                                    self.input_key(me, false);
                                }
                                _ => {
                                    self.input_key(me, true);
                                }
                            }
                        } else {
                            self.input_key(me, false);
                        }
                    }
                    self.update_auto_disconnect_timer();
                }
                Some(message::Union::Clipboard(cb)) => {
                    if self.should_handle_text_clipboard_message() && self.clipboard_enabled() {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(vec![cb], ClipboardSide::Host);
                        // ios as the controlled side is actually not supported for now.
                        // The following code is only used to preserve the logic of handling text clipboard on mobile.
                        #[cfg(target_os = "ios")]
                        {
                            let content = if cb.compress {
                                hbb_common::compress::decompress(&cb.content)
                            } else {
                                cb.content.into()
                            };
                            if let Ok(content) = String::from_utf8(content) {
                                let data =
                                    HashMap::from([("name", "clipboard"), ("content", &content)]);
                                if let Ok(data) = serde_json::to_string(&data) {
                                    let _ = crate::flutter::push_global_event(
                                        crate::flutter::APP_TYPE_MAIN,
                                        data,
                                    );
                                }
                            }
                        }
                        #[cfg(target_os = "android")]
                        crate::clipboard::handle_msg_clipboard(cb);
                    }
                }
                Some(message::Union::MultiClipboards(_mcb)) => {
                    if self.should_handle_text_clipboard_message() && self.clipboard_enabled() {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        update_clipboard(_mcb.clipboards, ClipboardSide::Host);
                        #[cfg(target_os = "android")]
                        crate::clipboard::handle_msg_multi_clipboards(_mcb);
                    }
                }
                #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
                Some(message::Union::Cliprdr(clip)) => {
                    if let Some(cliprdr::Union::Files(files)) = &clip.union {
                        self.post_file_audit(
                            FileAuditType::RemoteReceive,
                            "",
                            files
                                .files
                                .iter()
                                .map(|f| (f.name.clone(), f.size as i64))
                                .collect::<Vec<(String, i64)>>(),
                            json!({}),
                        );
                    } else if let Some(clip) = msg_2_clip(clip) {
                        #[cfg(target_os = "windows")]
                        {
                            self.send_to_cm(ipc::Data::ClipboardFile(clip));
                        }
                        #[cfg(feature = "unix-file-copy-paste")]
                        if crate::is_support_file_copy_paste(&self.lr.version) {
                            let mut out_msgs = vec![];

                            #[cfg(target_os = "macos")]
                            if clipboard::platform::unix::macos::should_handle_msg(&clip) {
                                if let Err(e) = clipboard::ContextSend::make_sure_enabled() {
                                    log::error!("failed to restart clipboard context: {}", e);
                                } else {
                                    let _ =
                                        clipboard::ContextSend::proc(|context| -> ResultType<()> {
                                            context
                                                .server_clip_file(self.inner.id(), clip)
                                                .map_err(|e| e.into())
                                        });
                                }
                            } else {
                                out_msgs = unix_file_clip::serve_clip_messages(
                                    ClipboardSide::Host,
                                    clip,
                                    self.inner.id(),
                                );
                            }

                            #[cfg(not(target_os = "macos"))]
                            {
                                out_msgs = unix_file_clip::serve_clip_messages(
                                    ClipboardSide::Host,
                                    clip,
                                    self.inner.id(),
                                );
                            }

                            for msg in out_msgs.into_iter() {
                                if let Some(message::Union::Cliprdr(cliprdr)) = msg.union.as_ref() {
                                    if let Some(cliprdr::Union::Files(files)) =
                                        cliprdr.union.as_ref()
                                    {
                                        self.post_file_audit(
                                            FileAuditType::RemoteSend,
                                            "",
                                            files
                                                .files
                                                .iter()
                                                .map(|f| (f.name.clone(), f.size as i64))
                                                .collect::<Vec<(String, i64)>>(),
                                            json!({}),
                                        );
                                        continue;
                                    }
                                }
                                self.send(msg).await;
                            }
                        }
                    }
                }
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
                Some(message::Union::Misc(misc)) => match misc.union {
                    Some(misc::Union::SwitchDisplay(s)) => {
                        self.handle_switch_display(s).await;
                    }
                    Some(misc::Union::CaptureDisplays(displays)) => {
                        let add = displays.add.iter().map(|d| *d as usize).collect::<Vec<_>>();
                        let sub = displays.sub.iter().map(|d| *d as usize).collect::<Vec<_>>();
                        let set = displays.set.iter().map(|d| *d as usize).collect::<Vec<_>>();
                        self.capture_displays(&add, &sub, &set).await;
                    }
                    #[cfg(windows)]
                    Some(misc::Union::ToggleVirtualDisplay(t)) => {
                        if !self.view_camera {
                            self.toggle_virtual_display(t).await;
                        }
                    }
                    Some(misc::Union::TogglePrivacyMode(t)) => {
                        if !self.view_camera {
                            self.toggle_privacy_mode(t).await;
                        }
                    }
                    Some(misc::Union::ChatMessage(c)) => {
                        self.send_to_cm(ipc::Data::ChatMessage { text: c.text });
                        self.chat_unanswered = true;
                        self.update_auto_disconnect_timer();
                    }
                    Some(misc::Union::Option(o)) => {
                        if self.authed_conn_type() == Some(AuthConnType::Remote) {
                            self.update_options(&o).await;
                        } else if let Some(option) = self.scoped_update_option_message(&o) {
                            self.update_options(&option).await;
                        }
                    }
                    Some(misc::Union::RefreshVideo(r)) => {
                        if self.should_handle_render_broadcast_message() {
                            if r {
                                // Refresh all videos.
                                // Compatibility with old versions and sciter(remote).
                                self.refresh_video_display(None);
                            }
                            self.update_auto_disconnect_timer();
                        }
                    }
                    Some(misc::Union::RefreshVideoDisplay(display)) => {
                        if self.should_handle_render_broadcast_message() {
                            self.refresh_video_display(Some(display as usize));
                            self.update_auto_disconnect_timer();
                        }
                    }
                    Some(misc::Union::VideoReceived(_)) => {
                        video_service::notify_video_frame_fetched_by_conn_id(
                            self.inner.id,
                            Some(Instant::now().into()),
                        );
                    }
                    Some(misc::Union::QuickLaunchRequest(request)) => {
                        let denied = crate::quick_launch::denied(&request);
                        let response = if self.authorized && self.is_authed_remote_conn() && self.peer_keyboard_enabled() {
                            match hbb_common::tokio::task::spawn_blocking(move || crate::quick_launch::handle(&request)).await {
                                Ok(response) => response,
                                Err(error) => { log::error!("Quick launch worker failed: {error}"); denied }
                            }
                        } else { denied };
                        let mut misc = Misc::new();
                        misc.set_quick_launch_response(response);
                        let mut msg = Message::new();
                        msg.set_misc(misc);
                        self.send(msg).await;
                    }
                    Some(misc::Union::RestartRemoteDevice(_)) => {
                        #[cfg(not(any(target_os = "android", target_os = "ios")))]
                        if self.restart {
                            // force_reboot, not work on linux vm and macos 14
                            #[cfg(any(target_os = "linux", target_os = "windows"))]
                            match system_shutdown::force_reboot() {
                                Ok(_) => log::info!("Restart by the peer"),
                                Err(e) => log::error!("Failed to restart: {}", e),
                            }
                            #[cfg(any(target_os = "linux", target_os = "macos"))]
                            match system_shutdown::reboot() {
                                Ok(_) => log::info!("Restart by the peer"),
                                Err(e) => log::error!("Failed to restart: {}", e),
                            }
                        }
                    }
                    #[cfg(windows)]
                    Some(misc::Union::ElevationRequest(r)) => match r.union {
                        Some(elevation_request::Union::Direct(_)) => {
                            self.handle_elevation_request(portable_client::StartPara::Direct)
                                .await;
                        }
                        Some(elevation_request::Union::Logon(r)) => {
                            self.handle_elevation_request(portable_client::StartPara::Logon(
                                r.username, r.password,
                            ))
                            .await;
                        }
                        _ => {}
                    },
                    Some(misc::Union::AudioFormat(format)) => {
                        if !self.disable_audio {
                            // Drop the audio sender previously.
                            drop(std::mem::replace(&mut self.audio_sender, None));
                            self.audio_sender = Some(start_audio_thread());
                            self.audio_sender
                                .as_ref()
                                .map(|a| allow_err!(a.send(MediaData::AudioFormat(format))));
                        }
                    }
                    #[cfg(feature = "flutter")]
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::SwitchSidesRequest(s)) => {
                        if let Ok(uuid) = uuid::Uuid::from_slice(&s.uuid.to_vec()[..]) {
                            if crate::server::insert_pending_switch_sides_uuid(
                                self.lr.my_id.clone(),
                                uuid.clone(),
                            ) {
                                crate::run_me(vec![
                                    "--connect",
                                    &self.lr.my_id,
                                    "--switch_uuid",
                                    uuid.to_string().as_ref(),
                                ])
                                .ok();
                            }
                            self.on_close("switch sides", false).await;
                            return false;
                        }
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::ChangeResolution(r)) => {
                        if !self.view_camera {
                            self.change_resolution(None, &r);
                        }
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    Some(misc::Union::ChangeDisplayResolution(dr)) => {
                        if !self.view_camera {
                            self.change_resolution(Some(dr.display as _), &dr.resolution);
                        }
                    }
                    Some(misc::Union::AutoAdjustFps(fps)) => video_service::VIDEO_QOS
                        .lock()
                        .unwrap()
                        .user_auto_adjust_fps(self.inner.id(), fps),
                    Some(misc::Union::ClientRecordStatus(status)) => video_service::VIDEO_QOS
                        .lock()
                        .unwrap()
                        .user_record(self.inner.id(), status),
                    #[cfg(windows)]
                    Some(misc::Union::SelectedSid(sid)) => {
                        if let Some(current_process_sid) =
                            crate::platform::get_current_process_session_id()
                        {
                            let sessions = crate::platform::get_available_sessions(false);
                            crate::platform::windows::sessions::pin_session_from_selection(
                                sid, &sessions,
                            );
                            if crate::platform::is_installed()
                                && crate::platform::is_share_rdp()
                                && raii::AuthedConnID::non_port_forward_conn_count() == 1
                                && sessions.len() > 1
                                && current_process_sid != sid
                                && sessions.iter().any(|e| e.sid == sid)
                            {
                                std::thread::spawn(move || {
                                    let _ = ipc::connect_to_user_session(Some(sid));
                                });
                                return false;
                            }
                            if self.file_transfer.is_some() {
                                if let Some((dir, show_hidden)) = self.delayed_read_dir.take() {
                                    self.read_dir(&dir, show_hidden);
                                }
                            } else if self.view_camera {
                                self.try_sub_camera_displays();
                            } else if !self.terminal {
                                self.try_sub_monitor_services();
                            }
                        }
                    }
                    Some(misc::Union::MessageQuery(mq)) => {
                        if let Some(msg_out) = video_service::make_display_changed_msg(
                            mq.switch_display as _,
                            None,
                            self.video_source(),
                        ) {
                            self.send(msg_out).await;
                        }
                    }
                    _ => {}
                },
                Some(message::Union::AudioFrame(frame)) => {
                    if !self.disable_audio {
                        if let Some(sender) = &self.audio_sender {
                            allow_err!(sender.send(MediaData::AudioFrame(Box::new(frame))));
                        } else {
                            log::warn!(
                                "Processing audio frame without the voice call audio sender."
                            );
                        }
                    }
                }
                Some(message::Union::VoiceCallRequest(request)) => {
                    if request.is_connect {
                        self.voice_call_request_timestamp = Some(
                            NonZeroI64::new(request.req_timestamp)
                                .unwrap_or(NonZeroI64::new(get_time()).unwrap()),
                        );
                        // Notify the connection manager.
                        self.send_to_cm(Data::VoiceCallIncoming);
                    } else {
                        self.close_voice_call().await;
                    }
                }
                Some(message::Union::VoiceCallResponse(_response)) => {
                    // TODO: Maybe we can do a voice call from cm directly.
                }
                Some(message::Union::ScreenshotRequest(request)) => {
                    if let Some(tx) = self.inner.tx.clone() {
                        crate::video_service::set_take_screenshot(
                            self.video_source(),
                            request.display as _,
                            request.sid.clone(),
                            tx,
                        );
                        self.refresh_video_display(Some(request.display as usize));
                    }
                }
                Some(message::Union::PortForwardChannel(ch)) => self.handle_port_forward_channel(ch),
                Some(message::Union::TerminalAction(action)) => {
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    allow_err!(self.handle_terminal_action(action).await);
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    log::warn!("Terminal action received but not supported on this platform");
                }
                _ => {}
            }
        }
        true
    }
}
