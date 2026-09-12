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
    #[cfg(test)]
    mod tests;
    #[cfg(feature = "screencapturekit")]
    lazy_static::lazy_static! {
        static ref HOST_SCREEN_CAPTURE_KIT: Result<Host, cpal::HostUnavailable> = cpal::host_from_id(cpal::HostId::ScreenCaptureKit);
    }

}
