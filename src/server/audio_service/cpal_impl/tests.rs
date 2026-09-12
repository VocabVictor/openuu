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
