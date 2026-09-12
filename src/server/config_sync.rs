#[cfg(any(target_os = "macos", target_os = "linux"))]
use super::*;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) async fn wait_initial_config_sync() {
    if crate::platform::is_root() {
        return;
    }

    // Non-server process should not block startup, but still keeps background sync/watch alive.
    if !crate::is_server() {
        tokio::spawn(async move {
            sync_and_watch_config_dir(None).await;
        });
        return;
    }

    let (sync_done_tx, mut sync_done_rx) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(async move {
        sync_and_watch_config_dir(Some(sync_done_tx)).await;
    });

    // Server process waits up to N seconds for initial root->local sync to reduce stale-start window.
    tokio::select! {
        _ = &mut sync_done_rx => {
        }
        _ = tokio::time::sleep(Duration::from_secs(CONFIG_SYNC_INITIAL_WAIT_SECS)) => {
            log::warn!(
                "timed out waiting {}s for initial config sync, continue startup and keep syncing in background",
                CONFIG_SYNC_INITIAL_WAIT_SECS
            );
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub(super) async fn sync_and_watch_config_dir(sync_done_tx: Option<tokio::sync::oneshot::Sender<()>>) {
    let mut cfg0 = (Config::get(), Config2::get());
    let mut synced = false;
    let mut is_root_config_empty = false;
    let mut sync_done_tx = sync_done_tx;
    let tries = if crate::is_server() { 30 } else { 3 };
    log::debug!("#tries of ipc service connection: {}", tries);
    use hbb_common::sleep;
    for i in 1..=tries {
        sleep(i as f32 * CONFIG_SYNC_INTERVAL_SECS).await;
        match crate::ipc::connect_service(1000).await {
            Ok(mut conn) => {
                if !synced {
                    if conn.send(&Data::SyncConfig(None)).await.is_ok() {
                        if let Ok(Some(data)) = conn.next_timeout(1000).await {
                            match data {
                                Data::SyncConfig(Some(configs)) => {
                                    let (config, config2) = *configs;
                                    let _chk = crate::ipc::CheckIfRestart::new();
                                    #[cfg(target_os = "macos")]
                                    let _chk_pk = crate::CheckIfResendPk::new();
                                    if !config.is_empty() {
                                        if cfg0.0 != config {
                                            cfg0.0 = config.clone();
                                            Config::set(config);
                                            log::info!("sync config from root");
                                        }
                                        if cfg0.1 != config2 {
                                            cfg0.1 = config2.clone();
                                            Config2::set(config2);
                                            log::info!("sync config2 from root");
                                        }
                                    } else {
                                        // only on macos, because this issue was only reproduced on macos
                                        #[cfg(target_os = "macos")]
                                        {
                                            // root config is empty, mark for sync in watch loop
                                            // to prevent root from generating a new config on login screen
                                            is_root_config_empty = true;
                                        }
                                    }
                                    synced = true;
                                    // Notify startup waiter once initial sync phase finishes successfully.
                                    if let Some(tx) = sync_done_tx.take() {
                                        let _ = tx.send(());
                                    }
                                }
                                _ => {}
                            };
                        };
                    }
                    if !synced {
                        log::warn!(
                            "initial config sync from root failed, reconnecting to ipc_service"
                        );
                        continue;
                    }
                }

                loop {
                    sleep(CONFIG_SYNC_INTERVAL_SECS).await;
                    let cfg = (Config::get(), Config2::get());
                    let should_sync = cfg != cfg0 || (is_root_config_empty && !cfg.0.is_empty());
                    if should_sync {
                        if is_root_config_empty {
                            log::info!("root config is empty, sync our config to root");
                        } else {
                            log::info!("config updated, sync to root");
                        }
                        match conn.send(&Data::SyncConfig(Some(cfg.clone().into()))).await {
                            Err(e) => {
                                log::error!("sync config to root failed: {}", e);
                                match crate::ipc::connect_service(1000).await {
                                    Ok(mut _conn) => {
                                        conn = _conn;
                                        log::info!("reconnected to ipc_service");
                                    }
                                    _ => {}
                                }
                            }
                            _ => {
                                cfg0 = cfg;
                                conn.next_timeout(1000).await.ok();
                                is_root_config_empty = false;
                            }
                        }
                    }
                }
            }
            Err(_) => {
                log::info!("#{} try: failed to connect to ipc_service", i);
            }
        }
    }
    // Notify startup waiter even when initial sync is skipped/failed, to avoid unnecessary waiting.
    if let Some(tx) = sync_done_tx.take() {
        let _ = tx.send(());
    }
    log::warn!("skipped config sync");
}
