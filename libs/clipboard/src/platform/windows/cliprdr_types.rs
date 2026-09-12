#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(unused_variables)]
#![allow(non_snake_case)]
#![allow(deref_nullptr)]

use super::*;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_HEADER {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
}
pub type CLIPRDR_HEADER = _CLIPRDR_HEADER;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_CAPABILITY_SET {
    pub capabilitySetType: UINT16,
    pub capabilitySetLength: UINT16,
}
pub type CLIPRDR_CAPABILITY_SET = _CLIPRDR_CAPABILITY_SET;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_GENERAL_CAPABILITY_SET {
    pub capabilitySetType: UINT16,
    pub capabilitySetLength: UINT16,
    pub version: UINT32,
    pub generalFlags: UINT32,
}
pub type CLIPRDR_GENERAL_CAPABILITY_SET = _CLIPRDR_GENERAL_CAPABILITY_SET;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_CAPABILITIES {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub cCapabilitiesSets: UINT32,
    pub capabilitySets: *mut CLIPRDR_CAPABILITY_SET,
}
pub type CLIPRDR_CAPABILITIES = _CLIPRDR_CAPABILITIES;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_MONITOR_READY {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
}
pub type CLIPRDR_MONITOR_READY = _CLIPRDR_MONITOR_READY;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_TEMP_DIRECTORY {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub szTempDir: [::std::os::raw::c_char; 520usize],
}
pub type CLIPRDR_TEMP_DIRECTORY = _CLIPRDR_TEMP_DIRECTORY;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT {
    pub formatId: UINT32,
    pub formatName: *mut ::std::os::raw::c_char,
}
pub type CLIPRDR_FORMAT = _CLIPRDR_FORMAT;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_LIST {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub numFormats: UINT32,
    pub formats: *mut CLIPRDR_FORMAT,
}
pub type CLIPRDR_FORMAT_LIST = _CLIPRDR_FORMAT_LIST;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_LIST_RESPONSE {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
}
pub type CLIPRDR_FORMAT_LIST_RESPONSE = _CLIPRDR_FORMAT_LIST_RESPONSE;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_LOCK_CLIPBOARD_DATA {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub clipDataId: UINT32,
}
pub type CLIPRDR_LOCK_CLIPBOARD_DATA = _CLIPRDR_LOCK_CLIPBOARD_DATA;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_UNLOCK_CLIPBOARD_DATA {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub clipDataId: UINT32,
}
pub type CLIPRDR_UNLOCK_CLIPBOARD_DATA = _CLIPRDR_UNLOCK_CLIPBOARD_DATA;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_DATA_REQUEST {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub requestedFormatId: UINT32,
}
pub type CLIPRDR_FORMAT_DATA_REQUEST = _CLIPRDR_FORMAT_DATA_REQUEST;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FORMAT_DATA_RESPONSE {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub requestedFormatData: *const BYTE,
}
pub type CLIPRDR_FORMAT_DATA_RESPONSE = _CLIPRDR_FORMAT_DATA_RESPONSE;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FILE_CONTENTS_REQUEST {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub streamId: UINT32,
    pub listIndex: UINT32,
    pub dwFlags: UINT32,
    pub nPositionLow: UINT32,
    pub nPositionHigh: UINT32,
    pub cbRequested: UINT32,
    pub haveClipDataId: BOOL,
    pub clipDataId: UINT32,
}
pub type CLIPRDR_FILE_CONTENTS_REQUEST = _CLIPRDR_FILE_CONTENTS_REQUEST;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _CLIPRDR_FILE_CONTENTS_RESPONSE {
    pub connID: UINT32,
    pub msgType: UINT16,
    pub msgFlags: UINT16,
    pub dataLen: UINT32,
    pub streamId: UINT32,
    pub cbRequested: UINT32,
    pub requestedData: *const BYTE,
}
pub type CLIPRDR_FILE_CONTENTS_RESPONSE = _CLIPRDR_FILE_CONTENTS_RESPONSE;
pub type CliprdrClientContext = _cliprdr_client_context;
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct _NOTIFICATION_MESSAGE {
    pub r#type: UINT32, // 0 - info, 1 - warning, 2 - error
    pub msg: *const BYTE,
    pub details: *const BYTE,
}
pub type NOTIFICATION_MESSAGE = _NOTIFICATION_MESSAGE;
pub type pcCliprdrServerCapabilities = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        capabilities: *const CLIPRDR_CAPABILITIES,
    ) -> UINT,
>;
pub type pcCliprdrClientCapabilities = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        capabilities: *const CLIPRDR_CAPABILITIES,
    ) -> UINT,
>;
pub type pcCliprdrMonitorReady = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        monitorReady: *const CLIPRDR_MONITOR_READY,
    ) -> UINT,
>;
pub type pcCliprdrTempDirectory = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        tempDirectory: *const CLIPRDR_TEMP_DIRECTORY,
    ) -> UINT,
>;
pub type pcNotifyClipboardMsg = ::std::option::Option<
    unsafe extern "C" fn(connID: UINT32, msg: *const NOTIFICATION_MESSAGE) -> UINT,
>;
pub type pcHandleClipboardFiles = ::std::option::Option<
    unsafe extern "C" fn(connID: UINT32, nFiles: size_t, fileNames: *mut *mut WCHAR) -> UINT,
>;
pub type pcCliprdrClientFormatList = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatList: *const CLIPRDR_FORMAT_LIST,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatList = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatList: *const CLIPRDR_FORMAT_LIST,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatListResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatListResponse: *const CLIPRDR_FORMAT_LIST_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatListResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatListResponse: *const CLIPRDR_FORMAT_LIST_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrClientLockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        lockClipboardData: *const CLIPRDR_LOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrServerLockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        lockClipboardData: *const CLIPRDR_LOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrClientUnlockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        unlockClipboardData: *const CLIPRDR_UNLOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrServerUnlockClipboardData = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        unlockClipboardData: *const CLIPRDR_UNLOCK_CLIPBOARD_DATA,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatDataRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataRequest: *const CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatDataRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataRequest: *const CLIPRDR_FORMAT_DATA_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrClientFormatDataResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataResponse: *const CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrServerFormatDataResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        formatDataResponse: *const CLIPRDR_FORMAT_DATA_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrClientFileContentsRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsRequest: *const CLIPRDR_FILE_CONTENTS_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrServerFileContentsRequest = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsRequest: *const CLIPRDR_FILE_CONTENTS_REQUEST,
    ) -> UINT,
>;
pub type pcCliprdrClientFileContentsResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsResponse: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
    ) -> UINT,
>;
pub type pcCliprdrServerFileContentsResponse = ::std::option::Option<
    unsafe extern "C" fn(
        context: *mut CliprdrClientContext,
        fileContentsResponse: *const CLIPRDR_FILE_CONTENTS_RESPONSE,
    ) -> UINT,
>;
