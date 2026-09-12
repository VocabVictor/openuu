use super::*;

pub enum StartPara {
    Direct,
    Logon(String, String),
}

// Launch flow summary:
// 1) Prepare/reset runtime shared memory + IPC token.
// 2) Start helper process (direct or logon) with shmem argument.
// 3) Keep STARTING=true until IPC ping/pong marks RUNNING, or timeout watchdog resets it.
pub(crate) fn start_portable_service(para: StartPara) -> ResultType<()> {
    log::info!("start portable service");
    let launch_token = {
        // Keep lock guards in explicit short scopes to make it obvious
        // there is no nested lock ordering (and to avoid Copilot false positives).
        let running = { *RUNNING.lock().unwrap() };
        let mut starting = STARTING.lock().unwrap();
        if *starting && !running && !has_running_portable_service_process() {
            log::warn!(
                "Detected stale portable service STARTING state without running process, reset it"
            );
            *starting = false;
        }
        if *starting || running {
            bail!("already running");
        }
        *starting = true;
        STARTING_TOKEN.fetch_add(1, Ordering::SeqCst) + 1
    };
    let start_result = (|| -> ResultType<()> {
        clear_runtime_shmem_state();
        let mut shmem_lock = SHMEM.lock().unwrap();
        let displays = scrap::Display::all()?;
        if displays.is_empty() {
            bail!("no display available!");
        }
        let mut max_pixel = 0;
        let align = 64;
        for d in displays {
            let resolutions = crate::platform::resolutions(&d.name());
            for r in resolutions {
                let pixel =
                    utils::align(r.width as _, align) * utils::align(r.height as _, align);
                if max_pixel < pixel {
                    max_pixel = pixel;
                }
            }
        }
        let shmem_size =
            utils::align(ADDR_CAPTURE_FRAME + max_pixel * 4, align).max(MIN_RUNTIME_SHMEM_LEN);
        let shmem_name = next_portable_service_shmem_name();
        if !is_valid_portable_service_shmem_name(&shmem_name) {
            bail!("Generated invalid portable service shared memory name");
        }
        let ipc_token = ipc::generate_one_time_ipc_token()?;
        // os error 112, no enough space
        *shmem_lock = Some(crate::portable_service::SharedMemory::create(
            &shmem_name,
            shmem_size,
        )?);
        *SHMEM_RUNTIME_NAME.lock().unwrap() = Some(shmem_name);
        shutdown_hooks::add_shutdown_hook(drop_portable_service_shared_memory);
        let shmem_name = SHMEM_RUNTIME_NAME
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| anyhow!("portable service shared memory name is unavailable"))?;
        let init_token_result = if let Some(shmem) = shmem_lock.as_mut() {
            unsafe {
                libc::memset(shmem.as_ptr() as _, 0, shmem.len() as _);
            }
            write_ipc_token_to_shmem(shmem, &ipc_token)
        } else {
            Ok(())
        };
        if let Err(e) = init_token_result {
            drop(shmem_lock);
            clear_runtime_shmem_state();
            bail!(
                "Failed to initialize portable service ipc token in shared memory: {}",
                e
            );
        };
        drop(shmem_lock);
        set_runtime_ipc_token(ipc_token.clone());
        let portable_service_arg = format!(
            "--portable-service {}",
            crate::portable_service::portable_service_shmem_arg(&shmem_name)
        );
        {
            let _sender = SENDER.lock().unwrap();
        }
        match para {
            StartPara::Direct => {
                match crate::platform::run_background(
                    &std::env::current_exe()?.to_string_lossy().to_string(),
                    &portable_service_arg,
                ) {
                    Ok(true) => {}
                    Ok(false) => {
                        clear_runtime_shmem_state();
                        bail!("Failed to run portable service process");
                    }
                    Err(e) => {
                        clear_runtime_shmem_state();
                        bail!("Failed to run portable service process: {}", e);
                    }
                }
            }
            StartPara::Logon(username, password) => {
                #[allow(unused_mut)]
                let mut exe = std::env::current_exe()?.to_string_lossy().to_string();
                #[cfg(feature = "flutter")]
                {
                    if let Some(dir) = Path::new(&exe).parent() {
                        if let Err(err) = set_path_permission(
                            Path::new(dir),
                            FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0,
                        ) {
                            clear_runtime_shmem_state();
                            bail!("Failed to set permission of {:?}: {}", dir, err);
                        }
                    }
                }
                #[cfg(not(feature = "flutter"))]
                if let Some((dir, dst)) =
                    crate::platform::windows::portable_service_logon_helper_paths()
                {
                    let cleanup_helper_artifacts = || {
                        if Path::new(&exe) != dst {
                            std::fs::remove_file(&dst).ok();
                        }
                        std::fs::remove_dir(&dir).ok();
                    };
                    let mut use_logon_helper_exe = false;
                    if let Err(err) = std::fs::create_dir_all(&dir) {
                        log::warn!(
                            "Failed to create portable service logon helper dir {:?}: {}",
                            dir,
                            err
                        );
                    } else if let Err(err) = std::fs::copy(&exe, &dst) {
                        log::warn!(
                            "Failed to copy portable service logon helper binary from '{}' to {:?}: {}",
                            exe,
                            dst,
                            err
                        );
                        cleanup_helper_artifacts();
                    } else if !dst.exists() {
                        log::warn!(
                            "Portable service logon helper binary missing after copy: {:?}",
                            dst
                        );
                        cleanup_helper_artifacts();
                    } else if let Err(err) =
                        set_path_permission(&dir, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0)
                    {
                        log::warn!(
                            "Failed to set portable service logon helper path permission for {:?}: {}",
                            dir,
                            err
                        );
                        cleanup_helper_artifacts();
                    } else {
                        use_logon_helper_exe = true;
                    }
                    if use_logon_helper_exe {
                        exe = dst.to_string_lossy().to_string();
                    }
                }
                if let Err(e) = crate::platform::windows::create_process_with_logon(
                    username.as_str(),
                    password.as_str(),
                    &exe,
                    &portable_service_arg,
                ) {
                    clear_runtime_shmem_state();
                    bail!("Failed to run portable service process: {}", e);
                }
            }
        }
        schedule_starting_timeout_reset(launch_token);
        Ok(())
    })();
    if start_result.is_err() {
        *STARTING.lock().unwrap() = false;
    }
    start_result
}

pub extern "C" fn drop_portable_service_shared_memory() {
    // https://stackoverflow.com/questions/35980148/why-does-an-atexit-handler-panic-when-it-accesses-stdout
    // Please make sure there is no print in the call stack
    clear_runtime_shmem_state();
}

pub fn set_quick_support(v: bool) {
    *QUICK_SUPPORT.lock().unwrap() = v;
}
