use super::*;

pub(super) fn create_format_msg(sample_rate: u32, channels: u16) -> Message {
    let format = AudioFormat {
        sample_rate,
        channels: channels as _,
        ..Default::default()
    };
    let mut misc = Misc::new();
    misc.set_audio_format(format);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

// Use a per-encoder counter for the Noise(Zero) Gate Attack Time.
// every audio data length is set to 480
// MAX_AUDIO_ZERO_COUNT=800 is similar as Gate Attack Time 3~5s(Linux) || 6~8s(Windows)
pub(super) const MAX_AUDIO_ZERO_COUNT: u16 = 800;

pub(super) struct AudioEncoder {
    pub(super) encoder: Encoder,
    pub(super) zero_count: u16,
}

impl AudioEncoder {
    pub(super) fn new(encoder: Encoder) -> Self {
        Self {
            encoder,
            zero_count: 0,
        }
    }

    pub(super) fn should_encode(&mut self, data: &[f32]) -> bool {
        if data.iter().filter(|x| **x != 0.).next().is_some() {
            self.zero_count = 0;
        } else if self.zero_count > MAX_AUDIO_ZERO_COUNT {
            if self.zero_count == MAX_AUDIO_ZERO_COUNT + 1 {
                log::debug!("Audio Zero Gate Attack");
                self.zero_count += 1;
            }
            return false;
        } else {
            self.zero_count += 1;
        }
        true
    }
}

pub(super) fn send_f32(data: &[f32], encoder: &mut AudioEncoder, sp: &GenericService) {
    if !encoder.should_encode(data) {
        return;
    }
    #[cfg(target_os = "android")]
    {
        // the permitted opus data size are 120, 240, 480, 960, 1920, and 2880
        // if data size is bigger than BATCH_SIZE, AND is an integer multiple of BATCH_SIZE
        // then upload in batches
        const BATCH_SIZE: usize = 960;
        let input_size = data.len();
        if input_size > BATCH_SIZE && input_size % BATCH_SIZE == 0 {
            let n = input_size / BATCH_SIZE;
            for i in 0..n {
                match encoder
                    .encoder
                    .encode_vec_float(&data[i * BATCH_SIZE..(i + 1) * BATCH_SIZE], BATCH_SIZE)
                {
                    Ok(data) => {
                        let mut msg_out = Message::new();
                        msg_out.set_audio_frame(AudioFrame {
                            data: data.into(),
                            ..Default::default()
                        });
                        sp.send(msg_out);
                    }
                    Err(error) => log::warn!("Failed to encode audio frame: {error:?}"),
                }
            }
        } else {
            log::debug!("invalid audio data size:{} ", input_size);
            return;
        }
    }

    #[cfg(not(target_os = "android"))]
    match encoder.encoder.encode_vec_float(data, data.len() * 6) {
        Ok(data) => {
            let mut msg_out = Message::new();
            msg_out.set_audio_frame(AudioFrame {
                data: data.into(),
                ..Default::default()
            });
            sp.send(msg_out);
        }
        Err(error) => log::warn!("Failed to encode audio frame: {error:?}"),
    }
}
