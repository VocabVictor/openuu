use super::*;

pub(super) fn play(sp: &GenericService) -> ResultType<ActiveCaptureStream> {
    use cpal::SampleFormat::*;
    let (device, config) = get_device()?;
    let sp = sp.clone();
    // Sample rate must be one of 8000, 12000, 16000, 24000, or 48000.
    let sample_rate_0 = config.sample_rate().0;
    let sample_rate = if sample_rate_0 < 12000 {
        8000
    } else if sample_rate_0 < 16000 {
        12000
    } else if sample_rate_0 < 24000 {
        16000
    } else if sample_rate_0 < 48000 {
        24000
    } else {
        48000
    };
    let ch = if config.channels() > 1 { Stereo } else { Mono };
    let max_channels = config.channels().max(ch as u16);
    let (_, max_packet_samples) = capture_packet_layout(sample_rate, max_channels)?;
    let encoder_config = audio_capture_queue::CaptureEncoderConfig {
        sample_rate,
        encode_channel: ch,
        max_packet_samples,
    };
    let (sender, encoder_worker) =
        audio_capture_queue::start_capture_encoder(encoder_config, sp)?;
    let output = CaptureStreamOutput {
        sender,
        sample_rate,
        encode_channel: ch,
    };
    let (stream, errors) = match config.sample_format() {
        I8 => build_input_stream::<i8>(device, &config, output)?,
        I16 => build_input_stream::<i16>(device, &config, output)?,
        I32 => build_input_stream::<i32>(device, &config, output)?,
        I64 => build_input_stream::<i64>(device, &config, output)?,
        U8 => build_input_stream::<u8>(device, &config, output)?,
        U16 => build_input_stream::<u16>(device, &config, output)?,
        U32 => build_input_stream::<u32>(device, &config, output)?,
        U64 => build_input_stream::<u64>(device, &config, output)?,
        F32 => build_input_stream::<f32>(device, &config, output)?,
        F64 => build_input_stream::<f64>(device, &config, output)?,
        f => bail!("unsupported audio format: {:?}", f),
    };
    stream.play()?;
    #[cfg(target_os = "macos")]
    log::info!("Audio capture start call succeeded");
    Ok(ActiveCaptureStream {
        stream: Some(Box::new(stream)),
        format: Arc::new(create_format_msg(sample_rate, ch as _)),
        _encoder_worker: encoder_worker,
        errors,
    })
}

pub(super) fn convert_input_samples<T>(data: &[T]) -> impl Iterator<Item = f32> + '_
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    data.iter()
        .map(|sample| <f32 as cpal::FromSample<T>>::from_sample_(*sample))
}

#[cfg(target_os = "macos")]
pub(super) fn log_capture_startup<T>(
    data: &[T],
    received_samples: bool,
    received_signal: bool,
) -> (bool, bool)
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    // Starting capture does not guarantee sample delivery or audible data.
    if !received_samples && !data.is_empty() {
        log::info!(
            "Audio capture received first PCM block: {} samples",
            data.len()
        );
    }
    let has_signal = received_signal
        || convert_input_samples(data).any(|sample| sample.is_finite() && sample != 0.0);
    if !received_signal && has_signal {
        log::info!("Audio capture received first nonzero PCM");
    }
    (received_samples || !data.is_empty(), has_signal)
}

pub(super) fn build_input_stream<T>(
    device: cpal::Device,
    config: &cpal::SupportedStreamConfig,
    output: CaptureStreamOutput,
) -> ResultType<(cpal::Stream, CaptureErrorHandler)>
where
    T: cpal::SizedSample,
    f32: cpal::FromSample<T>,
{
    let errors = CaptureErrorHandler::default();
    let callback_errors = errors.clone();
    let err_fn = move |err| callback_errors.handle(err);
    let processor_errors = errors.clone();
    #[cfg(target_os = "macos")]
    let (mut received_samples, mut received_signal) = (false, false);
    let sample_rate_0 = config.sample_rate().0;
    log::debug!("Audio sample rate : {}", output.sample_rate);
    let device_channel = config.channels();
    let (_, capture_frame_samples) = capture_packet_layout(sample_rate_0, device_channel)?;
    let mut frame = audio_capture::CaptureFrameBuffer::new(capture_frame_samples)?;
    let processor_config = CaptureFrameProcessorConfig {
        input_rate: sample_rate_0,
        output_rate: output.sample_rate,
        device_channel,
        encode_channel: output.encode_channel as _,
    };
    let mut processor = CaptureFrameProcessor::new(processor_config, output.sender)?;
    let timeout = None;
    let stream = device.build_input_stream(
        &config.config(),
        move |data: &[T], _: &InputCallbackInfo| {
            if processor_errors.needs_restart() {
                return;
            }
            #[cfg(target_os = "macos")]
            {
                (received_samples, received_signal) =
                    log_capture_startup(data, received_samples, received_signal);
            }
            frame.process(convert_input_samples(data), |frame| {
                processor_errors.process_frame(|| processor.process(frame));
            });
        },
        err_fn,
        timeout,
    )?;
    Ok((stream, errors))
}
