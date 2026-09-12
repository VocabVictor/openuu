use super::*;

/// Audio handler for the [`Client`].
#[derive(Default)]
pub struct AudioHandler {
    pub(super) audio_decoder: Option<(AudioDecoder, Vec<f32>)>,
    #[cfg(target_os = "linux")]
    pub(super) simple: Option<psimple::Simple>,
    #[cfg(not(target_os = "linux"))]
    pub(super) audio_buffer: AudioBuffer,
    #[cfg(not(target_os = "linux"))]
    pub(super) audio_resampler: Option<crate::audio_resampler::AudioResampler>,
    pub(super) sample_rate: (u32, u32),
    #[cfg(not(target_os = "linux"))]
    pub(super) audio_stream: Option<Box<dyn StreamTrait>>,
    pub(super) channels: u16,
    #[cfg(not(target_os = "linux"))]
    pub(super) device_channel: u16,
    #[cfg(not(target_os = "linux"))]
    pub(super) playback_status: Arc<audio_playback::AudioPlaybackStatus>,
}

impl AudioHandler {
    #[cfg(target_os = "linux")]
    pub(super) fn start_audio(&mut self, format0: AudioFormat) -> ResultType<()> {
        use psimple::Simple;
        use pulse::sample::{Format, Spec};
        use pulse::stream::Direction;

        let spec = Spec {
            format: Format::F32le,
            channels: format0.channels as _,
            rate: format0.sample_rate as _,
        };
        if !spec.is_valid() {
            bail!("Invalid audio format");
        }

        self.simple = Some(Simple::new(
            None,                   // Use the default server
            &crate::get_app_name(), // Our application’s name
            Direction::Playback,    // We want a playback stream
            None,                   // Use the default device
            "playback",             // Description of our stream
            &spec,                  // Our sample format
            None,                   // Use default channel map
            None,                   // Use default buffering attributes
        )?);
        self.sample_rate = (format0.sample_rate, format0.sample_rate);
        Ok(())
    }

    /// Start the audio playback.
    #[cfg(not(target_os = "linux"))]
    pub(super) fn start_audio(&mut self, format0: AudioFormat) -> ResultType<()> {
        let device = AUDIO_HOST
            .default_output_device()
            .with_context(|| "Failed to get default output device")?;
        log::info!(
            "Using default output device: \"{}\"",
            device.name().unwrap_or("".to_owned())
        );
        let config = device.default_output_config().map_err(|e| anyhow!(e))?;
        let sample_format = config.sample_format();
        log::info!("Default output format: {:?}", config);
        log::info!("Remote input format: {:?}", format0);
        #[allow(unused_mut)]
        let mut config: StreamConfig = config.into();
        #[cfg(not(target_os = "ios"))]
        {
            // this makes ios audio output not work
            config.buffer_size = cpal::BufferSize::Fixed(64);
        }

        self.sample_rate = (format0.sample_rate, config.sample_rate.0);
        let audio_resampler = create_audio_resampler(
            format0.sample_rate, config.sample_rate.0, format0.channels as _,
        )?;
        let mut build_output_stream = |config: StreamConfig| match sample_format {
            cpal::SampleFormat::I8 => self.build_output_stream::<i8>(&config, &device),
            cpal::SampleFormat::I16 => self.build_output_stream::<i16>(&config, &device),
            cpal::SampleFormat::I32 => self.build_output_stream::<i32>(&config, &device),
            cpal::SampleFormat::I64 => self.build_output_stream::<i64>(&config, &device),
            cpal::SampleFormat::U8 => self.build_output_stream::<u8>(&config, &device),
            cpal::SampleFormat::U16 => self.build_output_stream::<u16>(&config, &device),
            cpal::SampleFormat::U32 => self.build_output_stream::<u32>(&config, &device),
            cpal::SampleFormat::U64 => self.build_output_stream::<u64>(&config, &device),
            cpal::SampleFormat::F32 => self.build_output_stream::<f32>(&config, &device),
            cpal::SampleFormat::F64 => self.build_output_stream::<f64>(&config, &device),
            f => bail!("unsupported audio format: {:?}", f),
        };
        if config.channels > format0.channels as _ {
            let no_rechannel_config = StreamConfig {
                channels: format0.channels as _,
                ..config.clone()
            };
            if let Err(_) = build_output_stream(no_rechannel_config) {
                build_output_stream(config)?;
            }
        } else {
            build_output_stream(config)?;
        }
        self.audio_resampler = audio_resampler;

        Ok(())
    }

    /// Handle audio format and create an audio decoder.
    pub fn handle_format(&mut self, f: AudioFormat) {
        if !is_supported_audio_channel_count(f.channels) {
            log::error!("Unsupported audio channel count: {}", f.channels);
            return;
        }
        match AudioDecoder::new(f.sample_rate, if f.channels > 1 { Stereo } else { Mono }) {
            Ok(d) => {
                #[cfg(target_os = "linux")]
                let keep_existing_stream = self.simple.is_some()
                    && self.sample_rate.0 == f.sample_rate
                    && u32::from(self.channels) == f.channels;
                #[cfg(not(target_os = "linux"))]
                let keep_existing_stream = false;
                let buffer = vec![0.; f.sample_rate as usize * f.channels as usize];
                self.audio_decoder = Some((d, buffer));
                self.channels = f.channels as _;
                let result = self.start_audio(f);
                self.handle_audio_start_result(result, keep_existing_stream);
            }
            Err(err) => {
                log::error!("Failed to create audio decoder: {}", err);
            }
        }
    }

    pub(super) fn handle_audio_start_result(&mut self, result: ResultType<()>, keep_existing_stream: bool) {
        if let Err(error) = result {
            if keep_existing_stream {
                log::error!(
                    "Failed to replace audio playback stream; keeping the existing compatible stream: {error:#}"
                );
            } else {
                *self = Self::default();
                log::error!("Failed to start audio playback: {error:#}");
            }
        }
    }

    /// Handle audio frame and play it.
    #[inline]
    pub fn handle_frame(&mut self, frame: AudioFrame) {
        #[cfg(not(target_os = "linux"))]
        self.playback_status.report_errors();
        #[cfg(not(target_os = "linux"))]
        if self.audio_stream.is_none()
            || !self
                .playback_status
                .ready
                .load(std::sync::atomic::Ordering::Acquire)
        {
            return;
        }
        #[cfg(target_os = "linux")]
        if self.simple.is_none() {
            log::debug!("PulseAudio simple binding does not exists");
            return;
        }
        self.audio_decoder.as_mut().map(|(d, buffer)| {
            let decoded_frames = match d.decode_float(&frame.data, buffer, false) {
                Ok(decoded_frames) => decoded_frames,
                Err(error) => {
                    log::warn!("Failed to decode audio frame: {error:?}");
                    return;
                }
            };
            let channels = self.channels;
            let n = decoded_frames * channels as usize;
            #[cfg(not(target_os = "linux"))]
            {
                let config = DecodedAudioConfig {
                    sample_rate: self.sample_rate.1,
                    input_channels: self.channels,
                    output_channels: self.device_channel,
                };
                let buffer = match prepare_decoded_audio(
                    &buffer[0..n],
                    self.audio_resampler.as_mut(),
                    config,
                ) {
                    Ok(output) => output,
                    Err(error) => {
                        log::error!("Failed to resample decoded audio: {error:#}");
                        return;
                    }
                };
                self.audio_buffer.append_pcm(&buffer);
            }
            #[cfg(target_os = "linux")]
            {
                let data_u8 =
                    unsafe { std::slice::from_raw_parts::<u8>(buffer.as_ptr() as _, n * 4) };
                self.simple.as_mut().map(|x| x.write(data_u8));
            }
        });
    }

    /// Build audio output stream for current device.
    #[cfg(not(target_os = "linux"))]
    pub(super) fn build_output_stream<T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>>(
        &mut self,
        config: &StreamConfig,
        device: &Device,
    ) -> ResultType<()> {
        self.device_channel = config.channels;
        let err_fn = move |err| {
            // too many errors, will improve later
            log::trace!("an error occurred on stream: {}", err);
        };
        self.audio_buffer
            .resize(config.sample_rate.0 as _, config.channels as _);
        let audio_buffer = self.audio_buffer.0.clone();
        let discontinuity_generation = self.audio_buffer.3.clone();
        let mut playback_writer = audio_playback::AudioPlaybackWriter::new(
            audio_playback::AudioPlaybackConfig {
                sample_rate: config.sample_rate.0,
                channels: config.channels as usize,
            },
            audio_buffer,
            discontinuity_generation,
        )?;
        let playback_status = playback_writer.status.clone();
        let timeout = None;
        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                playback_writer.write_output(data);
            },
            err_fn,
            timeout,
        )?;
        stream.play()?;
        self.audio_stream = Some(Box::new(stream));
        self.playback_status = playback_status;
        Ok(())
    }
}

pub(super) fn is_supported_audio_channel_count(channels: u32) -> bool {
    (1..=2).contains(&channels)
}

#[cfg(test)]
mod audio_format_tests {
    use super::is_supported_audio_channel_count;

    #[test]
    fn only_mono_and_stereo_are_supported() {
        assert!(is_supported_audio_channel_count(1));
        assert!(is_supported_audio_channel_count(2));
        assert!(!is_supported_audio_channel_count(0));
        assert!(!is_supported_audio_channel_count(u32::MAX));
    }

    #[test]
    fn failed_audio_start_discards_format_state() {
        use super::{anyhow, AudioDecoder, AudioHandler, Stereo};

        const SAMPLE_RATE: u32 = 48_000;
        const CHANNELS: u16 = 2;
        let decoder = AudioDecoder::new(SAMPLE_RATE, Stereo).unwrap();
        let mut handler = AudioHandler {
            audio_decoder: Some((decoder, Vec::new())),
            sample_rate: (SAMPLE_RATE, SAMPLE_RATE),
            channels: CHANNELS,
            ..Default::default()
        };

        handler.handle_audio_start_result(Err(anyhow!("Injected playback startup failure")), false);

        assert!(handler.audio_decoder.is_none());
        assert_eq!(handler.channels, 0);
        assert_eq!(handler.sample_rate, (0, 0));
    }
}
