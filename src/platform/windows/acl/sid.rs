use super::*;

/// Returns the current process user SID as a standard SID string
/// (for example: `S-1-5-18`).
///
/// Source:
/// - Official SID-to-string API (`ConvertSidToStringSidW`):
///   https://learn.microsoft.com/en-us/windows/win32/api/sddl/nf-sddl-convertsidtostringsidw
pub(crate) fn current_process_user_sid_string() -> ResultType<String> {
    let mut token = HANDLE::default();
    let result = (|| -> ResultType<String> {
        unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(|e| anyhow!("Failed to open current process token: {}", e))?;
        }

        let buffer = unsafe { read_token_user_buffer(token, "current process")? };
        let token_user: TOKEN_USER =
            unsafe { std::ptr::read_unaligned(buffer.as_ptr() as *const TOKEN_USER) };
        if token_user.User.Sid.0.is_null() {
            bail!("Token SID is null");
        }

        let mut sid_string_ptr = PWSTR::null();
        unsafe {
            ConvertSidToStringSidW(token_user.User.Sid, &mut sid_string_ptr).map_err(|e| {
                anyhow!(
                    "ConvertSidToStringSidW failed for current process token SID: {}",
                    e
                )
            })?;
        }
        if sid_string_ptr.is_null() {
            bail!("ConvertSidToStringSidW returned null SID string pointer");
        }
        let _sid_string_guard = LocalAllocGuard(sid_string_ptr.0 as *mut std::ffi::c_void);
        unsafe {
            sid_string_ptr
                .to_string()
                .map_err(|e| anyhow!("Failed to decode SID string as UTF-16: {}", e))
        }
    })();

    if !token.is_invalid() {
        unsafe {
            let _ = CloseHandle(token);
        }
    }
    result
}

#[derive(Debug)]
pub(in crate::platform) struct LocalAllocGuard(pub(in crate::platform) *mut std::ffi::c_void);

impl LocalAllocGuard {
    #[inline]
    pub(super) fn as_sid_ptr(&self) -> *mut std::ffi::c_void {
        self.0
    }
}

impl Drop for LocalAllocGuard {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        // Buffers returned by ConvertStringSidToSidW / SetEntriesInAclW /
        // ConvertSidToStringSidW are LocalAlloc-owned and must be LocalFree'ed.
        unsafe {
            let _ = LocalFree(Some(HLOCAL(self.0)));
        }
    }
}

#[inline]
pub(in crate::platform) fn sid_string_to_local_alloc_guard(sid: &str) -> ResultType<LocalAllocGuard> {
    let sid_utf16 = wide_string(sid);
    let mut sid_ptr = PSID::default();
    unsafe {
        ConvertStringSidToSidW(PCWSTR::from_raw(sid_utf16.as_ptr()), &mut sid_ptr)
            .map_err(|e| anyhow!("ConvertStringSidToSidW failed for '{}': {}", sid, e))?;
    }
    if sid_ptr.0.is_null() {
        bail!("ConvertStringSidToSidW returned null SID for '{}'", sid);
    }
    Ok(LocalAllocGuard(sid_ptr.0))
}

#[inline]
pub(super) fn make_sid_trustee_entry(
    sid_ptr: *mut std::ffi::c_void,
    access_permissions: u32,
    inheritance: ACE_FLAGS,
    is_group: bool,
) -> EXPLICIT_ACCESS_W {
    // `is_group` is explicitly provided by the caller from the concrete SID semantic
    // (e.g. Administrators/Authenticated Users => group, LocalSystem/current user => user).
    EXPLICIT_ACCESS_W {
        grfAccessPermissions: access_permissions,
        grfAccessMode: SET_ACCESS,
        grfInheritance: inheritance,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: std::ptr::null_mut(),
            MultipleTrusteeOperation: Default::default(),
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: if is_group {
                TRUSTEE_IS_GROUP
            } else {
                TRUSTEE_IS_USER
            },
            // SAFETY: With TrusteeForm=TRUSTEE_IS_SID, ptstrName is interpreted as PSID.
            ptstrName: PWSTR::from_raw(sid_ptr as *mut u16),
        },
    }
}
