use super::*;

pub(super) fn has_running_portable_service_process() -> bool {
    let app_exe = format!("{}.exe", crate::get_app_name().to_lowercase());
    !crate::platform::get_pids_of_process_with_first_arg(&app_exe, "--portable-service")
        .is_empty()
}

#[inline]
pub(super) fn next_portable_service_shmem_name() -> String {
    format!(
        "{}_{}_{:08x}",
        crate::portable_service::SHMEM_NAME,
        std::process::id(),
        hbb_common::rand::random::<u32>()
    )
}

#[inline]
pub(super) fn set_runtime_ipc_token(token: String) {
    *IPC_RUNTIME_TOKEN.lock().unwrap() = Some(token);
}

#[inline]
pub(super) fn schedule_remove_runtime_shmem_flink_retry(name: String) {
    std::thread::spawn(move || {
        const MAX_RETRY: usize = 20;
        const RETRY_INTERVAL: Duration = Duration::from_millis(200);
        for _ in 0..MAX_RETRY {
            std::thread::sleep(RETRY_INTERVAL);
            if remove_shared_memory_flink_once(&name, false, "Client cleanup") {
                return;
            }
        }
        log::warn!(
            "Failed to remove portable service shared-memory flink artifact '{}' after retry",
            name
        );
    });
}

#[inline]
pub(super) fn clear_runtime_shmem_state() {
    let mut runtime_token = IPC_RUNTIME_TOKEN.lock().unwrap();
    let mut shmem_lock = SHMEM.lock().unwrap();
    if let Some(shmem) = shmem_lock.as_mut() {
        clear_ipc_token_in_shmem(shmem);
    }
    *shmem_lock = None;
    let runtime_name = SHMEM_RUNTIME_NAME.lock().unwrap().take();
    *runtime_token = None;
    drop(runtime_token);
    drop(shmem_lock);
    if let Some(name) = runtime_name.as_deref() {
        if !remove_shared_memory_flink_once(name, true, "Client cleanup") {
            schedule_remove_runtime_shmem_flink_retry(name.to_owned());
        }
    }
}

#[inline]
pub(super) fn consume_runtime_ipc_token_if_match(candidate: &str) -> (bool, Option<String>) {
    let mut token = IPC_RUNTIME_TOKEN.lock().unwrap();
    if !token
        .as_deref()
        .is_some_and(|expected| ipc::constant_time_ipc_token_eq(expected, candidate))
    {
        return (false, None);
    }
    let mut shmem_lock = SHMEM.lock().unwrap();
    let matched_shmem_name = SHMEM_RUNTIME_NAME.lock().unwrap().clone();
    *token = None;
    if let Some(shmem) = shmem_lock.as_mut() {
        clear_ipc_token_in_shmem(shmem);
    }
    (true, matched_shmem_name)
}

#[inline]
pub(super) fn restore_runtime_ipc_token_after_failed_handshake(
    token: &str,
    expected_shmem_name: Option<&str>,
) {
    let mut runtime_token = IPC_RUNTIME_TOKEN.lock().unwrap();
    if let Some(current) = runtime_token.as_deref() {
        if current != token {
            log::debug!(
                "Skip restoring portable service ipc token after handshake failure: runtime token has changed to a newer value"
            );
            return;
        }
    }
    let mut shmem_lock = SHMEM.lock().unwrap();
    let current_shmem_name = SHMEM_RUNTIME_NAME.lock().unwrap().clone();
    if current_shmem_name.as_deref() != expected_shmem_name {
        if runtime_token.as_deref() == Some(token) {
            *runtime_token = None;
        }
        log::debug!(
            "Skip restoring portable service ipc token after handshake failure: shared-memory instance has changed"
        );
        return;
    }
    let shmem_write_error = if let Some(shmem) = shmem_lock.as_mut() {
        write_ipc_token_to_shmem(shmem, token)
            .err()
            .map(|err| err.to_string())
    } else {
        Some("shared memory unavailable".to_owned())
    };
    if let Some(err) = shmem_write_error {
        if runtime_token.as_deref() == Some(token) {
            *runtime_token = None;
        }
        log::warn!(
            "Failed to restore portable service ipc token after handshake failure: {}",
            err
        );
        return;
    }
    *runtime_token = Some(token.to_owned());
}

#[inline]
pub(super) fn schedule_starting_timeout_reset(launch_token: u64) {
    std::thread::spawn(move || {
        std::thread::sleep(PORTABLE_SERVICE_STARTUP_TIMEOUT);
        let should_reset = {
            // Guard against stale watchdogs from previous launches:
            // only the watchdog that matches the latest STARTING_TOKEN may reset STARTING.
            let current_token = STARTING_TOKEN.load(Ordering::SeqCst);
            // Keep lock guards in explicit short scopes to make it obvious
            // there is no nested lock ordering (and to avoid Copilot false positives).
            let starting = { *STARTING.lock().unwrap() };
            let running = { *RUNNING.lock().unwrap() };
            current_token == launch_token && starting && !running
        };
        if should_reset {
            log::warn!(
                "Portable service startup timeout before IPC ready, reset STARTING state"
            );
            *STARTING.lock().unwrap() = false;
        }
    });
}
