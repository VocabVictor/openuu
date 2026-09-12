//! windows implementation
#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use crate::{
    send_data, send_data_exclude, ClipboardFile, CliprdrError, CliprdrServiceContext,
    ProgressPercent, ResultType, ERR_CODE_INVALID_PARAMETER, ERR_CODE_SEND_MSG,
    ERR_CODE_SERVER_FUNCTION_NONE, VEC_MSG_CHANNEL,
};
use hbb_common::{allow_err, log};
use std::{
    boxed::Box,
    ffi::{CStr, CString},
    result::Result,
};

// only used error code will be recorded here
/// success
const CHANNEL_RC_OK: u32 = 0;
/// error code from WinError.h
/// success
const ERROR_SUCCESS: u32 = 0;
/// allocation failure
const CHANNEL_RC_NO_MEMORY: u32 = 12;
/// error code from WinError.h
/// used by FreeRDP to represent errors.
const ERROR_INTERNAL_ERROR: u32 = 0x54F;

mod win_types;
pub use win_types::*;
mod cliprdr_types;
pub use cliprdr_types::*;
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
    fn wf_cliprdr_file_descriptor_name_valid(name: *const WCHAR) -> BOOL;
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

fn ret_to_result(ret: u32) -> Result<(), CliprdrError> {
    match ret {
        #[allow(unreachable_patterns)]
        // CHANNEL_RC_OK is unreachable, but ignore it
        ERROR_SUCCESS | CHANNEL_RC_OK => Ok(()),
        CHANNEL_RC_NO_MEMORY => Err(CliprdrError::CliprdrOutOfMemory),
        ERROR_INTERNAL_ERROR => Err(CliprdrError::ClipboardInternalError),
        e => Err(CliprdrError::Unknown(e)),
    }
}

pub fn empty_clipboard(context: &mut CliprdrClientContext, conn_id: i32) -> bool {
    unsafe { TRUE == empty_cliprdr(context, conn_id as u32) }
}

pub fn server_clip_file(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    msg: ClipboardFile,
) -> u32 {
    let mut ret = 0;
    match msg {
        ClipboardFile::NotifyCallback { .. } => {
            // unreachable
        }
        ClipboardFile::MonitorReady => {
            log::debug!("server_monitor_ready called");
            ret = server_monitor_ready(context, conn_id);
            log::debug!(
                "server_monitor_ready called, conn_id {}, return {}",
                conn_id,
                ret
            );
        }
        ClipboardFile::FormatList { format_list } => {
            log::debug!(
                "server_format_list called, conn_id {}, format_list: {:?}",
                conn_id,
                &format_list
            );
            send_data_exclude(conn_id as _, ClipboardFile::TryEmpty);
            ret = server_format_list(context, conn_id, format_list);
            log::debug!(
                "server_format_list called, conn_id {}, return {}",
                conn_id,
                ret
            );
        }
        ClipboardFile::FormatListResponse { msg_flags } => {
            log::debug!("server_format_list_response called");
            ret = server_format_list_response(context, conn_id, msg_flags);
            log::debug!(
                "server_format_list_response called, conn_id {}, msg_flags {}, return {}",
                conn_id,
                msg_flags,
                ret
            );
        }
        ClipboardFile::FormatDataRequest {
            requested_format_id,
        } => {
            log::debug!("server_format_data_request called");
            ret = server_format_data_request(context, conn_id, requested_format_id);
            log::debug!(
                "server_format_data_request called, conn_id {}, requested_format_id {}, return {}",
                conn_id,
                requested_format_id,
                ret
            );
        }
        ClipboardFile::FormatDataResponse {
            msg_flags,
            format_data,
        } => {
            log::debug!("server_format_data_response called");
            ret = server_format_data_response(context, conn_id, msg_flags, format_data);
            log::debug!(
                "server_format_data_response called, conn_id {}, msg_flags: {}, return {}",
                conn_id,
                msg_flags,
                ret
            );
        }
        ClipboardFile::FileContentsRequest {
            stream_id,
            list_index,
            dw_flags,
            n_position_low,
            n_position_high,
            cb_requested,
            have_clip_data_id,
            clip_data_id,
        } => {
            log::debug!("server_file_contents_request called");
            ret = server_file_contents_request(
                context,
                conn_id,
                stream_id,
                list_index,
                dw_flags,
                n_position_low,
                n_position_high,
                cb_requested,
                have_clip_data_id,
                clip_data_id,
            );
            log::debug!("server_file_contents_request called, conn_id {}, stream_id: {}, list_index {}, dw_flags {}, n_position_low {}, n_position_high {}, cb_requested {}, have_clip_data_id {}, clip_data_id {}, return {}",                 conn_id,
                stream_id,
                list_index,
                dw_flags,
                n_position_low,
                n_position_high,
                cb_requested,
                have_clip_data_id,
                clip_data_id,
                ret
            );
        }
        ClipboardFile::FileContentsResponse {
            msg_flags,
            stream_id,
            requested_data,
        } => {
            log::debug!("server_file_contents_response called");
            ret = server_file_contents_response(
                context,
                conn_id,
                msg_flags,
                stream_id,
                requested_data,
            );
            log::debug!("server_file_contents_response called, conn_id {}, msg_flags {}, stream_id {}, return {}",
                conn_id,
                msg_flags,
                stream_id,
                ret
            );
        }
        ClipboardFile::TryEmpty => {
            log::debug!("empty_clipboard called");
            let ret = empty_clipboard(context, conn_id);
            log::debug!(
                "empty_clipboard called, conn_id {}, return {}",
                conn_id,
                ret
            );
        }
        ClipboardFile::Files { .. } => {
            // unreachable
        }
    }
    ret
}

pub fn server_monitor_ready(context: &mut CliprdrClientContext, conn_id: i32) -> u32 {
    unsafe {
        let monitor_ready = CLIPRDR_MONITOR_READY {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
        };
        if let Some(f) = context.MonitorReady {
            let ret = f(context, &monitor_ready);
            ret as u32
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        }
    }
}

pub fn server_format_list(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    format_list: Vec<(i32, String)>,
) -> u32 {
    unsafe {
        let num_formats = format_list.len() as UINT32;
        let mut formats = format_list
            .into_iter()
            .map(|format| {
                if format.1.is_empty() {
                    CLIPRDR_FORMAT {
                        formatId: format.0 as UINT32,
                        formatName: 0 as *mut _,
                    }
                } else {
                    let n = match CString::new(format.1) {
                        Ok(n) => n,
                        Err(_) => CString::new("").unwrap_or_default(),
                    };
                    CLIPRDR_FORMAT {
                        formatId: format.0 as UINT32,
                        formatName: n.into_raw(),
                    }
                }
            })
            .collect::<Vec<CLIPRDR_FORMAT>>();

        let format_list = CLIPRDR_FORMAT_LIST {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            numFormats: num_formats,
            formats: formats.as_mut_ptr(),
        };

        let ret = if let Some(f) = context.ServerFormatList {
            f(context, &format_list)
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        };

        for f in formats {
            if !f.formatName.is_null() {
                // retake pointer to free memory
                let _ = CString::from_raw(f.formatName);
            }
        }

        ret as u32
    }
}

pub fn server_format_list_response(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    msg_flags: i32,
) -> u32 {
    unsafe {
        let format_list_response = CLIPRDR_FORMAT_LIST_RESPONSE {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: msg_flags as UINT16,
            dataLen: 0 as UINT32,
        };

        if let Some(f) = context.ServerFormatListResponse {
            f(context, &format_list_response)
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        }
    }
}

pub fn server_format_data_request(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    requested_format_id: i32,
) -> u32 {
    unsafe {
        let format_data_request = CLIPRDR_FORMAT_DATA_REQUEST {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            requestedFormatId: requested_format_id as UINT32,
        };
        if let Some(f) = context.ServerFormatDataRequest {
            f(context, &format_data_request)
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        }
    }
}

pub fn server_format_data_response(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    msg_flags: i32,
    mut format_data: Vec<u8>,
) -> u32 {
    unsafe {
        let format_data_response = CLIPRDR_FORMAT_DATA_RESPONSE {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: msg_flags as UINT16,
            dataLen: format_data.len() as UINT32,
            requestedFormatData: format_data.as_mut_ptr(),
        };
        if let Some(f) = context.ServerFormatDataResponse {
            f(context, &format_data_response)
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        }
    }
}

pub fn server_file_contents_request(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    stream_id: i32,
    list_index: i32,
    dw_flags: i32,
    n_position_low: i32,
    n_position_high: i32,
    cb_requested: i32,
    have_clip_data_id: bool,
    clip_data_id: i32,
) -> u32 {
    unsafe {
        let file_contents_request = CLIPRDR_FILE_CONTENTS_REQUEST {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: 0 as UINT16,
            dataLen: 0 as UINT32,
            streamId: stream_id as UINT32,
            listIndex: list_index as UINT32,
            dwFlags: dw_flags as UINT32,
            nPositionLow: n_position_low as UINT32,
            nPositionHigh: n_position_high as UINT32,
            cbRequested: cb_requested as UINT32,
            haveClipDataId: if have_clip_data_id { TRUE } else { FALSE },
            clipDataId: clip_data_id as UINT32,
        };
        if let Some(f) = context.ServerFileContentsRequest {
            f(context, &file_contents_request)
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        }
    }
}

pub fn server_file_contents_response(
    context: &mut CliprdrClientContext,
    conn_id: i32,
    msg_flags: i32,
    stream_id: i32,
    mut requested_data: Vec<u8>,
) -> u32 {
    unsafe {
        let file_contents_response = CLIPRDR_FILE_CONTENTS_RESPONSE {
            connID: conn_id as UINT32,
            msgType: 0 as UINT16,
            msgFlags: msg_flags as UINT16,
            dataLen: 4 + requested_data.len() as UINT32,
            streamId: stream_id as UINT32,
            cbRequested: requested_data.len() as UINT32,
            requestedData: requested_data.as_mut_ptr(),
        };
        if let Some(f) = context.ServerFileContentsResponse {
            f(context, &file_contents_response)
        } else {
            ERR_CODE_SERVER_FUNCTION_NONE
        }
    }
}

pub fn create_cliprdr_context(
    enable_files: bool,
    enable_others: bool,
    response_wait_timeout_secs: u32,
) -> ResultType<Box<CliprdrClientContext>> {
    Ok(CliprdrClientContext::create(
        enable_files,
        enable_others,
        response_wait_timeout_secs,
        Some(notify_callback),
        Some(handle_clipboard_files),
        Some(client_format_list),
        Some(client_format_list_response),
        Some(client_format_data_request),
        Some(client_format_data_response),
        Some(client_file_contents_request),
        Some(client_file_contents_response),
    )?)
}

extern "C" fn notify_callback(conn_id: UINT32, msg: *const NOTIFICATION_MESSAGE) -> UINT {
    log::debug!("notify_callback called");
    let data = unsafe {
        let msg = &*msg;
        let details = if msg.details.is_null() {
            Ok("")
        } else {
            CStr::from_ptr(msg.details as _).to_str()
        };
        match (CStr::from_ptr(msg.msg as _).to_str(), details) {
            (Ok(m), Ok(d)) => {
                let msgtype = format!(
                    "custom-{}-nocancel-nook-hasclose",
                    if msg.r#type == 0 {
                        "info"
                    } else if msg.r#type == 1 {
                        "warn"
                    } else {
                        "error"
                    }
                );
                let title = "Clipboard";
                let text = if d.is_empty() {
                    m.to_string()
                } else {
                    format!("{} {}", m, d)
                };
                ClipboardFile::NotifyCallback {
                    r#type: msgtype,
                    title: title.to_string(),
                    text,
                }
            }
            _ => {
                log::error!("notify_callback: failed to convert msg");
                return ERR_CODE_INVALID_PARAMETER;
            }
        }
    };
    // no need to handle result here
    allow_err!(send_data(conn_id as _, data));

    0
}

extern "C" fn handle_clipboard_files(
    conn_id: UINT32,
    n_files: size_t,
    file_names: *mut *mut WCHAR,
) -> UINT {
    if n_files == 0 {
        return 0;
    }

    let data = unsafe {
        let mut files = Vec::new();
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;
        for i in 0..n_files {
            let file_name_ptr = *file_names.offset(i as isize);
            if !file_name_ptr.is_null() {
                let mut len = 0;
                while *file_name_ptr.offset(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(file_name_ptr, len as usize);
                let os_string = OsString::from_wide(slice);
                match os_string.to_str() {
                    Some(n) => match std::fs::metadata(n) {
                        Ok(meta) => {
                            if meta.is_file() {
                                files.push((n.to_owned(), meta.len()));
                            }
                        }
                        Err(e) => {
                            log::warn!(
                                "handle_clipboard_files: Failed to get metadata for file '{}': {}",
                                n,
                                e
                            );
                        }
                    },
                    None => {
                        log::warn!("handle_clipboard_files: Failed to convert file name to UTF-8");
                    }
                };
            }
        }
        if files.is_empty() {
            return 0;
        }

        ClipboardFile::Files { files }
    };
    // no need to handle result here
    allow_err!(send_data(conn_id as _, data));

    0
}

extern "C" fn client_format_list(
    _context: *mut CliprdrClientContext,
    clip_format_list: *const CLIPRDR_FORMAT_LIST,
) -> UINT {
    let conn_id;
    let mut format_list: Vec<(i32, String)> = Vec::new();
    unsafe {
        let mut i = 0u32;
        while i < (*clip_format_list).numFormats {
            let format_data = &(*(*clip_format_list).formats.offset(i as isize));
            if format_data.formatName.is_null() {
                format_list.push((format_data.formatId as i32, "".to_owned()));
            } else {
                let format_name = CStr::from_ptr(format_data.formatName).to_str();
                let format_name = match format_name {
                    Ok(n) => n.to_owned(),
                    Err(_) => {
                        log::warn!("failed to get format name");
                        "".to_owned()
                    }
                };
                format_list.push((format_data.formatId as i32, format_name));
            }
            // log::debug!("format list item {}: format id: {}, format name: {}", i, format_data.formatId, &format_name);
            i += 1;
        }
        conn_id = (*clip_format_list).connID as i32;
    }
    log::debug!(
        "client_format_list called, client id: {}, format_list: {:?}",
        conn_id,
        &format_list
    );
    let data = ClipboardFile::FormatList { format_list };
    // no need to handle result here
    if conn_id == 0 {
        // msg_channel is used for debug, VEC_MSG_CHANNEL cannot be inspected by the debugger.
        let msg_channel = VEC_MSG_CHANNEL.read().unwrap();
        msg_channel
            .iter()
            .for_each(|msg_channel| allow_err!(msg_channel.sender.send(data.clone())));
    } else {
        match send_data(conn_id, data) {
            Ok(_) => {}
            Err(e) => {
                log::error!("failed to send format list: {:?}", e);
                return ERR_CODE_SEND_MSG;
            }
        }
    }

    0
}

extern "C" fn client_format_list_response(
    _context: *mut CliprdrClientContext,
    format_list_response: *const CLIPRDR_FORMAT_LIST_RESPONSE,
) -> UINT {
    let conn_id;
    let msg_flags;
    unsafe {
        conn_id = (*format_list_response).connID as i32;
        msg_flags = (*format_list_response).msgFlags as i32;
    }
    log::debug!(
        "client_format_list_response called, client id: {}, msg_flags: {}",
        conn_id,
        msg_flags
    );
    let data = ClipboardFile::FormatListResponse { msg_flags };
    match send_data(conn_id, data) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("failed to send format list response: {:?}", e);
            ERR_CODE_SEND_MSG
        }
    }
}

extern "C" fn client_format_data_request(
    _context: *mut CliprdrClientContext,
    format_data_request: *const CLIPRDR_FORMAT_DATA_REQUEST,
) -> UINT {
    let conn_id;
    let requested_format_id;
    unsafe {
        conn_id = (*format_data_request).connID as i32;
        requested_format_id = (*format_data_request).requestedFormatId as i32;
    }
    let data = ClipboardFile::FormatDataRequest {
        requested_format_id,
    };
    log::debug!(
        "client_format_data_request called, conn_id: {}, requested_format_id: {}",
        conn_id,
        requested_format_id
    );
    match send_data(conn_id, data) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("failed to send format data request: {:?}", e);
            ERR_CODE_SEND_MSG
        }
    }
}

extern "C" fn client_format_data_response(
    _context: *mut CliprdrClientContext,
    format_data_response: *const CLIPRDR_FORMAT_DATA_RESPONSE,
) -> UINT {
    let conn_id;
    let msg_flags;
    let format_data;
    unsafe {
        conn_id = (*format_data_response).connID as i32;
        msg_flags = (*format_data_response).msgFlags as i32;
        if (*format_data_response).requestedFormatData.is_null() {
            format_data = Vec::new();
        } else {
            format_data = std::slice::from_raw_parts(
                (*format_data_response).requestedFormatData,
                (*format_data_response).dataLen as usize,
            )
            .to_vec();
        }
    }
    log::debug!(
        "client_format_data_response called, client id: {}, msg_flags: {}",
        conn_id,
        msg_flags
    );
    let data = ClipboardFile::FormatDataResponse {
        msg_flags,
        format_data,
    };
    match send_data(conn_id, data) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("failed to send format data response: {:?}", e);
            ERR_CODE_SEND_MSG
        }
    }
}

extern "C" fn client_file_contents_request(
    _context: *mut CliprdrClientContext,
    file_contents_request: *const CLIPRDR_FILE_CONTENTS_REQUEST,
) -> UINT {
    // TODO: support huge file?
    // if (!cliprdr->hasHugeFileSupport)
    // {
    // 	if (((UINT64)fileContentsRequest->cbRequested + fileContentsRequest->nPositionLow) >
    // 	    UINT32_MAX)
    // 		return ERROR_INVALID_PARAMETER;
    // 	if (fileContentsRequest->nPositionHigh != 0)
    // 		return ERROR_INVALID_PARAMETER;
    // }

    let conn_id;
    let stream_id;
    let list_index;
    let dw_flags;
    let n_position_low;
    let n_position_high;
    let cb_requested;
    let have_clip_data_id;
    let clip_data_id;
    unsafe {
        conn_id = (*file_contents_request).connID as i32;
        stream_id = (*file_contents_request).streamId as i32;
        list_index = (*file_contents_request).listIndex as i32;
        dw_flags = (*file_contents_request).dwFlags as i32;
        n_position_low = (*file_contents_request).nPositionLow as i32;
        n_position_high = (*file_contents_request).nPositionHigh as i32;
        cb_requested = (*file_contents_request).cbRequested as i32;
        have_clip_data_id = (*file_contents_request).haveClipDataId == TRUE;
        clip_data_id = (*file_contents_request).clipDataId as i32;
    }
    let data = ClipboardFile::FileContentsRequest {
        stream_id,
        list_index,
        dw_flags,
        n_position_low,
        n_position_high,
        cb_requested,
        have_clip_data_id,
        clip_data_id,
    };
    log::debug!("client_file_contents_request called, data: {:?}", &data);
    match send_data(conn_id, data) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("failed to send file contents request: {:?}", e);
            ERR_CODE_SEND_MSG
        }
    }
}

extern "C" fn client_file_contents_response(
    _context: *mut CliprdrClientContext,
    file_contents_response: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
) -> UINT {
    let conn_id;
    let msg_flags;
    let stream_id;
    let requested_data;
    unsafe {
        conn_id = (*file_contents_response).connID as i32;
        msg_flags = (*file_contents_response).msgFlags as i32;
        stream_id = (*file_contents_response).streamId as i32;
        if (*file_contents_response).requestedData.is_null() {
            requested_data = Vec::new();
        } else {
            requested_data = std::slice::from_raw_parts(
                (*file_contents_response).requestedData,
                (*file_contents_response).cbRequested as usize,
            )
            .to_vec();
        }
    }
    let data = ClipboardFile::FileContentsResponse {
        msg_flags,
        stream_id,
        requested_data,
    };
    log::debug!(
        "client_file_contents_response called, conn_id: {}, msg_flags: {}, stream_id: {}",
        conn_id,
        msg_flags,
        stream_id
    );
    match send_data(conn_id, data) {
        Ok(_) => 0,
        Err(e) => {
            log::error!("failed to send file contents response: {:?}", e);
            ERR_CODE_SEND_MSG
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::once;

    const FILE_NAME_CODE_UNITS: usize = 260;
    const FILE_NAME_CASES: &[(&str, bool)] = &[
        ("", false),
        ("/absolute", false),
        ("C:\\absolute", false),
        ("dir\\..\\payload", false),
        ("dir//payload", false),
        ("file.", false),
        ("file ", false),
        (" report.txt", false),
        (" NUL.txt", false),
        ("dir\\ nested.txt", false),
        ("CON", false),
        ("nul.txt", false),
        ("dir\\AUX.log", false),
        ("PRN.tar.gz", false),
        ("com1", false),
        ("COM\u{00b9}.txt", false),
        ("COM\u{00b2}.txt", false),
        ("lpt9.log", false),
        ("dir/LPT\u{00b3}", false),
        ("CONIN$", false),
        ("dir\\conout$", false),
        ("CLOCK$", false),
        ("bad<name", false),
        ("bad>name", false),
        ("bad:name", false),
        ("bad\"name", false),
        ("bad|name", false),
        ("bad?name", false),
        ("bad*name", false),
        ("bad\u{0001}name", false),
        ("dir\\bad\u{001f}name", false),
        ("normal.txt", true),
        (".gitignore", true),
        ("dir\\nested file.txt", true),
        ("dir/nested file.txt", true),
        ("com10.txt", true),
        ("auxiliary.log", true),
        ("clock$.txt", true),
        ("conin$.txt", true),
    ];

    fn file_descriptor_name_valid(name: &str) -> bool {
        let wide_name: Vec<_> = name.encode_utf16().chain(once(0)).collect();
        unsafe { wf_cliprdr_file_descriptor_name_valid(wide_name.as_ptr()) == TRUE }
    }

    #[test]
    fn validates_file_descriptor_names() {
        for &(name, expected) in FILE_NAME_CASES {
            assert_eq!(
                file_descriptor_name_valid(name),
                expected,
                "unexpected validity for {name:?}"
            );
        }
    }

    #[test]
    fn rejects_non_terminated_file_descriptor_name() {
        let wide_name = [WCHAR::from(b'a'); FILE_NAME_CODE_UNITS];
        assert_eq!(
            unsafe { wf_cliprdr_file_descriptor_name_valid(wide_name.as_ptr()) },
            FALSE
        );
    }
}
