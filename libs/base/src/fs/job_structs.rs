use super::*;

#[derive(Default, Serialize, Deserialize, Debug)]
pub struct FileDigest {
    pub size: u64,
    pub modified: u64,
}

#[derive(Default, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransferJob {
    pub id: i32,
    pub r#type: JobType,
    pub remote: String,
    pub data_source: DataSource,
    pub show_hidden: bool,
    pub is_remote: bool,
    pub is_last_job: bool,
    #[serde(skip_serializing)]
    pub paused: bool,
    #[serde(skip_serializing)]
    pub(super) confirmation_window: usize,
    #[serde(skip_serializing)]
    pub(super) prefetched: std::collections::HashSet<i32>,
    #[serde(skip_serializing)]
    pub(super) confirmations: std::collections::HashMap<i32, FileTransferSendConfirmRequest>,
    #[serde(skip_serializing)]
    pub(super) file_digests: std::collections::HashMap<i32, FileDigest>,
    pub is_resume: bool,
    pub file_num: i32,
    #[serde(skip_serializing)]
    pub(super) files: Vec<FileEntry>,
    pub conn_id: i32, // server only

    #[serde(skip_serializing)]
    pub(super) data_stream: Option<DataStream>,
    pub total_size: u64,
    pub(super) finished_size: u64,
    pub(super) transferred: u64,
    pub(super) enable_overwrite_detection: bool,
    pub(super) file_confirmed: bool,
    // indicating the last file is skipped
    pub(super) file_skipped: bool,
    pub(super) file_is_waiting: bool,
    pub(super) default_overwrite_strategy: Option<bool>,
    #[serde(skip_serializing)]
    pub(super) digest: FileDigest,
    #[serde(skip_serializing)]
    pub(super) compression: TransferCompression,
}

// Reprobe periodically so mixed-content files can regain compression.
#[derive(Debug, Default)]
pub(super) struct TransferCompression {
    pub(super) skip_blocks: u8,
}

impl TransferCompression {
    pub(super) fn encode(&mut self, data: &[u8]) -> Option<Vec<u8>> {
        if self.skip_blocks > 0 {
            self.skip_blocks -= 1;
            return None;
        }
        let encoded = compress(data);
        if !encoded.is_empty() && encoded.len() < data.len() - data.len() / 32 {
            Some(encoded)
        } else {
            self.skip_blocks = 31;
            None
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct TransferJobMeta {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub remote: String,
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub show_hidden: bool,
    #[serde(default)]
    pub file_num: i32,
    #[serde(default)]
    pub is_remote: bool,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct RemoveJobMeta {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub is_remote: bool,
    #[serde(default)]
    pub no_confirm: bool,
}
