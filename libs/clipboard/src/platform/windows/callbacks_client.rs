#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use super::*;

pub(super) extern "C" fn client_format_list(
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

pub(super) extern "C" fn client_format_list_response(
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

pub(super) extern "C" fn client_format_data_request(
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

pub(super) extern "C" fn client_format_data_response(
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

pub(super) extern "C" fn client_file_contents_request(
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

pub(super) extern "C" fn client_file_contents_response(
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
