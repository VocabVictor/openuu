use super::*;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tokio::main(flavor = "current_thread")]
pub async fn start_ipc<T: InvokeUiCM>(cm: ConnectionManager<T>) {
    #[cfg(target_os = "windows")]
    {
        let enabled = crate::Connection::is_permission_enabled_locally(OPTION_ENABLE_FILE_TRANSFER);
        let mut lock = crate::ui_interface::IS_FILE_TRANSFER_ENABLED
            .lock()
            .unwrap();
        ContextSend::enable(enabled);
        *lock = Some(enabled);
    }
    match ipc::new_listener("_cm").await {
        Ok(mut incoming) => {
            while let Some(result) = incoming.next().await {
                match result {
                    Ok(stream) => {
                        log::debug!("Got new connection");
                        tokio::spawn(IpcTaskRunner::<T>::ipc_task(
                            Connection::new(stream),
                            cm.clone(),
                        ));
                    }
                    Err(err) => {
                        log::error!("Couldn't get cm client: {:?}", err);
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start cm ipc server: {}", err);
        }
    }
    quit_cm();
}

#[cfg(target_os = "android")]
#[tokio::main(flavor = "current_thread")]
pub async fn start_listen<T: InvokeUiCM>(
    cm: ConnectionManager<T>,
    mut rx: mpsc::UnboundedReceiver<Data>,
    tx: mpsc::UnboundedSender<Data>,
) {
    let mut current_id = 0;
    let mut write_jobs: Vec<fs::TransferJob> = Vec::new();
    loop {
        match rx.recv().await {
            Some(Data::Login {
                id,
                is_file_transfer,
                is_view_camera,
                is_terminal,
                port_forward,
                peer_id,
                name,
                avatar,
                authorized,
                keyboard,
                clipboard,
                audio,
                file,
                restart,
                recording,
                block_input,
                privacy_mode,
                from_switch,
                ..
            }) => {
                current_id = id;
                cm.add_connection(
                    id,
                    is_file_transfer,
                    is_view_camera,
                    is_terminal,
                    port_forward,
                    peer_id,
                    name,
                    avatar,
                    authorized,
                    keyboard,
                    clipboard,
                    audio,
                    file,
                    restart,
                    recording,
                    block_input,
                    privacy_mode,
                    from_switch,
                    tx.clone(),
                );
            }
            Some(Data::ChatMessage { text }) => {
                cm.new_message(current_id, text);
            }
            Some(Data::FS(fs)) => {
                // Android doesn't need CM-side file reading (no need_validate_file_read_access)
                let mut read_jobs_placeholder: Vec<fs::TransferJob> = Vec::new();
                handle_fs(
                    fs,
                    &mut write_jobs,
                    &mut read_jobs_placeholder,
                    &tx,
                    None,
                    current_id,
                )
                .await;
            }
            Some(Data::Close) => {
                break;
            }
            Some(Data::StartVoiceCall) => {
                cm.voice_call_started(current_id);
            }
            Some(Data::VoiceCallIncoming) => {
                cm.voice_call_incoming(current_id);
            }
            Some(Data::CloseVoiceCall(reason)) => {
                cm.voice_call_closed(current_id, reason.as_str());
            }
            None => {
                break;
            }
            _ => {}
        }
    }
    cm.remove_connection(current_id, true);
}
