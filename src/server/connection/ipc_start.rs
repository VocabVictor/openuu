use super::*;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
// IPC bootstrap summary:
// - Start CM when missing, then bridge bidirectional messages between this task and CM IPC.
pub(super) async fn start_ipc(
    mut rx_to_cm: mpsc::UnboundedReceiver<ipc::Data>,
    tx_from_cm: mpsc::UnboundedSender<ipc::Data>,
) -> ResultType<()> {
    use hbb_common::anyhow::anyhow;

    loop {
        if !crate::platform::is_prelogin() {
            break;
        }
        sleep(1.).await;
    }
    let mut stream = None;
    if let Ok(s) = crate::ipc::connect(1000, "_cm").await {
        stream = Some(s);
    }
    if stream.is_none() {
        let args = vec!["--cm"];
        let run_done;
        if crate::platform::is_root() {
            let mut res = Ok(None);
            for _ in 0..10 {
                #[cfg(not(any(target_os = "linux")))]
                {
                    log::debug!("Start cm");
                    res = crate::platform::run_as_user(args.clone());
                }
                #[cfg(target_os = "linux")]
                {
                    log::debug!("Start cm");
                    res = crate::platform::run_as_user(args.clone(), None, None::<(&str, &str)>);
                }
                if res.is_ok() {
                    break;
                }
                log::error!("Failed to run cm: {res:?}");
                sleep(1.).await;
            }
            if let Some(task) = res? {
                super::super::add_child(task);
            }
            run_done = true;
        } else {
            run_done = false;
        }
        if !run_done {
            log::debug!("Start cm");
            super::super::add_child(crate::run_me(args)?);
        }
        for _ in 0..20 {
            sleep(0.3).await;
            if let Ok(s) = crate::ipc::connect(1000, "_cm").await {
                stream = Some(s);
                break;
            }
        }
    }
    if stream.is_none() {
        bail!("Failed to connect to connection manager");
    }

    let mut stream = stream.ok_or(anyhow!("none stream"))?;
    loop {
        tokio::select! {
            res = stream.next() => {
                match res {
                    Err(err) => {
                        return Err(err.into());
                    }
                    Ok(Some(data)) => {
                        match data {
                            ipc::Data::ClickTime(_)=> {
                                let ct = CLICK_TIME.load(Ordering::SeqCst);
                                let data = ipc::Data::ClickTime(ct);
                                stream.send(&data).await?;
                            }
                            // FileBlockFromCM: data is always sent separately via send_raw.
                            // The data field has #[serde(skip)], so it's empty after deserialization.
                            // Read the raw data bytes following this message.
                            //
                            // Note: Empty data (for empty files) is correctly handled. BytesCodec with
                            // raw=false adds a length prefix, so next_raw() returns empty BytesMut for
                            // zero-length frames. This mirrors the WriteBlock pattern below.
                            ipc::Data::FileBlockFromCM { id, file_num, data: _, compressed, conn_id } => {
                                let raw_data = stream.next_raw().await?;
                                tx_from_cm.send(ipc::Data::FileBlockFromCM {
                                    id,
                                    file_num,
                                    data: raw_data.into(),
                                    compressed,
                                    conn_id,
                                })?;
                            }
                            _ => {
                                tx_from_cm.send(data)?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            res = rx_to_cm.recv() => {
                match res {
                    Some(data) => {
                        if let Data::FS(ipc::FS::WriteBlock{id,
                            file_num,
                            data,
                            compressed}) = data {
                                stream.send(&Data::FS(ipc::FS::WriteBlock{id, file_num, data: Bytes::new(), compressed})).await?;
                                stream.send_raw(data).await?;
                        } else {
                            stream.send(&data).await?;
                        }
                    }
                    None => {
                        bail!("expected");
                    }
                }
            }
        }
    }
}

// in case screen is sleep and blank, here to activate it
pub(super) fn try_activate_screen() {
    #[cfg(windows)]
    std::thread::spawn(|| {
        mouse_move_relative(-6, -6);
        std::thread::sleep(std::time::Duration::from_millis(30));
        mouse_move_relative(6, 6);
    });
}
