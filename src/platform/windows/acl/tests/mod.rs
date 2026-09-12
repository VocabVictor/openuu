use super::{
    current_process_user_sid_string, set_path_permission,
    set_path_permission_for_portable_service_shmem_dir,
    set_path_permission_for_portable_service_shmem_file, sid_string_to_local_alloc_guard,
    LocalAllocGuard, ResultType,
};
use hbb_common::bail;
use std::{
    fs,
    os::windows::{ffi::OsStrExt, fs::symlink_dir, fs::symlink_file},
    path::{Path, PathBuf},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Security::{
            AclSizeInformation,
            Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
            EqualSid as WinEqualSid, GetAce, GetAclInformation, GetSecurityDescriptorControl,
            ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, ACL_SIZE_INFORMATION,
            DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED,
        },
        Storage::FileSystem::{
            FILE_ALL_ACCESS, FILE_GENERIC_EXECUTE, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        },
    },
};

const ACCESS_ALLOWED_ACE_TYPE_U8: u8 = 0;

fn unique_acl_test_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rustdesk_acl_{}_{}_{}",
        prefix,
        std::process::id(),
        hbb_common::rand::random::<u32>()
    ))
}

fn try_create_dir_reparse_point(target: &Path, link: &Path, test_name: &str) -> bool {
    match symlink_dir(target, link) {
        Ok(()) => true,
        Err(err) => {
            eprintln!(
                "skip {}: failed to create directory reparse point (symlink): {}",
                test_name, err
            );
            false
        }
    }
}

fn try_create_file_reparse_point(target: &Path, link: &Path, test_name: &str) -> bool {
    match symlink_file(target, link) {
        Ok(()) => true,
        Err(err) => {
            eprintln!(
                "skip {}: failed to create file reparse point (symlink): {}",
                test_name, err
            );
            false
        }
    }
}

fn get_file_dacl(path: &Path) -> ResultType<(*mut ACL, LocalAllocGuard)> {
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut sd = PSECURITY_DESCRIPTOR::default();
    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let result = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut dacl),
            None,
            &mut sd,
        )
    };
    if result.0 != 0 {
        bail!(
            "GetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            result.0
        );
    }
    if dacl.is_null() || sd.0.is_null() {
        bail!("DACL/security descriptor missing for '{}'", path.display());
    }
    Ok((dacl, LocalAllocGuard(sd.0)))
}

fn has_allow_ace_with_mask(
    dacl: *const ACL,
    sid_ptr: *mut std::ffi::c_void,
    mask: u32,
) -> bool {
    let mut info = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl,
            &mut info as *mut _ as *mut std::ffi::c_void,
            std::mem::size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    }
    .is_err()
    {
        return false;
    }
    for index in 0..info.AceCount {
        let mut ace_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        if unsafe { GetAce(dacl, index, &mut ace_ptr) }.is_err() || ace_ptr.is_null() {
            continue;
        }
        let header = unsafe { &*(ace_ptr as *const ACE_HEADER) };
        if header.AceType != ACCESS_ALLOWED_ACE_TYPE_U8 {
            continue;
        }
        let allowed = unsafe { &*(ace_ptr as *const ACCESS_ALLOWED_ACE) };
        let ace_sid = PSID((&allowed.SidStart as *const u32) as *mut std::ffi::c_void);
        if unsafe { WinEqualSid(PSID(sid_ptr), ace_sid) }.is_ok()
            && (allowed.Mask & mask) == mask
        {
            return true;
        }
    }
    false
}

fn has_any_allow_ace_for_sid(dacl: *const ACL, sid_ptr: *mut std::ffi::c_void) -> bool {
    has_allow_ace_with_mask(dacl, sid_ptr, 0)
}

fn is_dacl_protected(sd: PSECURITY_DESCRIPTOR) -> bool {
    let mut control: u16 = 0;
    let mut revision: u32 = 0;
    if unsafe { GetSecurityDescriptorControl(sd, &mut control, &mut revision) }.is_err() {
        return false;
    }
    (control & SE_DACL_PROTECTED.0) != 0
}


mod policy;
mod reparse;
