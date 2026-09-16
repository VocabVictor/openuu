use std::ptr::null_mut;
use winapi::{
    shared::{
        devguid::GUID_DEVCLASS_DISPLAY,
        minwindef::{DWORD, FALSE},
        ntdef::ULONG,
    },
    um::{
        cfgmgr32::{CM_Get_DevNode_Status, CR_SUCCESS},
        cguid::GUID_NULL,
        setupapi::{
            SetupDiEnumDeviceInfo, SetupDiGetClassDevsW, SetupDiGetDeviceRegistryPropertyW,
            SP_DEVINFO_DATA,
        },
        wingdi::{
            DEVMODEW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE, DISPLAY_DEVICE_MIRRORING_DRIVER,
        },
        winnt::HANDLE,
        winuser::{EnumDisplayDevicesW, EnumDisplaySettingsExW, ENUM_CURRENT_SETTINGS},
    },
};

const DIGCF_PRESENT: DWORD = 0x00000002;
const SPDRP_DEVICEDESC: DWORD = 0x00000000;
const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;

#[inline]
pub(super) fn is_device_name(device_name: &str, name: &str) -> bool {
    if name.len() == device_name.len() {
        name == device_name
    } else if name.len() > device_name.len() {
        false
    } else {
        &device_name[..name.len()] == name && device_name.as_bytes()[name.len() as usize] == 0
    }
}

pub(super) fn get_device_names(device_string: Option<&str>) -> Vec<String> {
    let mut device_names = Vec::new();
    let mut dd: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
    dd.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as DWORD;
    let mut i_dev_num = 0;
    loop {
        let result = unsafe { EnumDisplayDevicesW(null_mut(), i_dev_num, &mut dd, 0) };
        if result == 0 {
            break;
        }
        i_dev_num += 1;

        if 0 == (dd.StateFlags & DISPLAY_DEVICE_ACTIVE)
            || (dd.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER) > 0
        {
            continue;
        }

        let mut dm: DEVMODEW = unsafe { std::mem::zeroed() };
        dm.dmSize = std::mem::size_of::<DEVMODEW>() as _;
        dm.dmDriverExtra = 0;
        let ok = unsafe {
            EnumDisplaySettingsExW(
                dd.DeviceName.as_ptr(),
                ENUM_CURRENT_SETTINGS,
                &mut dm as _,
                0,
            )
        };
        if ok == FALSE {
            continue;
        }
        if dm.dmPelsHeight == 0 || dm.dmPelsWidth == 0 {
            continue;
        }

        if let (Ok(device_name), Ok(ds)) = (
            String::from_utf16(&dd.DeviceName),
            String::from_utf16(&dd.DeviceString),
        ) {
            if let Some(s) = device_string {
                if ds.len() >= s.len() && &ds[..s.len()] == s {
                    device_names.push(device_name);
                }
            } else {
                device_names.push(device_name);
            }
        }
    }
    device_names
}

pub(super) fn get_display_drivers() -> Vec<(String, u32)> {
    let mut display_drivers: Vec<(String, u32)> = Vec::new();

    let device_info_set = unsafe {
        SetupDiGetClassDevsW(
            &GUID_DEVCLASS_DISPLAY,
            null_mut(),
            null_mut(),
            DIGCF_PRESENT,
        )
    };

    if device_info_set == INVALID_HANDLE_VALUE {
        println!(
            "Failed to get device information set. Error: {}",
            std::io::Error::last_os_error()
        );
        return display_drivers;
    }

    let mut device_info_data = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
        ClassGuid: GUID_NULL,
        DevInst: 0,
        Reserved: 0,
    };

    let mut device_index = 0;
    loop {
        let result = unsafe {
            SetupDiEnumDeviceInfo(device_info_set, device_index, &mut device_info_data)
        };
        if result == 0 {
            break;
        }

        let mut data_type: DWORD = 0;
        let mut required_size: DWORD = 0;

        // Get the required buffer size for the driver description
        let mut buffer;
        unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                device_info_set,
                &mut device_info_data,
                SPDRP_DEVICEDESC,
                &mut data_type,
                null_mut(),
                0,
                &mut required_size,
            );

            buffer = vec![0; required_size as usize / 2];
            SetupDiGetDeviceRegistryPropertyW(
                device_info_set,
                &mut device_info_data,
                SPDRP_DEVICEDESC,
                &mut data_type,
                buffer.as_mut_ptr() as *mut u8,
                required_size,
                null_mut(),
            );
        }

        let Ok(driver_description) = String::from_utf16(&buffer) else {
            println!("Failed to convert driver description to string");
            device_index += 1;
            continue;
        };

        let mut status: ULONG = 0;
        let mut problem_number: ULONG = 0;
        // Get the device status and problem number
        let config_ret = unsafe {
            CM_Get_DevNode_Status(
                &mut status,
                &mut problem_number,
                device_info_data.DevInst,
                0,
            )
        };
        if config_ret != CR_SUCCESS {
            println!(
                "Failed to get device status. Error: {}",
                std::io::Error::last_os_error()
            );
            device_index += 1;
            continue;
        }
        display_drivers.push((driver_description, problem_number));
        device_index += 1;
    }

    display_drivers
}
