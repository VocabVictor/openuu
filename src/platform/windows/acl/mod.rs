// https://learn.microsoft.com/en-us/windows/win32/secgloss/security-glossary

use super::{read_token_user_buffer, wide_string, ResultType};
use hbb_common::{anyhow::anyhow, bail};
use std::{
    fs, io,
    os::windows::{ffi::OsStrExt, fs::MetadataExt},
    path::Path,
};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{CloseHandle, LocalFree, HANDLE, HLOCAL},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSidToSidW, GetNamedSecurityInfoW,
                SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, SET_ACCESS,
                SE_FILE_OBJECT, TRUSTEE_IS_GROUP, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
            },
            ACE_FLAGS, ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
            OBJECT_INHERIT_ACE, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
            TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::{FILE_ALL_ACCESS, FILE_GENERIC_WRITE},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
};
mod shmem;
mod sid;
pub use shmem::*;
pub(crate) use sid::*;

const FILE_ATTRIBUTE_REPARSE_POINT_U32: u32 = 0x400;

#[inline]
fn is_reparse_point(metadata: &fs::Metadata) -> bool {
    (metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_U32) != 0
}

fn apply_grant_sid_allow_ace_to_path(
    path: &Path,
    sid_ptr: *mut std::ffi::c_void,
    access_mask: u32,
    is_group: bool,
    is_dir: bool,
) -> ResultType<()> {
    // Merge mode: read existing DACL and append/replace ACE via SetEntriesInAclW.
    // https://learn.microsoft.com/en-us/windows/win32/secauthz/modifying-the-acls-of-an-object-in-c--
    let mut old_dacl: *mut ACL = std::ptr::null_mut();
    let mut security_descriptor = PSECURITY_DESCRIPTOR::default();
    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let get_named_result = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl),
            None,
            &mut security_descriptor,
        )
    };
    if get_named_result.0 != 0 {
        bail!(
            "GetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            get_named_result.0
        );
    }
    let _sd_guard = LocalAllocGuard(security_descriptor.0);

    let inherit_flags = if is_dir {
        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
    } else {
        NO_INHERITANCE
    };
    let explicit_access = [make_sid_trustee_entry(
        sid_ptr,
        access_mask,
        inherit_flags,
        is_group,
    )];
    let old_acl_option = if old_dacl.is_null() {
        None
    } else {
        Some(old_dacl as *const ACL)
    };
    let mut new_acl: *mut ACL = std::ptr::null_mut();
    let set_entries_result = unsafe {
        SetEntriesInAclW(
            Some(explicit_access.as_slice()),
            old_acl_option,
            &mut new_acl,
        )
    };
    if set_entries_result.0 != 0 {
        bail!(
            "SetEntriesInAclW failed for '{}': win32_error={}",
            path.display(),
            set_entries_result.0
        );
    }
    if new_acl.is_null() {
        bail!(
            "SetEntriesInAclW returned null ACL for '{}'",
            path.display()
        );
    }
    let _acl_guard = LocalAllocGuard(new_acl as *mut std::ffi::c_void);

    let set_named_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    if set_named_result.0 != 0 {
        bail!(
            "SetNamedSecurityInfoW failed for '{}': win32_error={}",
            path.display(),
            set_named_result.0
        );
    }
    Ok(())
}

/// Grants `Everyone` on `dir` recursively for helper/runtime files that must be
/// readable/executable across user contexts.
///
/// `access_mask` is the Win32 file access mask to grant recursively.
pub fn set_path_permission(dir: &Path, access_mask: u32) -> ResultType<()> {
    let metadata = fs::symlink_metadata(dir).map_err(|e| {
        anyhow!(
            "Failed to inspect ACL target directory '{}': {}",
            dir.display(),
            e
        )
    })?;
    if is_reparse_point(&metadata) {
        bail!(
            "ACL target directory is a reparse point and is rejected: '{}'",
            dir.display()
        );
    }
    if !metadata.file_type().is_dir() {
        bail!("ACL target is not a directory: '{}'", dir.display());
    }

    let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0")?;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        let metadata = fs::symlink_metadata(&path)
            .map_err(|e| anyhow!("Failed to inspect ACL target '{}': {}", path.display(), e))?;
        if is_reparse_point(&metadata) {
            continue;
        }
        let is_dir = metadata.file_type().is_dir();
        apply_grant_sid_allow_ace_to_path(
            &path,
            everyone_sid.as_sid_ptr(),
            access_mask,
            true,
            is_dir,
        )?;
        if !is_dir {
            continue;
        }
        for entry in fs::read_dir(&path)
            .map_err(|e| anyhow!("Failed to list ACL target dir '{}': {}", path.display(), e))?
        {
            let entry = entry.map_err(|e| {
                anyhow!(
                    "Failed to read ACL target dir entry under '{}': {}",
                    path.display(),
                    e
                )
            })?;
            stack.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
