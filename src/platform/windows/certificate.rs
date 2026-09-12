use super::*;

#[inline]
pub fn uninstall_cert() -> ResultType<()> {
    cert::uninstall_cert()
}

pub(super) mod cert {
    use hbb_common::ResultType;

    extern "C" {
        fn DeleteRustDeskTestCertsW();
    }
    pub fn uninstall_cert() -> ResultType<()> {
        unsafe {
            DeleteRustDeskTestCertsW();
        }
        Ok(())
    }
}
