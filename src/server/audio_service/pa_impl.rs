use super::*;

/// Reading the sample bytes back as `f32` needs a 4-byte aligned pointer.
/// Returns an aligned copy only when `data` is not already aligned; `None`
/// means the caller can reinterpret `data` where it is, with no copy.
fn align_to_32_if_needed(data: &[u8]) -> Option<hbb_common::mem::AlignedU8Vec> {
    if (data.as_ptr() as usize & 3) == 0 {
        return None;
    }
    let mut buf = hbb_common::mem::aligned_u8_vec(data.len(), 4);
    buf.extend_from_slice(data);
    Some(buf)
}

#[tokio::main(flavor = "current_thread")]
pub async fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    hbb_common::sleep(0.1).await; // one moment to wait for _pa ipc
    RESTARTING.store(false, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    let mut stream = crate::ipc::connect(1000, "_pa").await?;
    let mut encoder = AudioEncoder::new(Encoder::new(
        crate::platform::PA_SAMPLE_RATE,
        Stereo,
        LowDelay,
    )?);
    #[cfg(target_os = "linux")]
    allow_err!(
        stream
            .send(&crate::ipc::Data::Config((
                "audio-input".to_owned(),
                Some(super::get_audio_input())
            )))
            .await
    );
    #[cfg(target_os = "linux")]
    let zero_audio_frame: Vec<f32> = vec![0.; AUDIO_DATA_SIZE_U8 / 4];
    #[cfg(target_os = "android")]
    let mut android_data = vec![];
    while sp.ok() && !RESTARTING.load(Ordering::SeqCst) {
        sp.snapshot(|sps| {
            sps.send(create_format_msg(crate::platform::PA_SAMPLE_RATE, 2));
            Ok(())
        })?;

        #[cfg(target_os = "linux")]
        if let Ok(data) = stream.next_raw().await {
            if data.len() == 0 {
                send_f32(&zero_audio_frame, &mut encoder, &sp);
                continue;
            }

            if data.len() != AUDIO_DATA_SIZE_U8 {
                continue;
            }

            let data: Vec<u8> = data.into();
            let aligned = align_to_32_if_needed(&data);
            let bytes = aligned.as_deref().unwrap_or(&data[..]);
            // SAFETY: `bytes` is 4-byte aligned (either checked above or freshly
            // allocated with align 4), and only whole f32s are read from it.
            let data = unsafe {
                std::slice::from_raw_parts::<f32>(bytes.as_ptr() as _, bytes.len() / 4)
            };
            send_f32(data, &mut encoder, &sp);
        }

        #[cfg(target_os = "android")]
        if scrap::android::ffi::get_audio_raw(&mut android_data, &mut vec![]).is_some() {
            // Keep `android_data` as the reusable receive buffer: overwriting it with
            // an exact-capacity aligned buffer only made the next `get_audio_raw`
            // reallocate it, which dropped the alignment again.
            let aligned = align_to_32_if_needed(&android_data);
            let bytes = aligned.as_deref().unwrap_or(&android_data[..]);
            // SAFETY: `bytes` is 4-byte aligned (either checked above or freshly
            // allocated with align 4), and only whole f32s are read from it.
            let data = unsafe {
                std::slice::from_raw_parts::<f32>(bytes.as_ptr() as _, bytes.len() / 4)
            };
            send_f32(data, &mut encoder, &sp);
        } else {
            hbb_common::sleep(0.1).await;
        }
    }
    Ok(())
}
