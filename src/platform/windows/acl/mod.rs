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
mod tests {
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

    #[test]
    fn test_portable_service_shmem_dir_acl_policy() {
        let dir = unique_acl_test_path("dir");
        fs::create_dir_all(&dir).unwrap();
        set_path_permission_for_portable_service_shmem_dir(&dir).unwrap();

        let (dacl, sd_guard) = get_file_dacl(&dir).unwrap();
        let current_user_sid =
            sid_string_to_local_alloc_guard(&current_process_user_sid_string().unwrap()).unwrap();
        let system_sid = sid_string_to_local_alloc_guard("S-1-5-18").unwrap();
        let admin_sid = sid_string_to_local_alloc_guard("S-1-5-32-544").unwrap();
        let auth_users_sid = sid_string_to_local_alloc_guard("S-1-5-11").unwrap();
        let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
        let users_sid = sid_string_to_local_alloc_guard("S-1-5-32-545").unwrap();

        assert!(has_allow_ace_with_mask(
            dacl,
            system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            admin_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            auth_users_sid.as_sid_ptr(),
            FILE_GENERIC_WRITE.0
        ));
        assert!(!has_any_allow_ace_for_sid(dacl, everyone_sid.as_sid_ptr()));
        assert!(!has_any_allow_ace_for_sid(dacl, users_sid.as_sid_ptr()));
        assert!(is_dacl_protected(PSECURITY_DESCRIPTOR(
            sd_guard.as_sid_ptr()
        )));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_policy() {
        let dir = unique_acl_test_path("file");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shared_memory_portable_service_test");
        fs::write(&file, b"x").unwrap();
        set_path_permission_for_portable_service_shmem_file(&file).unwrap();

        let (dacl, sd_guard) = get_file_dacl(&file).unwrap();
        let current_user_sid =
            sid_string_to_local_alloc_guard(&current_process_user_sid_string().unwrap()).unwrap();
        let system_sid = sid_string_to_local_alloc_guard("S-1-5-18").unwrap();
        let admin_sid = sid_string_to_local_alloc_guard("S-1-5-32-544").unwrap();
        let auth_users_sid = sid_string_to_local_alloc_guard("S-1-5-11").unwrap();
        let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
        let users_sid = sid_string_to_local_alloc_guard("S-1-5-32-545").unwrap();

        assert!(has_allow_ace_with_mask(
            dacl,
            system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            admin_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(has_allow_ace_with_mask(
            dacl,
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0
        ));
        assert!(!has_any_allow_ace_for_sid(
            dacl,
            auth_users_sid.as_sid_ptr()
        ));
        assert!(!has_any_allow_ace_for_sid(dacl, everyone_sid.as_sid_ptr()));
        assert!(!has_any_allow_ace_for_sid(dacl, users_sid.as_sid_ptr()));
        assert!(is_dacl_protected(PSECURITY_DESCRIPTOR(
            sd_guard.as_sid_ptr()
        )));

        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_set_path_permission_rx_applies_recursively() {
        let root = unique_acl_test_path("set_path_permission");
        let child_dir = root.join("child");
        let child_file = child_dir.join("helper.exe");
        fs::create_dir_all(&child_dir).unwrap();
        fs::write(&child_file, b"x").unwrap();

        if let Err(err) = set_path_permission(&root, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0) {
            let text = err.to_string();
            let _ = fs::remove_file(&child_file);
            let _ = fs::remove_dir_all(&root);
            if text.contains("win32_error=5") || text.contains("Access is denied") {
                eprintln!(
                    "skip test_set_path_permission_rx_applies_recursively: insufficient WRITE_DAC in current environment: {}",
                    text
                );
                return;
            }
            panic!("set_path_permission failed unexpectedly: {}", text);
        }

        let everyone_sid = sid_string_to_local_alloc_guard("S-1-1-0").unwrap();
        let rx_mask = FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0;
        for target in [&root, &child_dir, &child_file] {
            let (dacl, _sd_guard) = get_file_dacl(target).unwrap();
            assert!(
                has_allow_ace_with_mask(dacl, everyone_sid.as_sid_ptr(), rx_mask),
                "Everyone RX grant missing on '{}'",
                target.display()
            );
        }

        let _ = fs::remove_file(&child_file);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_portable_service_shmem_dir_acl_rejects_file_target() {
        let dir = unique_acl_test_path("dir_target_file");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("target.txt");
        fs::write(&file, b"x").unwrap();
        let result = set_path_permission_for_portable_service_shmem_dir(&file);
        assert!(result.is_err());
        let _ = fs::remove_file(&file);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_rejects_dir_target() {
        let dir = unique_acl_test_path("file_target_dir");
        fs::create_dir_all(&dir).unwrap();
        let result = set_path_permission_for_portable_service_shmem_file(&dir);
        assert!(result.is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_rejects_missing_target() {
        let path = unique_acl_test_path("missing").join("shared_memory_missing");
        let result = set_path_permission_for_portable_service_shmem_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_path_permission_rejects_reparse_entrypoint() {
        let root = unique_acl_test_path("reparse_entry");
        let real_dir = root.join("real");
        let link_dir = root.join("link");
        fs::create_dir_all(&real_dir).unwrap();
        if !try_create_dir_reparse_point(
            &real_dir,
            &link_dir,
            "test_set_path_permission_rejects_reparse_entrypoint",
        ) {
            let _ = fs::remove_dir_all(&real_dir);
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let result = set_path_permission(&link_dir, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0);
        let text = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            text.contains("reparse point"),
            "expected reparse-point rejection, got '{}'",
            text
        );

        let _ = fs::remove_dir(&link_dir);
        let _ = fs::remove_dir_all(&real_dir);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_portable_service_shmem_dir_acl_rejects_reparse_target() {
        let root = unique_acl_test_path("reparse_shmem_dir");
        let real_dir = root.join("real");
        let link_dir = root.join("link");
        fs::create_dir_all(&real_dir).unwrap();
        if !try_create_dir_reparse_point(
            &real_dir,
            &link_dir,
            "test_portable_service_shmem_dir_acl_rejects_reparse_target",
        ) {
            let _ = fs::remove_dir_all(&real_dir);
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let result = set_path_permission_for_portable_service_shmem_dir(&link_dir);
        let text = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            text.contains("reparse point"),
            "expected reparse-point rejection, got '{}'",
            text
        );

        let _ = fs::remove_dir(&link_dir);
        let _ = fs::remove_dir_all(&real_dir);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn test_portable_service_shmem_file_acl_rejects_reparse_target() {
        let root = unique_acl_test_path("reparse_shmem_file");
        let real_file = root.join("real.txt");
        let link_file = root.join("link.txt");
        fs::create_dir_all(&root).unwrap();
        fs::write(&real_file, b"x").unwrap();
        if !try_create_file_reparse_point(
            &real_file,
            &link_file,
            "test_portable_service_shmem_file_acl_rejects_reparse_target",
        ) {
            let _ = fs::remove_file(&real_file);
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let result = set_path_permission_for_portable_service_shmem_file(&link_file);
        let text = result.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            text.contains("reparse point"),
            "expected reparse-point rejection, got '{}'",
            text
        );

        let _ = fs::remove_file(&link_file);
        let _ = fs::remove_file(&real_file);
        let _ = fs::remove_dir_all(&root);
    }
}
