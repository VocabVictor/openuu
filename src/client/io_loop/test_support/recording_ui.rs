use super::*;

/// An `InvokeUiSession` that only records what the io loop asked the UI to do.
#[derive(Clone, Default)]
pub(crate) struct RecordingUi {
    calls: Arc<std::sync::Mutex<Vec<String>>>,
}

impl RecordingUi {
    fn record(&self, call: String) {
        self.calls.lock().unwrap().push(call);
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn calls_clear(&self) {
        self.calls.lock().unwrap().clear();
    }

    pub(crate) fn has_call(&self, prefix: &str) -> bool {
        self.calls().iter().any(|c| c.starts_with(prefix))
    }
}

impl InvokeUiSession for RecordingUi {
    fn quick_launch_response(&self, response: String) {
        self.record(format!("quick_launch_response:{}", response.len()));
    }
    fn set_cursor_data(&self, cd: CursorData) {
        self.record(format!("set_cursor_data:{}", cd.id));
    }
    fn set_cursor_id(&self, id: String) {
        self.record(format!("set_cursor_id:{id}"));
    }
    fn set_cursor_position(&self, cp: CursorPosition) {
        self.record(format!("set_cursor_position:{},{}", cp.x, cp.y));
    }
    fn set_display(&self, x: i32, y: i32, w: i32, h: i32, cursor_embedded: bool, scale: f64) {
        self.record(format!("set_display:{x},{y},{w},{h},{cursor_embedded},{scale}"));
    }
    fn switch_display(&self, display: &SwitchDisplay) {
        self.record(format!("switch_display:{}", display.display));
    }
    fn set_peer_info(&self, peer_info: &PeerInfo) {
        self.record(format!("set_peer_info:{}", peer_info.version));
    }
    fn set_displays(&self, displays: &Vec<DisplayInfo>) {
        self.record(format!("set_displays:{}", displays.len()));
    }
    fn set_platform_additions(&self, data: &str) {
        self.record(format!("set_platform_additions:{data}"));
    }
    fn on_connected(&self, conn_type: ConnType) {
        self.record(format!("on_connected:{conn_type:?}"));
    }
    fn update_privacy_mode(&self) {
        self.record("update_privacy_mode".to_owned());
    }
    fn set_permission(&self, name: &str, value: bool) {
        self.record(format!("set_permission:{name}={value}"));
    }
    fn close_success(&self) {
        self.record("close_success".to_owned());
    }
    fn update_quality_status(&self, _qs: QualityStatus) {
        self.record("update_quality_status".to_owned());
    }
    fn set_connection_type(&self, is_secured: bool, direct: bool, stream_type: &str) {
        self.record(format!("set_connection_type:{is_secured},{direct},{stream_type}"));
    }
    fn set_fingerprint(&self, fingerprint: String) {
        self.record(format!("set_fingerprint:{fingerprint}"));
    }
    fn job_error(&self, id: i32, err: String, file_num: i32) {
        self.record(format!("job_error:{id},{file_num},{err}"));
    }
    fn job_done(&self, id: i32, file_num: i32) {
        self.record(format!("job_done:{id},{file_num}"));
    }
    fn clear_all_jobs(&self) {
        self.record("clear_all_jobs".to_owned());
    }
    fn new_message(&self, msg: String) {
        self.record(format!("new_message:{msg}"));
    }
    fn update_transfer_list(&self) {
        self.record("update_transfer_list".to_owned());
    }
    fn load_last_job(&self, cnt: i32, _job_json: &str, auto_start: bool) {
        self.record(format!("load_last_job:{cnt},{auto_start}"));
    }
    fn update_folder_files(
        &self,
        id: i32,
        entries: &Vec<FileEntry>,
        path: String,
        is_local: bool,
        only_count: bool,
    ) {
        self.record(format!(
            "update_folder_files:{id},{},{path},{is_local},{only_count}",
            entries.len()
        ));
    }
    fn confirm_delete_files(&self, id: i32, i: i32, name: String) {
        self.record(format!("confirm_delete_files:{id},{i},{name}"));
    }
    fn override_file_confirm(
        &self,
        id: i32,
        file_num: i32,
        to: String,
        is_upload: bool,
        is_identical: bool,
    ) {
        self.record(format!(
            "override_file_confirm:{id},{file_num},{to},{is_upload},{is_identical}"
        ));
    }
    fn update_block_input_state(&self, on: bool) {
        self.record(format!("update_block_input_state:{on}"));
    }
    fn job_progress(&self, id: i32, file_num: i32, _speed: f64, _finished_size: f64) {
        self.record(format!("job_progress:{id},{file_num}"));
    }
    fn adapt_size(&self) {
        self.record("adapt_size".to_owned());
    }
    fn on_rgba(&self, display: usize, _rgba: &mut scrap::ImageRgb) {
        self.record(format!("on_rgba:{display}"));
    }
    fn msgbox(&self, msgtype: &str, title: &str, text: &str, link: &str, retry: bool) {
        self.record(format!("msgbox:{msgtype}|{title}|{text}|{link}|{retry}"));
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    fn clipboard(&self, content: String) {
        self.record(format!("clipboard:{content}"));
    }
    fn cancel_msgbox(&self, tag: &str) {
        self.record(format!("cancel_msgbox:{tag}"));
    }
    fn switch_back(&self, id: &str) {
        self.record(format!("switch_back:{id}"));
    }
    fn portable_service_running(&self, running: bool) {
        self.record(format!("portable_service_running:{running}"));
    }
    fn on_voice_call_started(&self) {
        self.record("on_voice_call_started".to_owned());
    }
    fn on_voice_call_closed(&self, reason: &str) {
        self.record(format!("on_voice_call_closed:{reason}"));
    }
    fn on_voice_call_waiting(&self) {
        self.record("on_voice_call_waiting".to_owned());
    }
    fn on_voice_call_incoming(&self) {
        self.record("on_voice_call_incoming".to_owned());
    }
    fn get_rgba(&self, _display: usize) -> *const u8 {
        std::ptr::null()
    }
    fn next_rgba(&self, display: usize) {
        self.record(format!("next_rgba:{display}"));
    }
    #[cfg(all(feature = "vram", feature = "flutter"))]
    fn on_texture(&self, display: usize, _texture: *mut c_void) {
        self.record(format!("on_texture:{display}"));
    }
    fn set_multiple_windows_session(&self, sessions: Vec<WindowsSession>) {
        self.record(format!("set_multiple_windows_session:{}", sessions.len()));
    }
    fn set_current_display(&self, disp_idx: i32) {
        self.record(format!("set_current_display:{disp_idx}"));
    }
    #[cfg(feature = "flutter")]
    fn is_multi_ui_session(&self) -> bool {
        false
    }
    fn update_record_status(&self, start: bool) {
        self.record(format!("update_record_status:{start}"));
    }
    fn update_empty_dirs(&self, res: ReadEmptyDirsResponse) {
        self.record(format!("update_empty_dirs:{}", res.path));
    }
    fn handle_screenshot_resp(&self, sid: String, msg: String) {
        self.record(format!("handle_screenshot_resp:{sid}|{msg}"));
    }
    fn handle_terminal_response(&self, response: TerminalResponse) {
        self.record(format!("handle_terminal_response:{}", response.has_opened()));
    }
}
