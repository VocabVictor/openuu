use super::*;

#[derive(Default)]
pub struct State {
    pub(super) stream: Option<ActiveCaptureStream>,
}

pub(super) struct ActiveCaptureStream {
    pub(super) stream: Option<Box<dyn StreamTrait>>,
    pub(super) format: Arc<Message>,
    pub(super) _encoder_worker: audio_capture_queue::CaptureEncoderWorker,
    pub(super) errors: CaptureErrorHandler,
}

impl Drop for ActiveCaptureStream {
    fn drop(&mut self) {
        self.stream.take();
    }
}

impl super::super::service::Reset for State {
    fn reset(&mut self) {
        self.stream.take();
    }
}

pub(super) fn run_restart(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
    state.reset();
    sp.snapshot(|_sps: ServiceSwap<_>| Ok(()))?;
    match &state.stream {
        None => {
            state.stream = Some(play(&sp)?);
        }
        _ => {}
    }
    if let Some(stream) = &state.stream {
        sp.send_shared(stream.format.clone());
        #[cfg(target_os = "macos")]
        log::info!("Audio capture stream recreated; replacement format sent");
    }
    RESTARTING.store(false, Ordering::SeqCst);
    Ok(())
}

pub(super) fn run_serv_snapshot(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
    sp.snapshot(|sps| {
        match &state.stream {
            None => {
                state.stream = Some(play(&sp)?);
            }
            _ => {}
        }
        if let Some(stream) = &state.stream {
            sps.send_shared(stream.format.clone());
        }
        Ok(())
    })?;
    Ok(())
}

pub fn run(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
    if let Some(stream) = &state.stream {
        if stream.errors.needs_restart() {
            // Recreate on the service thread, outside the capture callbacks.
            log::warn!("Recreating audio capture stream after an error");
            super::super::restart();
        }
    }
    if !RESTARTING.load(Ordering::SeqCst) {
        run_serv_snapshot(sp, state)
    } else {
        run_restart(sp, state)
    }
}
