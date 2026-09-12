use super::*;

/// Get the SID of the user from a token.
/// Returns a Vec<u8> containing the SID bytes.
pub fn get_user_sid_from_token(user_token: UserToken) -> Result<Vec<u8>> {
    let token_handle = HANDLE(user_token.as_raw() as _);

    // First call to get required buffer size
    let mut return_length = 0u32;
    let _ = unsafe { GetTokenInformation(token_handle, TokenUser, None, 0, &mut return_length) };

    if return_length == 0 {
        return Err(anyhow!(
            "Failed to get token information size: {}",
            std::io::Error::last_os_error()
        ));
    }

    // Allocate buffer and get token information
    let mut buffer = vec![0u8; return_length as usize];
    unsafe {
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(buffer.as_mut_ptr() as *mut c_void),
            return_length,
            &mut return_length,
        )
        .map_err(|e| anyhow!("Failed to get token information: {}", e))?;
    }

    // Extract SID from TOKEN_USER structure
    let token_user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
    let sid_ptr = token_user.User.Sid;

    // Get SID length and copy to owned buffer
    let sid_length = unsafe { GetLengthSid(sid_ptr) };

    if sid_length == 0 {
        return Err(anyhow!("Invalid SID length"));
    }

    let mut sid_buffer = vec![0u8; sid_length as usize];
    unsafe {
        ptr::copy_nonoverlapping(
            sid_ptr.0 as *const u8,
            sid_buffer.as_mut_ptr(),
            sid_length as usize,
        );
    }

    Ok(sid_buffer)
}

/// Create a restricted DACL that only allows SYSTEM and a specific user.
/// Returns a pointer to the ACL that must be freed with LocalFree.
///
/// # Safety
///
/// This function is safe to call, but contains internal unsafe code that relies on
/// pointer lifetime guarantees:
///
/// - The `user_sid` slice must contain valid SID binary data.
/// - Internally, raw pointers to `system_sid_buffer` (stack-allocated) and `user_sid`
///   are stored in `TRUSTEE_W.ptstrName` fields. These pointers are only used during
///   the `SetEntriesInAclW` call, which occurs before either buffer goes out of scope.
/// - The returned ACL pointer is allocated by Windows and must be freed with `LocalFree`.
pub fn create_restricted_dacl(user_sid: &[u8]) -> Result<*mut c_void> {
    // Create SYSTEM SID (well-known SID: S-1-5-18)
    // SAFETY: This buffer must outlive the TRUSTEE_W structures that reference it
    let mut system_sid_buffer = vec![0u8; 64]; // Max SID size
    let mut system_sid_size = system_sid_buffer.len() as u32;
    unsafe {
        CreateWellKnownSid(
            WinLocalSystemSid,
            None, // No domain SID
            Some(PSID(system_sid_buffer.as_mut_ptr() as *mut c_void)),
            &mut system_sid_size,
        )
        .map_err(|e| anyhow!("Failed to create SYSTEM SID: {}", e))?;
    }

    // Build EXPLICIT_ACCESS entries for SYSTEM and user
    // SAFETY: The ptstrName pointers below reference system_sid_buffer and user_sid.
    // These buffers must remain valid until SetEntriesInAclW returns.
    let mut explicit_access: [EXPLICIT_ACCESS_W; 2] = unsafe { std::mem::zeroed() };

    // Entry 0: SYSTEM - full access
    explicit_access[0].grfAccessPermissions = FILE_ALL_ACCESS.0;
    explicit_access[0].grfAccessMode = SET_ACCESS;
    explicit_access[0].grfInheritance = ACE_FLAGS(0); // No inheritance for pipes
    explicit_access[0].Trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: Default::default(),
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        ptstrName: PWSTR::from_raw(system_sid_buffer.as_ptr() as *mut u16),
    };

    // Entry 1: User - full access
    explicit_access[1].grfAccessPermissions = FILE_ALL_ACCESS.0;
    explicit_access[1].grfAccessMode = SET_ACCESS;
    explicit_access[1].grfInheritance = ACE_FLAGS(0); // No inheritance for pipes
                                                      // SAFETY: When TrusteeForm is TRUSTEE_IS_SID, ptstrName is interpreted as a PSID
                                                      // pointer, not a string pointer. The Windows API reuses this field for different
                                                      // purposes based on TrusteeForm. The SID binary data in user_sid is valid for
                                                      // the duration of this function call (until SetEntriesInAclW returns).
    explicit_access[1].Trustee = TRUSTEE_W {
        pMultipleTrustee: ptr::null_mut(),
        MultipleTrusteeOperation: Default::default(),
        TrusteeForm: TRUSTEE_IS_SID,
        TrusteeType: TRUSTEE_IS_USER,
        ptstrName: PWSTR::from_raw(user_sid.as_ptr() as *mut u16),
    };

    // Create ACL from explicit access entries
    // After this call returns, system_sid_buffer and user_sid are no longer needed
    let mut new_acl: *mut ACL = ptr::null_mut();
    let result = unsafe {
        SetEntriesInAclW(
            Some(&explicit_access),
            None, // No existing ACL
            &mut new_acl,
        )
    };

    if result.0 != 0 {
        return Err(anyhow!(
            "SetEntriesInAclW failed with error code: {}",
            result.0
        ));
    }

    if new_acl.is_null() {
        return Err(anyhow!("SetEntriesInAclW returned null ACL"));
    }

    Ok(new_acl as *mut c_void)
}
