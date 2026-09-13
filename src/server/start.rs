use super::*;

/// How often the children that are still running are looked at. There is no portable way
/// to be woken when one exits, so while any exist they are polled.
const REAP_INTERVAL: Duration = Duration::from_millis(100);
/// A backstop for the reaper's sleep, in case a child was added without a notification.
const REAP_IDLE: Duration = Duration::from_secs(60);

/// A child process the reaper can ask whether it is over.
pub(super) trait Reapable {
    fn finished(&mut self) -> bool;
}

impl Reapable for std::process::Child {
    fn finished(&mut self) -> bool {
        matches!(self.try_wait(), Ok(Some(_)))
    }
}

/// Drop the children that have exited, and say how long to wait before looking again.
/// `None` means there is nothing left to reap: the next child has to be spawned before
/// there is anything to do, and a machine with nobody connected to it never spawns one.
pub(super) fn reap<T: Reapable>(children: &mut Vec<T>) -> Option<Duration> {
    children.retain_mut(|child| !child.finished());
    (!children.is_empty()).then_some(REAP_INTERVAL)
}

/// Add a child for the reaper to look after. Spawning is what wakes it, so this is the
/// only way to add one.
pub fn add_child(child: std::process::Child) {
    CHILD_PROCESS.lock().unwrap().push(child);
    CHILD_SPAWNED.notify();
}

pub fn check_zombie() {
    std::thread::spawn(|| loop {
        let next = reap(&mut CHILD_PROCESS.lock().unwrap());
        match next {
            Some(interval) => std::thread::sleep(interval),
            None => CHILD_SPAWNED.wait_for_change(REAP_IDLE),
        }
    });
}

/// Start the host server that allows the remote peer to control the current machine.
///
/// # Arguments
///
/// * `is_server` - Whether the current client is definitely the server.
/// If true, the server will be started.
/// Otherwise, client will check if there's already a server and start one if not.
#[cfg(any(target_os = "android", target_os = "ios"))]
#[tokio::main]
pub async fn start_server(_is_server: bool) {
    crate::RendezvousMediator::start_all().await;
}

/// Start the host server that allows the remote peer to control the current machine.
///
/// # Arguments
///
/// * `is_server` - Whether the current client is definitely the server.
/// If true, the server will be started.
/// Otherwise, client will check if there's already a server and start one if not.
/// * `no_server` - If `is_server` is false, whether to start a server if not found.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main]
pub async fn start_server(is_server: bool, no_server: bool) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        #[cfg(target_os = "linux")]
        {
            log::info!("DISPLAY={:?}", std::env::var("DISPLAY"));
            log::info!("XAUTHORITY={:?}", std::env::var("XAUTHORITY"));
        }
        #[cfg(windows)]
        base::platform::windows::start_cpu_performance_monitor();
    });

    if is_server {
        crate::common::set_server_running(true);
        std::thread::spawn(move || {
            if let Err(err) = crate::ipc::start("") {
                log::error!("Failed to start ipc: {}", err);
                if crate::is_server() {
                    log::error!("ipc is occupied by another process, try kill it");
                    std::thread::spawn(stop_main_window_process).join().ok();
                }
                std::process::exit(-1);
            }
        });
        // Warm the DRM availability cache before any client connects, so the first connection does
        // not race a cold `_drm` probe and ship an empty display list ("No displays" + retry).
        // X11 is skipped -- probing there makes the root service open DRM readers for a path this
        // session can never take -- but that decision belongs to `warm_availability`, which already
        // makes it, and NOT to this call site. Deciding it here is the same one-shot-at-startup
        // mistake the pre-warm had: `is_x11()` answers "x11" whenever loginctl cannot yet name the
        // seat0 session, which during a boot is exactly when this runs, and nothing revisits it --
        // so a Wayland host that came up slowly skipped the warm for the life of the process and
        // got back the cold-probe "No displays" symptom the warm exists to remove.
        #[cfg(all(target_os = "linux", feature = "drm"))]
        if let Err(err) = std::thread::Builder::new()
            .name("drm-warm".into())
            .spawn(drm_capturer::warm_availability)
        {
            // Same reason as the root service's startup threads: `thread::spawn` panics on EAGAIN
            // and that would abort `start_server`. Skipping the warm costs the first session the
            // cold probe, which is what happened before the warm existed.
            log::warn!("drm: could not spawn the availability warm ({err}); skipping it");
        }
        input_service::fix_key_down_timeout_loop();
        #[cfg(target_os = "linux")]
        if input_service::wayland_use_uinput() {
            allow_err!(input_service::setup_uinput(0, 1920, 0, 1080).await);
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        wait_initial_config_sync().await;
        #[cfg(target_os = "windows")]
        crate::platform::try_kill_broker();
        #[cfg(feature = "hwcodec")]
        scrap::hwcodec::start_check_process();
        crate::RendezvousMediator::start_all().await;
    } else {
        match crate::ipc::connect(1000, "").await {
            Ok(mut conn) => {
                if conn.send(&Data::SyncConfig(None)).await.is_ok() {
                    if let Ok(Some(data)) = conn.next_timeout(1000).await {
                        match data {
                            Data::SyncConfig(Some(configs)) => {
                                let (config, config2) = *configs;
                                if Config::set(config) {
                                    log::info!("config synced");
                                }
                                if Config2::set(config2) {
                                    log::info!("config2 synced");
                                }
                            }
                            _ => {}
                        }
                    }
                }
                #[cfg(feature = "hwcodec")]
                #[cfg(any(target_os = "windows", target_os = "linux"))]
                crate::ipc::client_get_hwcodec_config_thread(0);
            }
            Err(err) => {
                log::info!("server not started: {err:?}, no_server: {no_server}");
                if no_server {
                    hbb_common::sleep(1.0).await;
                    std::thread::spawn(|| start_server(false, true));
                } else {
                    log::info!("try start server");
                    std::thread::spawn(|| start_server(true, false));
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[tokio::main(flavor = "current_thread")]
pub async fn start_ipc_url_server() {
    log::debug!("Start an ipc server for listening to url schemes");
    match crate::ipc::new_listener("_url").await {
        Ok(mut incoming) => {
            while let Some(Ok(conn)) = incoming.next().await {
                let mut conn = crate::ipc::Connection::new(conn);
                match conn.next_timeout(1000).await {
                    Ok(Some(data)) => match data {
                        #[cfg(feature = "flutter")]
                        Data::UrlLink(url) => {
                            let mut m = HashMap::new();
                            m.insert("name", "on_url_scheme_received");
                            m.insert("url", url.as_str());
                            let event = serde_json::to_string(&m).unwrap_or("".to_owned());
                            match crate::flutter::push_global_event(
                                crate::flutter::APP_TYPE_MAIN,
                                event,
                            ) {
                                None => log::warn!("No main window app found!"),
                                Some(..) => {}
                            }
                        }
                        _ => {
                            log::warn!("An unexpected data was sent to the ipc url server.")
                        }
                    },
                    Err(err) => {
                        log::error!("{}", err);
                    }
                    _ => {}
                }
            }
        }
        Err(err) => {
            log::error!("{}", err);
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn stop_main_window_process() {
    // this may also kill another --server process,
    // but --server usually can be auto restarted by --service, so it is ok
    if let Ok(mut conn) = crate::ipc::connect(1000, "").await {
        conn.send(&crate::ipc::Data::Close).await.ok();
    }
    #[cfg(windows)]
    {
        // in case above failure, e.g. zombie process
        if let Err(e) = crate::platform::try_kill_rustdesk_main_window_process() {
            log::error!("kill failed: {}", e);
        }
    }
}
