use clipboard_master::{CallbackResult, ClipboardHandler, Master, Shutdown};
use hbb_common::{bail, log, ResultType};
use std::{
    collections::HashMap,
    io,
    sync::mpsc::{channel, Sender},
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

lazy_static::lazy_static! {
    pub static ref CLIPBOARD_LISTENER: Arc<Mutex<ClipboardListener>> = Default::default();
}

struct Handler {
    subscribers: Arc<Mutex<HashMap<String, Sender<CallbackResult>>>>,
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        let sub_lock = self.subscribers.lock().unwrap();
        for tx in sub_lock.values() {
            tx.send(CallbackResult::Next).ok();
        }
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
        let msg = format!("Clipboard listener error: {}", error);
        let sub_lock = self.subscribers.lock().unwrap();
        for tx in sub_lock.values() {
            tx.send(CallbackResult::StopWithError(io::Error::new(
                io::ErrorKind::Other,
                msg.clone(),
            )))
            .ok();
        }
        CallbackResult::Next
    }
}

#[derive(Default)]
pub struct ClipboardListener {
    subscribers: Arc<Mutex<HashMap<String, Sender<CallbackResult>>>>,
    handle: Option<(Shutdown, JoinHandle<()>)>,
}

pub fn subscribe(name: String, tx: Sender<CallbackResult>) -> ResultType<()> {
    log::info!("Subscribe clipboard listener: {}", &name);
    let mut listener_lock = CLIPBOARD_LISTENER.lock().unwrap();
    listener_lock
        .subscribers
        .lock()
        .unwrap()
        .insert(name.clone(), tx);

    cleanup_stale_listener(&mut listener_lock);
    if listener_lock.handle.is_none() {
        log::info!("Start clipboard listener thread");
        let handler = Handler {
            subscribers: listener_lock.subscribers.clone(),
        };
        let (tx_start_res, rx_start_res) = channel();
        let h = start_clipboard_master_thread(handler, tx_start_res);
        let shutdown = match rx_start_res.recv() {
            Ok((Some(s), _)) => s,
            Ok((None, err)) => {
                bail!(err);
            }

            Err(e) => {
                bail!("Failed to create clipboard listener: {}", e);
            }
        };
        listener_lock.handle = Some((shutdown, h));
        log::info!("Clipboard listener thread started");
    }

    log::info!("Clipboard listener subscribed: {}", name);
    Ok(())
}

fn cleanup_stale_listener(listener: &mut ClipboardListener) {
    if !listener
        .handle
        .as_ref()
        .map(|(_, h)| h.is_finished())
        .unwrap_or(false)
    {
        return;
    }
    if let Some((shutdown, h)) = listener.handle.take() {
        log::warn!("Cleaning up stale clipboard listener handle");
        if let Err(e) = h.join() {
            log::error!("Clipboard listener thread panicked during stale cleanup: {:?}", e);
        }
        drop(shutdown);
    }
}

pub fn unsubscribe(name: &str) {
    log::info!("Unsubscribe clipboard listener: {}", name);
    let mut listener_lock = CLIPBOARD_LISTENER.lock().unwrap();
    let is_empty = {
        let mut sub_lock = listener_lock.subscribers.lock().unwrap();
        if let Some(tx) = sub_lock.remove(name) {
            tx.send(CallbackResult::Stop).ok();
        }
        sub_lock.is_empty()
    };
    if is_empty {
        if let Some((shutdown, h)) = listener_lock.handle.take() {
            log::info!("Stop clipboard listener thread");
            shutdown.signal();
            h.join().ok();
            log::info!("Clipboard listener thread stopped");
        }
    }
    log::info!("Clipboard listener unsubscribed: {}", name);
}

fn start_clipboard_master_thread(
    handler: impl ClipboardHandler + Send + 'static,
    tx_start_res: Sender<(Option<Shutdown>, String)>,
) -> JoinHandle<()> {
    // https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage#:~:text=The%20window%20must%20belong%20to%20the%20current%20thread.
    let h = std::thread::spawn(move || match Master::new(handler) {
        Ok(mut master) => {
            tx_start_res
                .send((Some(master.shutdown_channel()), "".to_owned()))
                .ok();
            log::debug!("Clipboard listener started");
            if let Err(err) = master.run() {
                log::error!("Failed to run clipboard listener: {}", err);
            } else {
                log::debug!("Clipboard listener stopped");
            }
        }
        Err(err) => {
            tx_start_res
                .send((
                    None,
                    format!("Failed to create clipboard listener: {}", err),
                ))
                .ok();
        }
    });
    h
}
