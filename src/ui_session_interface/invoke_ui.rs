use super::*;

pub trait InvokeUiSession: Send + Sync + Clone + 'static + Sized + Default {
    fn quick_launch_response(&self, _response: String) {}
    fn set_cursor_data(&self, cd: CursorData);
    fn set_cursor_id(&self, id: String);
    fn set_cursor_position(&self, cp: CursorPosition);
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, cursor_embedded: bool, scale: f64);
    fn switch_display(&self, display: &SwitchDisplay);
    fn set_peer_info(&self, peer_info: &PeerInfo); // flutter
    fn set_displays(&self, displays: &Vec<DisplayInfo>);
    fn set_platform_additions(&self, data: &str);
    fn on_connected(&self, conn_type: ConnType);
    fn update_privacy_mode(&self);
    fn set_permission(&self, name: &str, value: bool);
    fn close_success(&self);
    fn update_quality_status(&self, qs: QualityStatus);
    fn set_connection_type(&self, is_secured: bool, direct: bool, stream_type: &str);
    fn set_fingerprint(&self, fingerprint: String);
    fn job_error(&self, id: i32, err: String, file_num: i32);
    fn job_done(&self, id: i32, file_num: i32);
    fn clear_all_jobs(&self);
    fn new_message(&self, msg: String);
    fn update_transfer_list(&self);
    fn load_last_job(&self, cnt: i32, job_json: &str, auto_start: bool);
    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    );
    fn confirm_delete_files(&self, id: i32, i: i32, name: String);
    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        to: String,
        is_upload: bool,
        is_identical: bool,
    );
    fn update_block_input_state(&self, on: bool);
    fn job_progress(&self, id: i32, file_num: i32, speed: f64, finished_size: f64);
    fn adapt_size(&self);
    fn on_rgba(&self, display: usize, rgba: &mut scrap::ImageRgb);
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool);
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String);
    fn cancel_msgbox(&self, tag: &str);
    fn switch_back(&self, id: &str);
    fn portable_service_running(&self, running: bool);
    fn on_voice_call_started(&self);
    fn on_voice_call_closed(&self, reason: &str);
    fn on_voice_call_waiting(&self);
    fn on_voice_call_incoming(&self);
    fn get_rgba(&self, display: usize) -> *const u8;
    fn next_rgba(&self, display: usize);
    #[cfg(all(feature = "vram", feature = "flutter"))]
    fn on_texture(&self, display: usize, texture: *mut c_void);
    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>);
    fn set_current_display(&self, disp_idx: i32);
    #[cfg(feature = "flutter")]
    fn is_multi_ui_session(&self) -> bool;
    fn update_record_status(&self, start: bool);
    fn update_empty_dirs(&self, _res: ReadEmptyDirsResponse) {}
    fn handle_screenshot_resp(&self, sid: String, msg: String);
    fn handle_terminal_response(&self, response: TerminalResponse);
}

impl<T: InvokeUiSession> Deref for Session<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.ui_handler
    }
}

impl<T: InvokeUiSession> DerefMut for Session<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ui_handler
    }
}

impl<T: InvokeUiSession> FileManager for Session<T> {}
