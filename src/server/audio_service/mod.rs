// both soundio and cpal use wasapi on windows and coreaudio on mac, they do not support loopback.
// libpulseaudio support loopback because pulseaudio is a standalone audio service with some
// configuration, but need to install the library and start the service on OS, not a good choice.
// windows: https://docs.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording
// mac: https://github.com/mattingalls/Soundflower
// https://docs.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iaudioclient
// https://github.com/ExistentialAudio/BlackHole

// if pactl not work, please run
// sudo apt-get --purge --reinstall install pulseaudio
// https://askubuntu.com/questions/403416/how-to-listen-live-sounds-from-input-from-external-sound-card
// https://wiki.debian.org/audio-loopback
// https://github.com/krruzic/pulsectl

use super::*;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use hbb_common::anyhow::anyhow;
#[cfg(any(target_os = "linux", target_os = "android"))]
use magnum_opus::Application::LowDelay;
use magnum_opus::{Channels::*, Encoder};
use std::sync::atomic::{AtomicBool, Ordering};

pub const NAME: &'static str = "audio";
pub const AUDIO_DATA_SIZE_U8: usize = 960 * 4; // 10ms in 48000 stereo
static RESTARTING: AtomicBool = AtomicBool::new(false);

#[cfg(any(target_os = "linux", target_os = "android"))]
mod pa_impl;
mod encoder;
use encoder::*;

lazy_static::lazy_static! {
    static ref VOICE_CALL_INPUT_DEVICE: Arc::<Mutex::<Option<String>>> = Default::default();
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::repeat::<cpal_impl::State, _, _>(&svc.clone(), 33, cpal_impl::run);
    svc.sp
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), pa_impl::run);
    svc.sp
}

#[inline]
pub fn get_voice_call_input_device() -> Option<String> {
    VOICE_CALL_INPUT_DEVICE.lock().unwrap().clone()
}

#[inline]
pub fn set_voice_call_input_device(device: Option<String>, set_if_present: bool) {
    if !set_if_present && VOICE_CALL_INPUT_DEVICE.lock().unwrap().is_some() {
        return;
    }

    if *VOICE_CALL_INPUT_DEVICE.lock().unwrap() == device {
        return;
    }
    *VOICE_CALL_INPUT_DEVICE.lock().unwrap() = device;
    restart();
}

#[inline]
fn get_audio_input() -> String {
    VOICE_CALL_INPUT_DEVICE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or(Config::get_option("audio-input"))
}

pub fn restart() {
    log::info!("restart the audio service, freezing now...");
    if RESTARTING.load(Ordering::SeqCst) {
        return;
    }
    RESTARTING.store(true, Ordering::SeqCst);
}

#[inline]
#[cfg(feature = "screencapturekit")]
pub fn is_screen_capture_kit_available() -> bool {
    cpal::available_hosts()
        .iter()
        .any(|host| *host == cpal::HostId::ScreenCaptureKit)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod audio_capture;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod audio_capture_error;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod audio_capture_queue;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod cpal_impl {
    use self::service::{Reset, ServiceSwap};
    use super::audio_capture_error::CaptureErrorHandler;
    use super::*;
    use cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        Device, Host, InputCallbackInfo, SupportedStreamConfig,
    };

    lazy_static::lazy_static! {
        static ref HOST: Host = cpal::default_host();
    }

    const AUDIO_PACKETS_PER_SECOND: usize = 100;

    mod state_run;
    pub use state_run::*;
    mod processor;
    use processor::*;
    mod device;
    use device::*;
    mod stream;
    use stream::*;
    #[cfg(feature = "screencapturekit")]
    lazy_static::lazy_static! {
        static ref HOST_SCREEN_CAPTURE_KIT: Result<Host, cpal::HostUnavailable> = cpal::host_from_id(cpal::HostId::ScreenCaptureKit);
    }

    #[cfg(test)]
    mod tests {
        use super::super::audio_capture_queue::{
            new_pcm_handoff, start_capture_encoder, CaptureEncoderConfig,
        };
        use super::{
            capture_packet_layout, convert_input_samples, CaptureFrameProcessor,
            CaptureFrameProcessorConfig,
        };
        use crate::audio_resampler::allocation_tests::assert_no_allocations;
        use crate::server::EmptyExtraFieldService;
        use magnum_opus::Channels::{Mono, Stereo};

        const INVALID_CAPTURE_RATE: u32 = 99;
        const RATE_24_KHZ: u32 = 24_000;
        const RATE_44_1_KHZ: u32 = 44_100;
        const RATE_48_KHZ: u32 = 48_000;
        const MONO_CHANNELS: u16 = 1;
        const NEGATIVE_FULL_SCALE_LIMIT: f32 = -0.99;
        const POSITIVE_FULL_SCALE_LIMIT: f32 = 0.99;
        const STEREO_CHANNELS: u16 = 2;
        const SURROUND_CHANNELS: u16 = 6;
        const ZERO_CHANNELS: u16 = 0;

        #[test]
        fn capture_sample_conversion_uses_cpal_traits() {
            let input = [i16::MIN, 0, i16::MAX];
            let output: Vec<_> = convert_input_samples(&input).collect();

            assert_eq!(output.len(), input.len());
            assert!(output[0] <= NEGATIVE_FULL_SCALE_LIMIT);
            assert_eq!(output[1], 0.0);
            assert!(output[2] >= POSITIVE_FULL_SCALE_LIMIT);
        }

        #[test]
        fn capture_packet_layout_validates_rate_and_channels() {
            let expected_frames = RATE_48_KHZ as usize / super::AUDIO_PACKETS_PER_SECOND;
            assert_eq!(
                capture_packet_layout(RATE_48_KHZ, STEREO_CHANNELS).unwrap(),
                (expected_frames, expected_frames * STEREO_CHANNELS as usize)
            );
            assert!(capture_packet_layout(INVALID_CAPTURE_RATE, MONO_CHANNELS).is_err());
            assert!(capture_packet_layout(RATE_48_KHZ, ZERO_CHANNELS).is_err());
        }

        #[test]
        fn capture_callback_pipeline_does_not_allocate_after_warmup() {
            for (input_rate, output_rate, device_channel, encode_channel) in [
                (RATE_48_KHZ, RATE_48_KHZ, MONO_CHANNELS, MONO_CHANNELS),
                (RATE_48_KHZ, RATE_48_KHZ, STEREO_CHANNELS, STEREO_CHANNELS),
                (RATE_44_1_KHZ, RATE_24_KHZ, STEREO_CHANNELS, STEREO_CHANNELS),
                (RATE_48_KHZ, RATE_48_KHZ, SURROUND_CHANNELS, STEREO_CHANNELS),
            ] {
                assert_capture_processor_does_not_allocate(CaptureFrameProcessorConfig {
                    input_rate,
                    output_rate,
                    device_channel,
                    encode_channel,
                });
            }
        }

        #[test]
        fn capture_pcm_handoff_reuses_buffers_and_accounts_for_loss() {
            const QUEUE_CAPACITY: usize = 2;
            const PACKET_SAMPLES: usize = 4;
            const FIRST: [f32; PACKET_SAMPLES] = [1.0; PACKET_SAMPLES];
            const SECOND: [f32; PACKET_SAMPLES] = [2.0; PACKET_SAMPLES];
            const THIRD: [f32; PACKET_SAMPLES] = [3.0; PACKET_SAMPLES];
            const OVERSIZED_SAMPLES: usize = PACKET_SAMPLES + 1;
            const OVERSIZED: [f32; OVERSIZED_SAMPLES] = [1.0; OVERSIZED_SAMPLES];

            let (mut sender, receiver) = new_pcm_handoff(QUEUE_CAPACITY, PACKET_SAMPLES).unwrap();
            sender.set_wake_thread(std::thread::current()).unwrap();
            assert_no_allocations(|| {
                sender.submit(&FIRST);
                sender.submit(&SECOND);
                sender.submit(&THIRD);
            });

            let loss = receiver.take_loss();
            assert_eq!(loss.dropped, 1);
            assert_eq!(loss.oversized, 0);
            assert_eq!(loss.recycle_failures, 0);
            let second = receiver.pop().unwrap();
            let third = receiver.pop().unwrap();
            assert_eq!(second, SECOND);
            assert_eq!(third, THIRD);
            receiver.recycle(second);
            receiver.recycle(third);
            assert!(receiver.is_empty());

            assert_no_allocations(|| sender.submit(&OVERSIZED));
            let loss = receiver.take_loss();
            assert_eq!(loss.dropped, 0);
            assert_eq!(loss.oversized, 1);
            assert_eq!(loss.recycle_failures, 0);
            assert!(receiver.is_empty());
        }

        #[test]
        fn capture_pcm_handoff_rejects_invalid_layouts() {
            assert!(new_pcm_handoff(0, 1).is_err());
            assert!(new_pcm_handoff(1, 0).is_err());
        }

        fn assert_capture_processor_does_not_allocate(config: CaptureFrameProcessorConfig) {
            const INPUT_LEVEL: f32 = 0.25;
            const TEST_SERVICE_NAME: &str = "audio-allocation-test";

            let service = EmptyExtraFieldService::new(TEST_SERVICE_NAME.to_owned(), true).sp;
            let encode_channel = if config.encode_channel == MONO_CHANNELS {
                Mono
            } else {
                Stereo
            };
            let encoder_config = CaptureEncoderConfig {
                sample_rate: config.output_rate,
                encode_channel,
                max_packet_samples: config.output_rate as usize / super::AUDIO_PACKETS_PER_SECOND
                    * config.device_channel.max(config.encode_channel) as usize,
            };
            let (sender, worker) = start_capture_encoder(encoder_config, service).unwrap();
            let mut processor = CaptureFrameProcessor::new(config, sender).unwrap();
            let errors = super::CaptureErrorHandler::default();
            let input = vec![
                INPUT_LEVEL;
                config.input_rate as usize / super::AUDIO_PACKETS_PER_SECOND
                    * config.device_channel as usize
            ];
            let mut frame_buffer =
                super::audio_capture::CaptureFrameBuffer::new(input.len()).unwrap();

            frame_buffer.process(convert_input_samples(&input), |frame| {
                errors.process_frame(|| processor.process(frame));
            });
            assert_no_allocations(|| {
                frame_buffer.process(convert_input_samples(&input), |frame| {
                    errors.process_frame(|| processor.process(frame));
                });
            });
            assert!(!errors.needs_restart());
            drop(processor);
            drop(worker);
        }
    }
}
