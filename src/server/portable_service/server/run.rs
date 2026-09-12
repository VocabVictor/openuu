use super::*;

pub fn run_portable_service() {
    let shmem_name = match portable_service_shmem_name_from_args() {
        Some(name) => name,
        None => {
            if has_portable_service_shmem_arg() {
                log::error!(
                    "Invalid portable service shared memory argument, aborting startup"
                );
            } else {
                log::error!(
                    "Missing portable service shared memory argument, aborting startup"
                );
            }
            return;
        }
    };
    let shmem = match SharedMemory::open_existing(&shmem_name) {
        Ok(shmem) => Arc::new(shmem),
        Err(e) => {
            log::error!("Failed to open existing shared memory: {:?}", e);
            return;
        }
    };
    if let Err(e) = validate_runtime_shmem_layout(shmem.as_ref()) {
        log::error!("{}", e);
        return;
    }
    let ipc_token = match read_ipc_token_from_shmem(shmem.as_ref()) {
        Some(token) => token,
        None => {
            log::error!(
                "Missing portable service ipc token in shared memory, aborting startup"
            );
            return;
        }
    };
    let shmem1 = shmem.clone();
    let shmem2 = shmem.clone();
    let mut threads = vec![];
    threads.push(std::thread::spawn(|| {
        run_get_cursor_info(shmem1);
    }));
    threads.push(std::thread::spawn(|| {
        run_capture(shmem2);
    }));
    threads.push(std::thread::spawn(move || {
        run_ipc_client(ipc_token);
    }));
    // Detached shutdown watchdog:
    // - gives graceful shutdown/cleanup a short window
    // - force-exits the process if workers are still stuck
    std::thread::spawn(|| {
        run_exit_check();
    });
    let record_pos_handle = crate::input_service::try_start_record_cursor_pos();
    // Arm forced-exit watchdog only for worker join phase.
    // Once join phase completes, cleanup should not be interrupted by forced exit.
    FORCE_EXIT_ARMED.store(true, Ordering::SeqCst);
    for th in threads.drain(..) {
        th.join().ok();
        log::info!("thread joined");
    }
    FORCE_EXIT_ARMED.store(false, Ordering::SeqCst);

    crate::input_service::try_stop_record_cursor_pos();
    if let Some(handle) = record_pos_handle {
        match handle.join() {
            Ok(_) => log::info!("record_pos_handle joined"),
            Err(e) => log::error!("record_pos_handle join error {:?}", &e),
        }
    }
    drop(shmem);
    remove_shared_memory_flink_with_retry(&shmem_name);
}

pub(super) fn run_exit_check() {
    pub(super) const FORCED_EXIT_DELAY: Duration = Duration::from_secs(3);
    loop {
        if EXIT.lock().unwrap().clone() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // Fallback only: normal shutdown path should complete and process should exit naturally.
    // This forced exit is a last resort when worker threads are stuck and graceful teardown
    // does not finish in time.
    std::thread::sleep(FORCED_EXIT_DELAY);
    if FORCE_EXIT_ARMED.load(Ordering::SeqCst) {
        log::warn!(
            "Portable service shutdown watchdog fallback triggered: forcing process exit after {:?}",
            FORCED_EXIT_DELAY
        );
        std::process::exit(0);
    }
}

pub(super) fn remove_shared_memory_flink_with_retry(name: &str) {
    pub(super) const MAX_RETRY: usize = 20;
    pub(super) const RETRY_INTERVAL: Duration = Duration::from_millis(200);
    for attempt in 0..MAX_RETRY {
        let is_last_attempt = attempt + 1 == MAX_RETRY;
        if remove_shared_memory_flink_once(name, is_last_attempt, "SYSTEM cleanup") {
            return;
        }
        if !is_last_attempt {
            std::thread::sleep(RETRY_INTERVAL);
        }
    }
    log::warn!(
        "SYSTEM cleanup failed to remove portable service shared-memory flink artifact '{}' after retry",
        name
    );
}

pub(super) fn run_get_cursor_info(shmem: Arc<SharedMemory>) {
    loop {
        if EXIT.lock().unwrap().clone() {
            break;
        }
        unsafe {
            let para = shmem.as_ptr().add(ADDR_CURSOR_PARA) as *mut CURSORINFO;
            (*para).cbSize = size_of::<CURSORINFO>() as _;
            let result = winuser::GetCursorInfo(para);
            if result == TRUE {
                utils::increase_counter(shmem.as_ptr().add(ADDR_CURSOR_COUNTER));
            }
        }
        // more frequent in case of `Error of mouse_cursor service`
        std::thread::sleep(Duration::from_millis(15));
    }
}
