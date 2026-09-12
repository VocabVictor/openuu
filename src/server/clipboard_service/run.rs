use super::*;

#[cfg(not(target_os = "android"))]
pub(super) fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    #[cfg(all(feature = "unix-file-copy-paste", target_os = "linux"))]
    let _fuse_call_on_ret = {
        if sp.name() == FILE_NAME {
            Some(init_fuse_context(false).map(|_| crate::SimpleCallOnReturn {
                b: true,
                f: Box::new(|| {
                    uninit_fuse_context(false);
                }),
            }))
        } else {
            None
        }
    };

    let (tx_cb_result, rx_cb_result) = channel();
    let ctx = Some(ClipboardContext::new().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?);
    clipboard_listener::subscribe(sp.name(), tx_cb_result)?;
    let mut handler = Handler {
        ctx,
        #[cfg(target_os = "windows")]
        stream: None,
        #[cfg(target_os = "windows")]
        rt: None,
    };

    while sp.ok() {
        match rx_cb_result.recv_timeout(Duration::from_millis(INTERVAL)) {
            Ok(CallbackResult::Next) => {
                #[cfg(feature = "unix-file-copy-paste")]
                if sp.name() == FILE_NAME {
                    handler.check_clipboard_file();
                    continue;
                }
                if let Some(msg) = handler.get_clipboard_msg() {
                    sp.send(msg);
                }
            }
            Ok(CallbackResult::Stop) => {
                log::debug!("Clipboard listener stopped");
                break;
            }
            Ok(CallbackResult::StopWithError(err)) => {
                bail!("Clipboard listener stopped with error: {}", err);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                log::error!("Clipboard listener disconnected");
                break;
            }
        }
    }

    clipboard_listener::unsubscribe(&sp.name());

    Ok(())
}

#[cfg(target_os = "android")]
pub(super) fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    CLIPBOARD_SERVICE_OK.store(sp.ok(), Ordering::SeqCst);
    while sp.ok() {
        if let Some(msg) = crate::clipboard::get_clipboards_msg(false) {
            sp.send(msg);
        }
        std::thread::sleep(Duration::from_millis(INTERVAL));
    }
    CLIPBOARD_SERVICE_OK.store(false, Ordering::SeqCst);
    Ok(())
}
