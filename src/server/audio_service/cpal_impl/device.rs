use super::*;

#[cfg(feature = "screencapturekit")]
pub(super) fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
    let audio_input = super::super::get_audio_input();
    if !audio_input.is_empty() {
        return get_audio_input(&audio_input);
    }
    if !is_screen_capture_kit_available() {
        return get_audio_input("");
    }
    let device = HOST_SCREEN_CAPTURE_KIT
        .as_ref()?
        .default_input_device()
        .with_context(|| "Failed to get default input device for loopback")?;
    let format = device
        .default_input_config()
        .map_err(|e| anyhow!(e))
        .with_context(|| "Failed to get input output format")?;
    log::info!("Default input format: {:?}", format);
    Ok((device, format))
}

#[cfg(windows)]
pub(super) fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
    let audio_input = super::super::get_audio_input();
    if !audio_input.is_empty() {
        return get_audio_input(&audio_input);
    }
    let device = HOST
        .default_output_device()
        .with_context(|| "Failed to get default output device for loopback")?;
    log::info!(
        "Default output device: {}",
        device.name().unwrap_or("".to_owned())
    );
    let format = device
        .default_output_config()
        .map_err(|e| anyhow!(e))
        .with_context(|| "Failed to get default output format")?;
    log::info!("Default output format: {:?}", format);
    Ok((device, format))
}

#[cfg(not(any(windows, feature = "screencapturekit")))]
pub(super) fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
    let audio_input = super::super::get_audio_input();
    get_audio_input(&audio_input)
}

pub(super) fn get_audio_input(audio_input: &str) -> ResultType<(Device, SupportedStreamConfig)> {
    let mut device = None;
    #[cfg(feature = "screencapturekit")]
    if !audio_input.is_empty() && is_screen_capture_kit_available() {
        for d in HOST_SCREEN_CAPTURE_KIT
            .as_ref()?
            .devices()
            .with_context(|| "Failed to get audio devices")?
        {
            if d.name().unwrap_or("".to_owned()) == audio_input {
                device = Some(d);
                break;
            }
        }
    }
    if device.is_none() && !audio_input.is_empty() {
        for d in HOST
            .devices()
            .with_context(|| "Failed to get audio devices")?
        {
            if d.name().unwrap_or("".to_owned()) == audio_input {
                device = Some(d);
                break;
            }
        }
    }
    let device = device.unwrap_or(
        HOST.default_input_device()
            .with_context(|| "Failed to get default input device for loopback")?,
    );
    log::info!("Input device: {}", device.name().unwrap_or("".to_owned()));
    let format = device
        .default_input_config()
        .map_err(|e| anyhow!(e))
        .with_context(|| "Failed to get default input format")?;
    log::info!("Default input format: {:?}", format);
    Ok((device, format))
}
