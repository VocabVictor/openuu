use base::platform::windows::is_windows_version_or_greater;
use hbb_common::{bail, ResultType};

pub mod rustdesk_idd;
mod windows;
pub mod amyuni_idd;

// This string is defined here.
//  https://github.com/rustdesk-org/RustDeskIddDriver/blob/b370aad3f50028b039aad211df60c8051c4a64d6/RustDeskIddDriver/RustDeskIddDriver.inf#LL73C1-L73C40
pub const RUSTDESK_IDD_DEVICE_STRING: &'static str = "RustDeskIddDriver Device\0";
pub const AMYUNI_IDD_DEVICE_STRING: &'static str = "USB Mobile Monitor Virtual Display\0";

const IDD_IMPL: &str = IDD_IMPL_AMYUNI;
const IDD_IMPL_RUSTDESK: &str = "rustdesk_idd";
const IDD_IMPL_AMYUNI: &str = "amyuni_idd";
const IDD_PLUG_OUT_ALL_INDEX: i32 = -1;

pub fn is_amyuni_idd() -> bool {
    IDD_IMPL == IDD_IMPL_AMYUNI
}

pub fn get_cur_device_string() -> &'static str {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => RUSTDESK_IDD_DEVICE_STRING,
        IDD_IMPL_AMYUNI => AMYUNI_IDD_DEVICE_STRING,
        _ => "",
    }
}

pub fn is_virtual_display_supported() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_windows_version_or_greater(10, 0, 19041, 0, 0)
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub fn plug_in_headless() -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_in_headless(),
        IDD_IMPL_AMYUNI => amyuni_idd::plug_in_headless(),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn get_platform_additions() -> serde_json::Map<String, serde_json::Value> {
    let mut map = serde_json::Map::new();
    if !crate::platform::windows::is_self_service_running() {
        return map;
    }
    map.insert("idd_impl".into(), serde_json::json!(IDD_IMPL));
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => {
            let virtual_displays = rustdesk_idd::get_virtual_displays();
            if !virtual_displays.is_empty() {
                map.insert(
                    "rustdesk_virtual_displays".into(),
                    serde_json::json!(virtual_displays),
                );
            }
        }
        IDD_IMPL_AMYUNI => {
            let c = amyuni_idd::get_monitor_count();
            if c > 0 {
                map.insert("amyuni_virtual_displays".into(), serde_json::json!(c));
            }
        }
        _ => {}
    }
    map
}

#[inline]
pub fn plug_in_monitor(idx: u32, modes: Vec<virtual_display::MonitorMode>) -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_in_index_modes(idx, modes),
        IDD_IMPL_AMYUNI => amyuni_idd::plug_in_monitor(),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn plug_out_monitor(index: i32, force_all: bool, force_one: bool) -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => {
            let indices = if index == IDD_PLUG_OUT_ALL_INDEX {
                rustdesk_idd::get_virtual_displays()
            } else {
                vec![index as _]
            };
            rustdesk_idd::plug_out_peer_request(&indices)
        }
        IDD_IMPL_AMYUNI => amyuni_idd::plug_out_monitor(index, force_all, force_one),
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn plug_in_peer_request(modes: Vec<Vec<virtual_display::MonitorMode>>) -> ResultType<Vec<u32>> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_in_peer_request(modes),
        IDD_IMPL_AMYUNI => {
            amyuni_idd::plug_in_monitor()?;
            Ok(vec![0])
        }
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn plug_out_monitor_indices(
    indices: &[u32],
    force_all: bool,
    force_one: bool,
) -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::plug_out_peer_request(indices),
        IDD_IMPL_AMYUNI => {
            for _idx in indices.iter() {
                amyuni_idd::plug_out_monitor(0, force_all, force_one)?;
            }
            Ok(())
        }
        _ => bail!("Unsupported virtual display implementation."),
    }
}

pub fn reset_all() -> ResultType<()> {
    match IDD_IMPL {
        IDD_IMPL_RUSTDESK => rustdesk_idd::reset_all(),
        IDD_IMPL_AMYUNI => amyuni_idd::reset_all(),
        _ => bail!("Unsupported virtual display implementation."),
    }
}


