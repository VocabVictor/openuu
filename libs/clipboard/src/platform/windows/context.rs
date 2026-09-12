#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use super::*;

// TODO: hide more members of clipboard context
#[repr(C)]
#[derive(Debug, Clone)]
pub struct _cliprdr_client_context {
    pub Custom: *mut ::std::os::raw::c_void,
    pub EnableFiles: BOOL,
    pub EnableOthers: BOOL,
    pub IsStopped: BOOL,
    pub ResponseWaitTimeoutSecs: UINT32,
    pub ServerCapabilities: pcCliprdrServerCapabilities,
    pub ClientCapabilities: pcCliprdrClientCapabilities,
    pub MonitorReady: pcCliprdrMonitorReady,
    pub TempDirectory: pcCliprdrTempDirectory,
    pub NotifyClipboardMsg: pcNotifyClipboardMsg,
    pub HandleClipboardFiles: pcHandleClipboardFiles,
    pub ClientFormatList: pcCliprdrClientFormatList,
    pub ServerFormatList: pcCliprdrServerFormatList,
    pub ClientFormatListResponse: pcCliprdrClientFormatListResponse,
    pub ServerFormatListResponse: pcCliprdrServerFormatListResponse,
    pub ClientLockClipboardData: pcCliprdrClientLockClipboardData,
    pub ServerLockClipboardData: pcCliprdrServerLockClipboardData,
    pub ClientUnlockClipboardData: pcCliprdrClientUnlockClipboardData,
    pub ServerUnlockClipboardData: pcCliprdrServerUnlockClipboardData,
    pub ClientFormatDataRequest: pcCliprdrClientFormatDataRequest,
    pub ServerFormatDataRequest: pcCliprdrServerFormatDataRequest,
    pub ClientFormatDataResponse: pcCliprdrClientFormatDataResponse,
    pub ServerFormatDataResponse: pcCliprdrServerFormatDataResponse,
    pub ClientFileContentsRequest: pcCliprdrClientFileContentsRequest,
    pub ServerFileContentsRequest: pcCliprdrServerFileContentsRequest,
    pub ClientFileContentsResponse: pcCliprdrClientFileContentsResponse,
    pub ServerFileContentsResponse: pcCliprdrServerFileContentsResponse,
    pub LastRequestedFormatId: UINT32,
}

// #[link(name = "user32")]
// #[link(name = "ole32")]
extern "C" {
    pub(crate) fn init_cliprdr(context: *mut CliprdrClientContext) -> BOOL;
    pub(crate) fn uninit_cliprdr(context: *mut CliprdrClientContext) -> BOOL;
    pub(crate) fn empty_cliprdr(context: *mut CliprdrClientContext, connID: UINT32) -> BOOL;
    #[cfg(test)]
    pub(super) fn wf_cliprdr_file_descriptor_name_valid(name: *const WCHAR) -> BOOL;
}

unsafe impl Send for CliprdrClientContext {}

unsafe impl Sync for CliprdrClientContext {}

impl CliprdrClientContext {
    pub fn create(
        enable_files: bool,
        enable_others: bool,
        response_wait_timeout_secs: u32,
        notify_callback: pcNotifyClipboardMsg,
        handle_clipboard_files: pcHandleClipboardFiles,
        client_format_list: pcCliprdrClientFormatList,
        client_format_list_response: pcCliprdrClientFormatListResponse,
        client_format_data_request: pcCliprdrClientFormatDataRequest,
        client_format_data_response: pcCliprdrClientFormatDataResponse,
        client_file_contents_request: pcCliprdrClientFileContentsRequest,
        client_file_contents_response: pcCliprdrClientFileContentsResponse,
    ) -> Result<Box<Self>, CliprdrError> {
        let context = CliprdrClientContext {
            Custom: 0 as *mut _,
            EnableFiles: if enable_files { TRUE } else { FALSE },
            EnableOthers: if enable_others { TRUE } else { FALSE },
            IsStopped: FALSE,
            ResponseWaitTimeoutSecs: response_wait_timeout_secs,
            ServerCapabilities: None,
            ClientCapabilities: None,
            MonitorReady: None,
            TempDirectory: None,
            NotifyClipboardMsg: notify_callback,
            HandleClipboardFiles: handle_clipboard_files,
            ClientFormatList: client_format_list,
            ServerFormatList: None,
            ClientFormatListResponse: client_format_list_response,
            ServerFormatListResponse: None,
            ClientLockClipboardData: None,
            ServerLockClipboardData: None,
            ClientUnlockClipboardData: None,
            ServerUnlockClipboardData: None,
            ClientFormatDataRequest: client_format_data_request,
            ServerFormatDataRequest: None,
            ClientFormatDataResponse: client_format_data_response,
            ServerFormatDataResponse: None,
            ClientFileContentsRequest: client_file_contents_request,
            ServerFileContentsRequest: None,
            ClientFileContentsResponse: client_file_contents_response,
            ServerFileContentsResponse: None,
            LastRequestedFormatId: 0,
        };
        let mut context = Box::new(context);
        unsafe {
            if FALSE == init_cliprdr(&mut (*context)) {
                println!("Failed to init cliprdr");
                Err(CliprdrError::CliprdrInit)
            } else {
                Ok(context)
            }
        }
    }
}

impl Drop for CliprdrClientContext {
    fn drop(&mut self) {
        unsafe {
            if FALSE == uninit_cliprdr(&mut *self) {
                println!("Failed to uninit cliprdr");
            } else {
                println!("Succeeded to uninit cliprdr");
            }
        }
    }
}

impl CliprdrServiceContext for CliprdrClientContext {
    fn set_is_stopped(&mut self) -> Result<(), CliprdrError> {
        self.IsStopped = TRUE;
        Ok(())
    }

    fn empty_clipboard(&mut self, conn_id: i32) -> Result<bool, CliprdrError> {
        Ok(empty_clipboard(self, conn_id))
    }

    fn server_clip_file(&mut self, conn_id: i32, msg: ClipboardFile) -> Result<(), CliprdrError> {
        let ret = server_clip_file(self, conn_id, msg);
        ret_to_result(ret)
    }

    fn get_progress_percent(&self) -> Option<ProgressPercent> {
        None
    }

    fn cancel(&mut self) {}
}

pub(super) fn ret_to_result(ret: u32) -> Result<(), CliprdrError> {
    match ret {
        #[allow(unreachable_patterns)]
        // CHANNEL_RC_OK is unreachable, but ignore it
        ERROR_SUCCESS | CHANNEL_RC_OK => Ok(()),
        CHANNEL_RC_NO_MEMORY => Err(CliprdrError::CliprdrOutOfMemory),
        ERROR_INTERNAL_ERROR => Err(CliprdrError::ClipboardInternalError),
        e => Err(CliprdrError::Unknown(e)),
    }
}
