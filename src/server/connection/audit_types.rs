use super::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AlarmAuditType {
    IpWhitelist = 0,
    ExceedThirtyAttempts = 1,
    SixAttemptsWithinOneMinute = 2,
    // ExceedThirtyLoginAttempts = 3,
    // MultipleLoginsAttemptsWithinOneMinute = 4,
    // MultipleLoginsAttemptsWithinOneHour = 5,
    ExceedIPv6PrefixAttempts = 6,
    TerminalOsLoginBackoff = 7,
    TerminalOsLoginConcurrency = 8,
    SessionScopeViolation = 9,
    IdWhitelist = 10,
}

pub enum FileAuditType {
    RemoteSend = 0,
    RemoteReceive = 1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileActionLog {
    pub(super) id: i32,
    pub(super) conn_id: i32,
    pub(super) path: String,
    pub(super) dir: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FileRenameLog {
    pub(super) conn_id: i32,
    pub(super) path: String,
    pub(super) new_name: String,
}

pub(super) struct FileRemoveLogControl {
    pub(super) conn_id: i32,
    pub(super) instant: Instant,
    pub(super) removed_files: Vec<FileRemoveFile>,
    pub(super) removed_dirs: Vec<FileRemoveDir>,
}

impl FileRemoveLogControl {
    pub(super) fn new(conn_id: i32) -> Self {
        FileRemoveLogControl {
            conn_id,
            instant: Instant::now(),
            removed_files: vec![],
            removed_dirs: vec![],
        }
    }

    pub(super) fn on_remove_file(&mut self, f: FileRemoveFile) -> Option<ipc::Data> {
        self.instant = Instant::now();
        self.removed_files.push(f.clone());
        Some(ipc::Data::FileTransferLog((
            "remove".to_string(),
            serde_json::to_string(&FileActionLog {
                id: f.id,
                conn_id: self.conn_id,
                path: f.path,
                dir: false,
            })
            .unwrap_or_default(),
        )))
    }

    pub(super) fn on_remove_dir(&mut self, d: FileRemoveDir) -> Option<ipc::Data> {
        self.instant = Instant::now();
        let direct_child = |parent: &str, child: &str| {
            PathBuf::from(child).parent().map(|x| x.to_path_buf()) == Some(PathBuf::from(parent))
        };
        self.removed_files
            .retain(|f| !direct_child(&f.path, &d.path));
        self.removed_dirs
            .retain(|x| !direct_child(&d.path, &x.path));
        if !self
            .removed_dirs
            .iter()
            .any(|x| direct_child(&x.path, &d.path))
        {
            self.removed_dirs.push(d.clone());
        }
        Some(ipc::Data::FileTransferLog((
            "remove".to_string(),
            serde_json::to_string(&FileActionLog {
                id: d.id,
                conn_id: self.conn_id,
                path: d.path,
                dir: true,
            })
            .unwrap_or_default(),
        )))
    }

    pub(super) fn on_timer(&mut self) -> Vec<ipc::Data> {
        if self.instant.elapsed().as_secs() < 1 {
            return vec![];
        }
        let mut v: Vec<ipc::Data> = vec![];
        self.removed_files
            .drain(..)
            .map(|f| {
                v.push(ipc::Data::FileTransferLog((
                    "remove".to_string(),
                    serde_json::to_string(&FileActionLog {
                        id: f.id,
                        conn_id: self.conn_id,
                        path: f.path,
                        dir: false,
                    })
                    .unwrap_or_default(),
                )));
            })
            .count();
        self.removed_dirs
            .drain(..)
            .map(|d| {
                v.push(ipc::Data::FileTransferLog((
                    "remove".to_string(),
                    serde_json::to_string(&FileActionLog {
                        id: d.id,
                        conn_id: self.conn_id,
                        path: d.path,
                        dir: true,
                    })
                    .unwrap_or_default(),
                )));
            })
            .count();
        v
    }
}
