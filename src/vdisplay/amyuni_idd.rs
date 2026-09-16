use super::windows;

mod plug;
pub use plug::*;
use crate::platform::{reg_display_settings, win_device};
use hbb_common::{bail, lazy_static, log, tokio::time::Instant, ResultType};
use std::{
    ptr::null_mut,
    sync::{atomic, Arc, Mutex},
    time::Duration,
};
use winapi::{
    shared::{guiddef::GUID, winerror::ERROR_NO_MORE_ITEMS},
    um::shellapi::ShellExecuteA,
};

const INF_PATH: &str = r#"usbmmidd_v2\usbmmIdd.inf"#;
const INTERFACE_GUID: GUID = GUID {
    Data1: 0xb5ffd75f,
    Data2: 0xda40,
    Data3: 0x4353,
    Data4: [0x8f, 0xf8, 0xb6, 0xda, 0xf6, 0xf1, 0xd8, 0xca],
};
const HARDWARE_ID: &str = "usbmmidd";
const PLUG_MONITOR_IO_CONTROL_CDOE: u32 = 2307084;
const INSTALLER_EXE_FILE: &str = "deviceinstaller64.exe";

lazy_static::lazy_static! {
    static ref LOCK: Arc<Mutex<()>> = Default::default();
    static ref LAST_PLUG_IN_HEADLESS_TIME: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
}
const VIRTUAL_DISPLAY_MAX_COUNT: usize = 4;
// The count of virtual displays plugged in.
// This count is not accurate, because:
// 1. The virtual display driver may also be controlled by other processes.
// 2. RustDesk may crash and restart, but the virtual displays are kept.
//
// to-do: Maybe a better way is to add an option asking the user if plug out all virtual displays on disconnect.
static VIRTUAL_DISPLAY_COUNT: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

fn get_deviceinstaller64_work_dir() -> ResultType<Option<Vec<u8>>> {
    let cur_exe = std::env::current_exe()?;
    let Some(cur_dir) = cur_exe.parent() else {
        bail!("Cannot get parent of current exe file.");
    };
    let work_dir = cur_dir.join("usbmmidd_v2");
    if !work_dir.exists() {
        return Ok(None);
    }
    let exe_path = work_dir.join(INSTALLER_EXE_FILE);
    if !exe_path.exists() {
        return Ok(None);
    }

    let Some(work_dir) = work_dir.to_str() else {
        bail!("Cannot convert work_dir to string.");
    };
    let mut work_dir2 = work_dir.as_bytes().to_vec();
    work_dir2.push(0);
    Ok(Some(work_dir2))
}

pub fn uninstall_driver() -> ResultType<()> {
    if let Ok(Some(work_dir)) = get_deviceinstaller64_work_dir() {
        if crate::platform::windows::is_x64() {
            log::info!("Uninstalling driver by deviceinstaller64.exe");
            install_if_x86_on_x64(&work_dir, "remove usbmmidd")?;
            // Sleep some time to wait for the driver to be uninstalled.
            std::thread::sleep(Duration::from_secs(2));
            return Ok(());
        }
    }

    log::info!("Uninstalling driver by SetupAPI");
    let mut reboot_required = false;
    let _ = unsafe { win_device::uninstall_driver(HARDWARE_ID, &mut reboot_required)? };
    Ok(())
}

// SetupDiCallClassInstaller() will always fail if current_exe() is built as x86 and running on x64.
// So we need to call another x64 version exe to install and uninstall the driver.
fn install_if_x86_on_x64(work_dir: &[u8], args: &str) -> ResultType<()> {
    const SW_HIDE: i32 = 0;
    let mut args = args.bytes().collect::<Vec<_>>();
    args.push(0);
    let mut exe_file = INSTALLER_EXE_FILE.bytes().collect::<Vec<_>>();
    exe_file.push(0);
    let hi = unsafe {
        ShellExecuteA(
            null_mut(),
            "open\0".as_ptr() as _,
            exe_file.as_ptr() as _,
            args.as_ptr() as _,
            work_dir.as_ptr() as _,
            SW_HIDE,
        ) as i32
    };
    if hi <= 32 {
        log::error!("Failed to run deviceinstaller: {}", hi);
        bail!("Failed to run deviceinstaller.")
    }
    Ok(())
}

// If the driver is installed by "deviceinstaller64.exe", the driver will be installed asynchronously.
// The caller must wait some time before using the driver.
fn check_install_driver(is_async: &mut bool) -> ResultType<()> {
    let _l = LOCK.lock().unwrap();
    let drivers = windows::get_display_drivers();
    if drivers
        .iter()
        .any(|(s, c)| s == super::AMYUNI_IDD_DEVICE_STRING && *c == 0)
    {
        *is_async = false;
        return Ok(());
    }

    if let Ok(Some(work_dir)) = get_deviceinstaller64_work_dir() {
        if crate::platform::windows::is_x64() {
            log::info!("Installing driver by deviceinstaller64.exe");
            install_if_x86_on_x64(&work_dir, "install usbmmidd.inf usbmmidd")?;
            *is_async = true;
            return Ok(());
        }
    }

    let exe_file = std::env::current_exe()?;
    let Some(cur_dir) = exe_file.parent() else {
        bail!("Cannot get parent of current exe file");
    };
    let inf_path = cur_dir.join(INF_PATH);
    if !inf_path.exists() {
        bail!("Driver inf file not found.");
    }
    let inf_path = inf_path.to_string_lossy().to_string();

    log::info!("Installing driver by SetupAPI");
    let mut reboot_required = false;
    let _ =
        unsafe { win_device::install_driver(&inf_path, HARDWARE_ID, &mut reboot_required)? };
    *is_async = false;
    Ok(())
}

pub fn reset_all() -> ResultType<()> {
    let _ = crate::privacy_mode::turn_off_privacy(0, None);
    let _ = plug_out_monitor(super::IDD_PLUG_OUT_ALL_INDEX, true, false);
    *LAST_PLUG_IN_HEADLESS_TIME.lock().unwrap() = None;
    Ok(())
}
