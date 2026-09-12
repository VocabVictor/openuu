use super::*;

#[derive(Clone, Copy)]
pub(super) struct CaptureFrameProcessorConfig {
    pub(super) input_rate: u32,
    pub(super) output_rate: u32,
    pub(super) device_channel: u16,
    pub(super) encode_channel: u16,
}

pub(super) struct CaptureFrameProcessor {
    pub(super) config: CaptureFrameProcessorConfig,
    pub(super) resampler: Option<crate::audio_resampler::FixedFrameAudioResampler>,
    pub(super) sender: audio_capture_queue::CapturePcmSender,
    pub(super) rechannel_buffer: Vec<f32>,
}

pub(super) struct CaptureStreamOutput {
    pub(super) sender: audio_capture_queue::CapturePcmSender,
    pub(super) sample_rate: u32,
    pub(super) encode_channel: magnum_opus::Channels,
}

impl CaptureFrameProcessor {
    pub(super) fn new(
        config: CaptureFrameProcessorConfig,
        sender: audio_capture_queue::CapturePcmSender,
    ) -> ResultType<Self> {
        let resampler = if config.input_rate == config.output_rate {
            None
        } else {
            let output_frames = config.output_rate as usize / AUDIO_PACKETS_PER_SECOND;
            Some(crate::audio_resampler::FixedFrameAudioResampler::new(
                crate::audio_resampler::AudioResamplerConfig {
                    input_rate: config.input_rate,
                    output_rate: config.output_rate,
                    channels: config.device_channel,
                },
                output_frames,
            )?)
        };
        Ok(Self {
            config,
            resampler,
            sender,
            rechannel_buffer: Vec::with_capacity(
                capture_packet_layout(config.output_rate, config.encode_channel)?.1,
            ),
        })
    }

    pub(super) fn process(&mut self, data: &[f32]) -> ResultType<()> {
        let config = self.config;
        let sender = &mut self.sender;
        let rechannel_buffer = &mut self.rechannel_buffer;
        let mut send_packet = |packet: &[f32]| {
            let packet =
                audio_capture::rechannel(packet, config.device_channel, rechannel_buffer);
            sender.submit(packet);
        };
        if let Some(resampler) = self.resampler.as_mut() {
            resampler.process_with(data, send_packet).with_context(|| {
                format!(
                    "Failed to resample captured audio from {} Hz to {} Hz",
                    config.input_rate, config.output_rate
                )
            })?;
        } else {
            send_packet(data);
        }
        Ok(())
    }
}

pub(super) fn capture_packet_layout(sample_rate: u32, channels: u16) -> ResultType<(usize, usize)> {
    if sample_rate < AUDIO_PACKETS_PER_SECOND as u32 || channels == 0 {
        bail!("Invalid audio capture layout: sample_rate={sample_rate}, channels={channels}");
    }
    let frames = sample_rate as usize / AUDIO_PACKETS_PER_SECOND;
    let samples = frames.checked_mul(channels as usize).with_context(|| {
        format!(
            "Audio capture frame size overflow: sample_rate={sample_rate}, channels={channels}"
        )
    })?;
    Ok((frames, samples))
}
