use super::*;

pub struct SharedMemory {
    pub(super) inner: Shmem,
}

unsafe impl Send for SharedMemory {}
unsafe impl Sync for SharedMemory {}

impl Deref for SharedMemory {
    type Target = Shmem;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for SharedMemory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl SharedMemory {
    pub fn create(name: &str, size: usize) -> ResultType<Self> {
        let flink = Self::flink(name.to_string())?;
        let shmem = match ShmemConf::new()
            .size(size)
            .flink(&flink)
            .force_create_flink()
            .create()
        {
            Ok(m) => m,
            Err(ShmemError::LinkExists) => {
                bail!(
                    "Unable to force create shmem flink {}, which should not happen.",
                    flink
                )
            }
            Err(e) => {
                bail!("Unable to create shmem flink {} : {}", flink, e);
            }
        };
        log::info!("Create shared memory, size: {}, flink: {}", size, flink);
        if let Err(err) = set_path_permission_for_portable_service_shmem_file(Path::new(&flink)) {
            // Release shmem handle first so best-effort flink cleanup has a chance to succeed.
            drop(shmem);
            match std::fs::remove_file(&flink) {
                Ok(()) => {
                    log::info!(
                        "Create cleanup removed portable service shared-memory flink artifact: {}",
                        flink
                    );
                }
                Err(remove_err) if remove_err.kind() == std::io::ErrorKind::NotFound => {}
                Err(remove_err) => {
                    log::warn!(
                        "Create cleanup failed to remove portable service shared-memory flink artifact {}: {}",
                        flink,
                        remove_err
                    );
                }
            }
            return Err(err);
        }
        Ok(SharedMemory { inner: shmem })
    }

    pub fn open_existing(name: &str) -> ResultType<Self> {
        let flink = Self::flink(name.to_string())?;
        let shmem = match ShmemConf::new().flink(&flink).allow_raw(true).open() {
            Ok(m) => m,
            Err(e) => {
                bail!("Unable to open existing shmem flink {} : {}", flink, e);
            }
        };
        log::info!("open existing shared memory, flink: {:?}", flink);
        Ok(SharedMemory { inner: shmem })
    }

    pub fn write(&self, addr: usize, data: &[u8]) {
        unsafe {
            debug_assert!(addr + data.len() <= self.inner.len());
            let ptr = self.inner.as_ptr().add(addr);
            let shared_mem_slice = slice::from_raw_parts_mut(ptr, data.len());
            shared_mem_slice.copy_from_slice(data);
        }
    }

    pub(super) fn flink(name: String) -> ResultType<String> {
        let mut dir = crate::platform::user_accessible_folder()?;
        dir = dir.join(hbb_common::config::APP_NAME.read().unwrap().clone());
        dir = dir.join(SHMEM_PARENT_DIR);
        let parent_created = !dir.exists();
        if parent_created {
            std::fs::create_dir_all(&dir)?;
        }
        if parent_created || crate::platform::is_root() {
            // Harden parent ACL on first provisioning and periodically on SYSTEM path.
            set_path_permission_for_portable_service_shmem_dir(&dir)?;
        } else {
            // Existing parents still need type/reparse validation. Non-SYSTEM callers may lack
            // WRITE_DAC on a valid parent, so avoid rebuilding the ACL here.
            validate_path_for_portable_service_shmem_dir(&dir)?;
        }
        Ok(dir
            .join(format!("shared_memory{}", name))
            .to_string_lossy()
            .to_string())
    }
}
