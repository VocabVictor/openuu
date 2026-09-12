use super::*;

pub unsafe fn uninstall_driver(
    hardware_id: &str,
    reboot_required: &mut bool,
) -> Result<(), DeviceError> {
    let dev_info =
        DeviceInfo::setup_di_get_class_devs_ex_w(null_mut(), DIGCF_ALLCLASSES | DIGCF_PRESENT)?;

    let mut device_info_list_detail = SP_DEVINFO_LIST_DETAIL_DATA_W {
        cbSize: std::mem::size_of::<SP_DEVINFO_LIST_DETAIL_DATA_W>() as _,
        ClassGuid: std::mem::zeroed(),
        RemoteMachineHandle: null_mut(),
        RemoteMachineName: [0; SP_MAX_MACHINENAME_LENGTH],
    };
    if SetupDiGetDeviceInfoListDetailW(*dev_info, &mut device_info_list_detail) == FALSE {
        return Err(DeviceError::new_api_last_err(
            "SetupDiGetDeviceInfoListDetailW",
        ));
    }

    let mut devinfo_data = SP_DEVINFO_DATA {
        cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as _,
        ClassGuid: std::mem::zeroed(),
        DevInst: 0,
        Reserved: 0,
    };

    let mut device_index = 0;
    loop {
        if SetupDiEnumDeviceInfo(*dev_info, device_index, &mut devinfo_data) == FALSE {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as _) {
                break;
            }
            return Err(DeviceError::WinApiLastErr(
                "SetupDiEnumDeviceInfo".to_string(),
                err,
            ));
        }

        match is_same_hardware_id(&dev_info, &mut devinfo_data, hardware_id) {
            Ok(false) => {
                device_index += 1;
                continue;
            }
            Err(e) => {
                log::error!("Failed to call is_same_hardware_id, {:?}", e);
                device_index += 1;
                continue;
            }
            _ => {}
        }

        let mut remove_device_params = SP_REMOVEDEVICE_PARAMS {
            ClassInstallHeader: SP_CLASSINSTALL_HEADER {
                cbSize: std::mem::size_of::<SP_CLASSINSTALL_HEADER>() as _,
                InstallFunction: DIF_REMOVE,
            },
            Scope: DI_REMOVEDEVICE_GLOBAL,
            HwProfile: 0,
        };

        if SetupDiSetClassInstallParamsW(
            *dev_info,
            &mut devinfo_data,
            &mut remove_device_params.ClassInstallHeader,
            std::mem::size_of::<SP_REMOVEDEVICE_PARAMS>() as _,
        ) == FALSE
        {
            return Err(DeviceError::new_api_last_err(
                "SetupDiSetClassInstallParams",
            ));
        }

        if SetupDiCallClassInstaller(DIF_REMOVE, *dev_info, &mut devinfo_data) == FALSE {
            return Err(DeviceError::new_api_last_err("SetupDiCallClassInstaller"));
        }

        let mut device_params = SP_DEVINSTALL_PARAMS_W {
            cbSize: std::mem::size_of::<SP_DEVINSTALL_PARAMS_W>() as _,
            Flags: 0,
            FlagsEx: 0,
            hwndParent: null_mut(),
            InstallMsgHandler: None,
            InstallMsgHandlerContext: null_mut(),
            FileQueue: null_mut(),
            ClassInstallReserved: 0,
            Reserved: 0,
            DriverPath: [0; MAX_PATH],
        };

        if SetupDiGetDeviceInstallParamsW(*dev_info, &mut devinfo_data, &mut device_params) == FALSE
        {
            log::error!(
                "Failed to call SetupDiGetDeviceInstallParamsW, {:?}",
                io::Error::last_os_error()
            );
        } else {
            if device_params.Flags & (DI_NEEDRESTART | DI_NEEDREBOOT) != 0 {
                *reboot_required = true;
            }
        }

        device_index += 1;
    }

    Ok(())
}
