use super::*;

/// Set sound input device.
pub fn set_sound_input(device: String) {
    let prior_device = get_option("audio-input".to_owned());
    if prior_device != device {
        log::info!("switch to audio input device {}", device);
        std::thread::spawn(move || {
            set_option("audio-input".to_owned(), device);
        });
    } else {
        log::info!("audio input is already set to {}", device);
    }
}

/// Get system's default sound input device name.
#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_default_sound_input() -> Option<String> {
    #[cfg(not(target_os = "linux"))]
    {
        use cpal::traits::{DeviceTrait, HostTrait};
        let host = cpal::default_host();
        let dev = host.default_input_device();
        return if let Some(dev) = dev {
            match dev.name() {
                Ok(name) => Some(name),
                Err(_) => None,
            }
        } else {
            None
        };
    }
    #[cfg(target_os = "linux")]
    {
        let input = crate::platform::linux::get_default_pa_source();
        return if let Some(input) = input {
            Some(input.1)
        } else {
            None
        };
    }
}

#[inline]
#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn get_default_sound_input() -> Option<String> {
    None
}

#[cfg(feature = "use_rubato")]
pub fn resample_channels(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use rubato::{
        InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction,
    };
    let params = InterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: InterpolationType::Nearest,
        oversampling_factor: 160,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut resampler = SincFixedIn::<f64>::new(
        sample_rate as f64 / sample_rate0 as f64,
        params,
        data.len() / (channels as usize),
        channels as _,
    );
    let mut waves_in = Vec::new();
    if channels == 2 {
        waves_in.push(
            data.iter()
                .step_by(2)
                .map(|x| *x as f64)
                .collect::<Vec<_>>(),
        );
        waves_in.push(
            data.iter()
                .skip(1)
                .step_by(2)
                .map(|x| *x as f64)
                .collect::<Vec<_>>(),
        );
    } else {
        waves_in.push(data.iter().map(|x| *x as f64).collect::<Vec<_>>());
    }
    if let Ok(x) = resampler.process(&waves_in) {
        if x.is_empty() {
            Vec::new()
        } else if x.len() == 2 {
            x[0].chunks(1)
                .zip(x[1].chunks(1))
                .flat_map(|(a, b)| a.into_iter().chain(b))
                .map(|x| *x as f32)
                .collect()
        } else {
            x[0].iter().map(|x| *x as f32).collect()
        }
    } else {
        Vec::new()
    }
}

#[cfg(all(feature = "use_dasp", feature = "use_samplerate"))]
compile_error!(
    "features `use_dasp` and `use_samplerate` are mutually exclusive; disable default features before selecting `use_samplerate`"
);

#[cfg(feature = "use_dasp")]
pub fn audio_resample(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use dasp::{interpolate::linear::Linear, signal, Signal};
    let n = data.len() / (channels as usize);
    let n = n * sample_rate as usize / sample_rate0 as usize;
    if channels == 2 {
        let mut source = signal::from_interleaved_samples_iter::<_, [_; 2]>(data.iter().cloned());
        let a = source.next();
        let b = source.next();
        let interp = Linear::new(a, b);
        let mut data = Vec::with_capacity(n << 1);
        for x in source
            .from_hz_to_hz(interp, sample_rate0 as _, sample_rate as _)
            .take(n)
        {
            data.push(x[0]);
            data.push(x[1]);
        }
        data
    } else {
        let mut source = signal::from_iter(data.iter().cloned());
        let a = source.next();
        let b = source.next();
        let interp = Linear::new(a, b);
        source
            .from_hz_to_hz(interp, sample_rate0 as _, sample_rate as _)
            .take(n)
            .collect()
    }
}

#[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
pub fn audio_resample(
    data: &[f32],
    sample_rate0: u32,
    sample_rate: u32,
    channels: u16,
) -> Vec<f32> {
    use samplerate::{convert, ConverterType};
    convert(
        sample_rate0 as _,
        sample_rate as _,
        channels as _,
        ConverterType::SincBestQuality,
        data,
    )
    .unwrap_or_default()
}
