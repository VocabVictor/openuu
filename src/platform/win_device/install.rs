use super::*;

pub unsafe fn install_driver(
    inf_path: &str,
    hardware_id: &str,
    reboot_required: &mut bool,
) -> Result<(), DeviceError> {
    let driver_inf_path = OsStr::new(inf_path)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<u16>>();
    let hardware_id = OsStr::new(hardware_id)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect::<Vec<u16>>();

    let mut class_guid: GUID = std::mem::zeroed();
    let mut class_name: [u16; 32] = [0; 32];

    if SetupDiGetINFClassW(
        driver_inf_path.as_ptr(),
        &mut class_guid,
        class_name.as_mut_ptr(),
        class_name.len() as _,
        null_mut(),
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err("SetupDiGetINFClassW"));
    }

    let dev_info = DeviceInfo::setup_di_create_device_info_list(&mut class_guid)?;

    let mut dev_info_data = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as _,
        ClassGuid: class_guid,
        DevInst: 0,
        Reserved: 0,
    };
    if SetupDiCreateDeviceInfoW(
        *dev_info,
        class_name.as_ptr(),
        &class_guid,
        null_mut(),
        null_mut(),
        DICD_GENERATE_ID,
        &mut dev_info_data,
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err("SetupDiCreateDeviceInfoW"));
    }

    if SetupDiSetDeviceRegistryPropertyW(
        *dev_info,
        &mut dev_info_data,
        SPDRP_HARDWAREID,
        hardware_id.as_ptr() as _,
        (hardware_id.len() * 2) as _,
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err(
            "SetupDiSetDeviceRegistryPropertyW",
        ));
    }

    if SetupDiCallClassInstaller(DIF_REGISTERDEVICE, *dev_info, &mut dev_info_data) == FALSE {
        return Err(DeviceError::new_api_last_err("SetupDiCallClassInstaller"));
    }

    let mut reboot_required_ = FALSE;
    if UpdateDriverForPlugAndPlayDevicesW(
        null_mut(),
        hardware_id.as_ptr(),
        driver_inf_path.as_ptr(),
        1,
        &mut reboot_required_,
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err(
            "UpdateDriverForPlugAndPlayDevicesW",
        ));
    }
    *reboot_required = reboot_required_ == TRUE;

    Ok(())
}

pub(super) unsafe fn is_same_hardware_id(
    dev_info: &DeviceInfo,
    devinfo_data: &mut SP_DEVINFO_DATA,
    hardware_id: &str,
) -> Result<bool, DeviceError> {
    let mut cur_hardware_id = [0u16; MAX_DEVICE_ID_LEN];
    if SetupDiGetDeviceRegistryPropertyW(
        **dev_info,
        devinfo_data,
        SPDRP_HARDWAREID,
        null_mut(),
        cur_hardware_id.as_mut_ptr() as _,
        cur_hardware_id.len() as _,
        null_mut(),
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err(
            "SetupDiGetDeviceRegistryPropertyW",
        ));
    }

    let cur_hardware_id = String::from_utf16_lossy(&cur_hardware_id)
        .trim_end_matches(char::from(0))
        .to_string();
    Ok(cur_hardware_id == hardware_id)
}
