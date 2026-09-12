use super::*;

pub unsafe fn device_io_control(
    interface_guid: &GUID,
    control_code: u32,
    inbuf: &[u8],
    outbuf_max_len: usize,
) -> Result<Vec<u8>, DeviceError> {
    let h_device = open_device_handle(interface_guid)?;
    let mut bytes_returned = 0;
    let mut outbuf: Vec<u8> = vec![];
    let outbuf_ptr = if outbuf_max_len > 0 {
        outbuf.reserve(outbuf_max_len);
        outbuf.as_mut_ptr()
    } else {
        null_mut()
    };
    let result = DeviceIoControl(
        h_device,
        control_code,
        inbuf.as_ptr() as _,
        inbuf.len() as _,
        outbuf_ptr as _,
        outbuf_max_len as _,
        &mut bytes_returned,
        null_mut(),
    );
    CloseHandle(h_device);
    if result == FALSE {
        return Err(DeviceError::new_api_last_err("DeviceIoControl"));
    }
    if outbuf_max_len > 0 {
        outbuf.set_len(bytes_returned as _);
        Ok(outbuf)
    } else {
        Ok(Vec::new())
    }
}

pub(super) unsafe fn get_device_path(interface_guid: &GUID) -> Result<Vec<u16>, DeviceError> {
    let dev_info = DeviceInfo::setup_di_get_class_devs_ex_w(
        interface_guid,
        DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
    )?;
    let mut device_interface_data = SP_DEVICE_INTERFACE_DATA {
        cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as _,
        InterfaceClassGuid: *interface_guid,
        Flags: 0,
        Reserved: 0,
    };
    if SetupDiEnumDeviceInterfaces(
        *dev_info,
        null_mut(),
        interface_guid,
        0,
        &mut device_interface_data,
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err("SetupDiEnumDeviceInterfaces"));
    }

    let mut required_length = 0;
    if SetupDiGetDeviceInterfaceDetailW(
        *dev_info,
        &mut device_interface_data,
        null_mut(),
        0,
        &mut required_length,
        null_mut(),
    ) == FALSE
    {
        let err = io::Error::last_os_error();
        if err.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as _) {
            return Err(DeviceError::WinApiLastErr(
                "SetupDiGetDeviceInterfaceDetailW".to_string(),
                err,
            ));
        }
    }

    let predicted_length = required_length;
    let mut vec_data: Vec<u8> = Vec::with_capacity(required_length as _);
    let device_interface_detail_data = vec_data.as_mut_ptr();
    let device_interface_detail_data =
        device_interface_detail_data as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W;
    (*device_interface_detail_data).cbSize =
        std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as _;
    if SetupDiGetDeviceInterfaceDetailW(
        *dev_info,
        &mut device_interface_data,
        device_interface_detail_data,
        predicted_length,
        &mut required_length,
        null_mut(),
    ) == FALSE
    {
        return Err(DeviceError::new_api_last_err(
            "SetupDiGetDeviceInterfaceDetailW",
        ));
    }

    let mut path = Vec::new();
    let device_path_ptr =
        std::ptr::addr_of!((*device_interface_detail_data).DevicePath) as *const u16;
    let steps = device_path_ptr as usize - vec_data.as_ptr() as usize;
    for i in 0..(predicted_length - steps as u32) / 2 {
        if *device_path_ptr.offset(i as _) == 0 {
            path.push(0);
            break;
        }
        path.push(*device_path_ptr.offset(i as _));
    }
    Ok(path)
}

pub(super) unsafe fn open_device_handle(interface_guid: &GUID) -> Result<HANDLE, DeviceError> {
    let device_path = get_device_path(interface_guid)?;
    let h_device = CreateFileW(
        device_path.as_ptr(),
        GENERIC_READ | GENERIC_WRITE,
        0,
        null_mut(),
        OPEN_EXISTING,
        0,
        null_mut(),
    );
    if h_device == INVALID_HANDLE_VALUE || h_device == NULL {
        return Err(DeviceError::new_api_last_err("CreateFileW"));
    }
    Ok(h_device)
}
