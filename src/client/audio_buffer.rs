use super::*;

#[cfg(not(target_os = "linux"))]
#[derive(Clone, Copy)]
pub(super) struct DecodedAudioConfig {
    pub(super) sample_rate: u32,
    pub(super) input_channels: u16,
    pub(super) output_channels: u16,
}

#[cfg(not(target_os = "linux"))]
pub(super) fn create_audio_resampler(
    input_rate: u32,
    output_rate: u32,
    channels: u16,
) -> ResultType<Option<crate::audio_resampler::AudioResampler>> {
    if input_rate == output_rate {
        return Ok(None);
    }
    Ok(Some(crate::audio_resampler::AudioResampler::new(
        crate::audio_resampler::AudioResamplerConfig {
            input_rate,
            output_rate,
            channels,
        },
    )?))
}

#[cfg(not(target_os = "linux"))]
pub(super) fn prepare_decoded_audio(
    input: &[f32],
    resampler: Option<&mut crate::audio_resampler::AudioResampler>,
    config: DecodedAudioConfig,
) -> Result<Vec<f32>, crate::audio_resampler::AudioResamplerError> {
    let mut output = match resampler {
        Some(resampler) => resampler.process(input)?,
        None => input.to_owned(),
    };
    if config.input_channels != config.output_channels {
        output = crate::audio_rechannel(
            output,
            config.sample_rate,
            config.sample_rate,
            config.input_channels,
            config.output_channels,
        );
    }
    Ok(output)
}

#[cfg(not(target_os = "linux"))]
pub(super) struct AudioBuffer(
    pub Arc<std::sync::Mutex<ringbuf::HeapRb<f32>>>,
    pub(super) usize,
    pub(super) [usize; 30],
    pub(super) Arc<std::sync::atomic::AtomicUsize>,
);

#[cfg(not(target_os = "linux"))]
impl Default for AudioBuffer {
    fn default() -> Self {
        Self(
            Arc::new(std::sync::Mutex::new(
                ringbuf::HeapRb::<f32>::new(48000 * 2 * AUDIO_BUFFER_MS / 1000), // 48000hz, 2 channel
            )),
            48000 * 2,
            [0; 30],
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        )
    }
}

#[cfg(not(target_os = "linux"))]
impl AudioBuffer {
    pub fn resize(&mut self, sample_rate: usize, channels: usize) {
        let capacity = sample_rate * channels * AUDIO_BUFFER_MS / 1000;
        let old_capacity = self.0.lock().unwrap().capacity();
        if capacity != old_capacity {
            *self.0.lock().unwrap() = ringbuf::HeapRb::<f32>::new(capacity);
            self.1 = sample_rate * channels;
            log::info!("Audio buffer resized from {old_capacity} to {capacity}");
        }
    }

    pub(super) fn try_shrink(&mut self, having: usize) {
        extern crate chrono;
        use chrono::prelude::*;

        let mut i = (having * 10) / self.1;
        if i > 29 {
            i = 29;
        }
        self.2[i] += 1;

        #[allow(non_upper_case_globals)]
        static mut tms: i64 = 0;
        let dt = Local::now().timestamp_millis();
        unsafe {
            if tms == 0 {
                tms = dt;
                return;
            } else if dt < tms + 12000 {
                return;
            }
            tms = dt;
        }

        // the safer water mark to drop
        let mut zero = 0;
        // the water mark taking most of time
        let mut max = 0;
        for i in 0..30 {
            if self.2[i] == 0 && zero == i {
                zero += 1;
            }

            if self.2[i] > self.2[max] {
                self.2[max] = 0;
                max = i;
            } else {
                self.2[i] = 0;
            }
        }
        zero = zero * 2 / 3;

        // how many data can be dropped:
        // 1. will not drop if buffered data is less than 600ms
        // 2. choose based on min(zero, max)
        const N: usize = 4;
        self.2[max] = 0;
        if max < 6 {
            return;
        } else if max > zero * N {
            max = zero * N;
        }

        let mut lock = self.0.lock().unwrap();
        let cap = lock.capacity();
        let having = lock.occupied_len();
        let skip = (cap * max / (30 * N) + 1) & (!1);
        if (having > skip * 3) && (skip > 0) {
            lock.skip(skip);
            let generation = self.signal_discontinuity();
            drop(lock);
            log::info!("skip {skip}, based {max} {zero}, generation={generation}");
        }
    }

    /// The caller must hold the PCM buffer lock while signaling the discard.
    pub(super) fn signal_discontinuity(&self) -> usize {
        self.3
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .wrapping_add(1)
    }

    /// append pcm to audio buffer, if buffered data
    /// exceeds AUDIO_BUFFER_MS,  only AUDIO_BUFFER_MS
    /// will be kept.
    pub(super) fn append_pcm2(&self, buffer: &[f32]) -> usize {
        let mut lock = self.0.lock().unwrap();
        let cap = lock.capacity();
        let having = lock.occupied_len() + buffer.len();
        lock.push_slice_overwrite(buffer);
        let discard = (having > cap).then(|| (having - cap, self.signal_discontinuity()));
        let occupied = lock.occupied_len();
        drop(lock);
        if let Some((discarded, generation)) = discard {
            log::debug!(
                "Audio buffer capacity discard: samples={discarded}, generation={generation}"
            );
        }
        occupied
    }

    /// append pcm to audio buffer, trying to drop data
    /// when data is too much (per 12 seconds) based
    /// statistics.
    pub fn append_pcm(&mut self, buffer: &[f32]) {
        let having = self.append_pcm2(buffer);
        self.try_shrink(having);
    }
}

#[cfg(all(test, not(target_os = "linux")))]
mod audio_buffer_discontinuity_tests {
    use super::AudioBuffer;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    };

    pub(super) const BUFFER_CAPACITY: usize = 4;
    pub(super) const BUFFER_LEVELS: usize = 30;
    pub(super) const FIRST_INPUT: [f32; 2] = [0.1, 0.2];
    pub(super) const OVERFLOWING_INPUT: [f32; 3] = [0.3, 0.4, 0.5];
    pub(super) const OVERSIZED_INPUT: [f32; 5] = [0.6, 0.7, 0.8, 0.9, 1.0];

    #[test]
    fn capacity_discards_signal_discontinuities() {
        let audio_buffer = AudioBuffer(
            Arc::new(Mutex::new(ringbuf::HeapRb::new(BUFFER_CAPACITY))),
            BUFFER_CAPACITY,
            [0; BUFFER_LEVELS],
            Arc::new(AtomicUsize::new(0)),
        );

        assert_eq!(audio_buffer.append_pcm2(&FIRST_INPUT), FIRST_INPUT.len());
        assert_eq!(audio_buffer.3.load(Ordering::Relaxed), 0);
        assert_eq!(
            audio_buffer.append_pcm2(&OVERFLOWING_INPUT),
            BUFFER_CAPACITY
        );
        assert_eq!(audio_buffer.3.load(Ordering::Relaxed), 1);
        assert_eq!(audio_buffer.append_pcm2(&OVERSIZED_INPUT), BUFFER_CAPACITY);
        assert_eq!(audio_buffer.3.load(Ordering::Relaxed), 2);
    }
}
