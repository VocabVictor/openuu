use super::*;

/// Hardens ACLs for portable-service shared-memory path (directory or file).
///
/// Why:
/// - Shared memory used by portable service carries runtime control/data and must not inherit
///   broad/default ACLs.
/// - We explicitly grant only trusted principals and remove broad groups to reduce local
///   privilege-boundary bypass risk.
///
/// ACL policy applied via Win32 ACL APIs (`SetEntriesInAclW` + `SetNamedSecurityInfoW`):
/// - common (directory + file):
///   - `S-1-5-18` (LocalSystem): full control
///   - `S-1-5-32-544` (Built-in Administrators): full control
///   - `current_process_user_sid_string()` result: full control
/// - directory (`portable_service_shmem` parent):
///   - keep `Authenticated Users` directory-level write so other local accounts can
///     create their own runtime shmem files after account switching
///   - `FILE_GENERIC_WRITE + NO_INHERITANCE` means write/create on this directory itself;
///     it is intentionally not inherited by children.
///     Reference:
///     - File access rights:
///       https://learn.microsoft.com/en-us/windows/win32/fileio/file-access-rights-constants
///     - ACE inheritance rules:
///       https://learn.microsoft.com/en-us/windows/win32/secauthz/ace-inheritance-rules
///   - remove `Everyone` and `Users` grants
/// - file (`shared_memory*` flink):
///   - remove broad grants:
///     - `S-1-1-0` (Everyone)
///     - `S-1-5-11` (Authenticated Users)
///     - `S-1-5-32-545` (Users)
///
/// https://learn.microsoft.com/en-us/windows/win32/secauthz/well-known-sids
pub fn set_path_permission_for_portable_service_shmem_dir(path: &Path) -> ResultType<()> {
    set_path_permission_for_portable_service_shmem_impl(path, true)
}

#[inline]
pub fn validate_path_for_portable_service_shmem_dir(path: &Path) -> ResultType<()> {
    validate_portable_service_shmem_dir_target(path)
}

#[inline]
pub fn set_path_permission_for_portable_service_shmem_file(path: &Path) -> ResultType<()> {
    set_path_permission_for_portable_service_shmem_impl(path, false)
}

pub(super) fn validate_portable_service_shmem_dir_target(path: &Path) -> ResultType<()> {
    let metadata = fs::symlink_metadata(path).map_err(|e| {
        anyhow!(
            "Failed to inspect portable service shared-memory ACL directory '{}': {}",
            path.display(),
            e
        )
    })?;
    if is_reparse_point(&metadata) {
        bail!(
            "Portable service shared-memory ACL directory target is a reparse point and is rejected: '{}'",
            path.display()
        );
    }
    if !metadata.file_type().is_dir() {
        bail!(
            "Portable service shared-memory ACL target is not a directory: '{}'",
            path.display()
        );
    }
    Ok(())
}

pub(super) fn set_path_permission_for_portable_service_shmem_impl(
    path: &Path,
    expect_dir: bool,
) -> ResultType<()> {
    if expect_dir {
        validate_portable_service_shmem_dir_target(path)?;
    } else {
        let metadata_result = fs::symlink_metadata(path);
        match metadata_result {
            Ok(metadata) => {
                if metadata.file_type().is_dir() {
                    bail!(
                        "Portable service shared-memory ACL target is a directory, expected file-like path: '{}'",
                        path.display()
                    );
                }
                if is_reparse_point(&metadata) {
                    bail!(
                        "Portable service shared-memory ACL file target is a reparse point and is rejected: '{}'",
                        path.display()
                    );
                }
            }
            Err(e)
                if e.kind() == io::ErrorKind::NotFound
                    || e.kind() == io::ErrorKind::PermissionDenied =>
            {
                // Keep going and let Win32 ACL APIs return the final OS error.
                // `Path::exists()/is_file()` and metadata can collapse ACL-denied paths into
                // a false "not found" signal under restricted directory ACLs.
            }
            Err(e) => {
                bail!(
                    "Failed to inspect portable service shared-memory ACL target '{}': {}",
                    path.display(),
                    e
                );
            }
        }
    }

    let user_sid = current_process_user_sid_string()?;
    let local_system_sid = sid_string_to_local_alloc_guard("S-1-5-18")?;
    let administrators_sid = sid_string_to_local_alloc_guard("S-1-5-32-544")?;
    let current_user_sid = sid_string_to_local_alloc_guard(&user_sid)?;
    let authenticated_users_sid = if expect_dir {
        Some(sid_string_to_local_alloc_guard("S-1-5-11")?)
    } else {
        None
    };

    let inherit_flags = if expect_dir {
        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
    } else {
        NO_INHERITANCE
    };
    let mut entries = vec![
        make_sid_trustee_entry(
            local_system_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            false,
        ),
        make_sid_trustee_entry(
            administrators_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            true,
        ),
        make_sid_trustee_entry(
            current_user_sid.as_sid_ptr(),
            FILE_ALL_ACCESS.0,
            inherit_flags,
            false,
        ),
    ];
    if let Some(auth_sid) = authenticated_users_sid.as_ref() {
        // Keep the shared parent directory multi-user writable at directory level.
        entries.push(make_sid_trustee_entry(
            auth_sid.as_sid_ptr(),
            FILE_GENERIC_WRITE.0,
            NO_INHERITANCE,
            true,
        ));
    }

    // Rebuild mode: build a fresh DACL (old ACL not merged) and apply as protected.
    // This avoids carrying over broad legacy ACEs from inherited/default ACLs.
    // Reference:
    // - SetEntriesInAclW:
    //   https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setentriesinaclw
    // - SetNamedSecurityInfoW (PROTECTED_DACL_SECURITY_INFORMATION):
    //   https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-setnamedsecurityinfow
    let mut new_acl: *mut ACL = std::ptr::null_mut();
    let set_entries_result =
        unsafe { SetEntriesInAclW(Some(entries.as_slice()), None, &mut new_acl) };
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

    let path_utf16: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let security_info = DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION;
    let set_named_result = unsafe {
        SetNamedSecurityInfoW(
            PCWSTR::from_raw(path_utf16.as_ptr()),
            SE_FILE_OBJECT,
            security_info,
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
