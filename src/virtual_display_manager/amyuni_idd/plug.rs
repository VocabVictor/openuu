use super::*;

#[inline]
fn plug_monitor_(
    add: bool,
    wait_timeout: Option<Duration>,
) -> Result<(), win_device::DeviceError> {
    let cmd = if add { 0x10 } else { 0x00 };
    let cmd = [cmd, 0x00, 0x00, 0x00];
    let now = Instant::now();
    let c1 = get_monitor_count();
    unsafe {
        win_device::device_io_control(&INTERFACE_GUID, PLUG_MONITOR_IO_CONTROL_CDOE, &cmd, 0)?;
    }
    if let Some(wait_timeout) = wait_timeout {
        while now.elapsed() < wait_timeout {
            if get_monitor_count() != c1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    // No need to consider concurrency here.
    if add {
        // If the monitor is plugged in, increase the count.
        // Though there's already a check of `VIRTUAL_DISPLAY_MAX_COUNT`, it's still better to check here for double ensure.
        if VIRTUAL_DISPLAY_COUNT.load(atomic::Ordering::SeqCst) < VIRTUAL_DISPLAY_MAX_COUNT {
            VIRTUAL_DISPLAY_COUNT.fetch_add(1, atomic::Ordering::SeqCst);
        }
    } else {
        if VIRTUAL_DISPLAY_COUNT.load(atomic::Ordering::SeqCst) > 0 {
            VIRTUAL_DISPLAY_COUNT.fetch_sub(1, atomic::Ordering::SeqCst);
        }
    }
    Ok(())
}

// `std::thread::sleep()` with a timeout is acceptable here.
// Because user can wait for a while to plug in a monitor.
fn plug_in_monitor_(
    add: bool,
    is_driver_async_installed: bool,
    wait_timeout: Option<Duration>,
) -> ResultType<()> {
    let timeout = Duration::from_secs(3);
    let now = Instant::now();
    let reg_connectivity_old = reg_display_settings::read_reg_connectivity();
    loop {
        match plug_monitor_(add, wait_timeout) {
            Ok(_) => {
                break;
            }
            Err(e) => {
                if is_driver_async_installed {
                    if let win_device::DeviceError::WinApiLastErr(_, e2) = &e {
                        if e2.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as _) {
                            if now.elapsed() < timeout {
                                std::thread::sleep(Duration::from_millis(100));
                                continue;
                            }
                        }
                    }
                }
                return Err(e.into());
            }
        }
    }
    // Workaround for the issue that we can't set the default the resolution.
    if let Ok(old_connectivity_old) = reg_connectivity_old {
        std::thread::spawn(move || {
            try_reset_resolution_on_first_plug_in(old_connectivity_old.len(), 1920, 1080);
        });
    }

    Ok(())
}

fn try_reset_resolution_on_first_plug_in(
    old_connectivity_len: usize,
    width: usize,
    height: usize,
) {
    for _ in 0..10 {
        std::thread::sleep(Duration::from_millis(300));
        if let Ok(reg_connectivity_new) = reg_display_settings::read_reg_connectivity() {
            if reg_connectivity_new.len() != old_connectivity_len {
                for name in
                    windows::get_device_names(Some(crate::virtual_display_manager::AMYUNI_IDD_DEVICE_STRING)).iter()
                {
                    crate::platform::change_resolution(&name, width, height).ok();
                }
                break;
            }
        }
    }
}

pub fn plug_in_headless() -> ResultType<()> {
    let mut tm = LAST_PLUG_IN_HEADLESS_TIME.lock().unwrap();
    if let Some(tm) = &mut *tm {
        if tm.elapsed() < Duration::from_secs(3) {
            bail!("Plugging in too frequently.");
        }
    }
    *tm = Some(Instant::now());
    drop(tm);

    let mut is_async = false;
    if let Err(e) = check_install_driver(&mut is_async) {
        log::error!("Failed to install driver: {}", e);
        bail!("Failed to install driver.");
    }

    plug_in_monitor_(true, is_async, Some(Duration::from_millis(3_000)))
}

pub fn plug_in_monitor() -> ResultType<()> {
    let mut is_async = false;
    if let Err(e) = check_install_driver(&mut is_async) {
        log::error!("Failed to install driver: {}", e);
        bail!("Failed to install driver.");
    }

    if get_monitor_count() == VIRTUAL_DISPLAY_MAX_COUNT {
        bail!("There are already {VIRTUAL_DISPLAY_MAX_COUNT} monitors plugged in.");
    }

    plug_in_monitor_(true, is_async, None)
}

// `index` the display index to plug out. -1 means plug out all.
// `force_all` is used to forcibly plug out all virtual displays.
// `force_one` is used to forcibly plug out one virtual display managed by other processes
//             if there're no virtual displays managed by RustDesk.
pub fn plug_out_monitor(index: i32, force_all: bool, force_one: bool) -> ResultType<()> {
    let plug_out_all = index == crate::virtual_display_manager::IDD_PLUG_OUT_ALL_INDEX;
    // If `plug_out_all and force_all` is true, forcibly plug out all virtual displays.
    // Though the driver may be controlled by other processes,
    // we still forcibly plug out all virtual displays.
    //
    // 1. RustDesk plug in 2 virtual displays. (RustDesk)
    // 2. Other process plug out all virtual displays. (User manually)
    // 3. Other process plug in 1 virtual display. (User manually)
    // 4. RustDesk plug out all virtual displays in this call. (RustDesk disconnect)
    //
    // This is not a normal scenario, RustDesk will plug out virtual display unexpectedly.
    let mut plug_in_count = VIRTUAL_DISPLAY_COUNT.load(atomic::Ordering::Relaxed);
    let amyuni_count = get_monitor_count();
    if !plug_out_all {
        if plug_in_count == 0 && amyuni_count > 0 {
            if force_one {
                plug_in_count = 1;
            } else {
                bail!("The virtual display is managed by other processes.");
            }
        }
    } else {
        // Ignore the message if trying to plug out all virtual displays.
    }

    let all_count = windows::get_device_names(None).len();
    let mut to_plug_out_count = match all_count {
        0 => return Ok(()),
        1 => {
            if plug_in_count == 0 {
                bail!("No virtual displays to plug out.")
            } else {
                if force_all {
                    1
                } else {
                    bail!("This only virtual display cannot be plugged out.")
                }
            }
        }
        _ => {
            if all_count == plug_in_count {
                if force_all {
                    all_count
                } else {
                    all_count - 1
                }
            } else {
                plug_in_count
            }
        }
    };
    if to_plug_out_count != 0 && !plug_out_all {
        to_plug_out_count = 1;
    }

    for _i in 0..to_plug_out_count {
        let _ = plug_monitor_(false, None);
    }
    Ok(())
}

#[inline]
pub fn get_monitor_count() -> usize {
    windows::get_device_names(Some(crate::virtual_display_manager::AMYUNI_IDD_DEVICE_STRING)).len()
}

#[inline]
pub fn is_my_display(name: &str) -> bool {
    windows::get_device_names(Some(crate::virtual_display_manager::AMYUNI_IDD_DEVICE_STRING))
        .iter()
        .any(|s| windows::is_device_name(s, name))
}
