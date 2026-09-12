#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use super::*;

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
