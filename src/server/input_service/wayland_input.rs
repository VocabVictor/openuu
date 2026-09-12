use super::*;

#[inline]
#[cfg(target_os = "linux")]
pub fn wayland_use_uinput() -> bool {
    !crate::platform::is_x11() && crate::is_server()
}

#[inline]
#[cfg(target_os = "linux")]
pub fn wayland_use_rdp_input() -> bool {
    !crate::platform::is_x11() && !crate::is_server()
}

#[cfg(target_os = "linux")]
pub struct TemporaryMouseMoveHandle {
    thread_handle: Option<std::thread::JoinHandle<()>>,
    tx: Option<mpsc::Sender<(i32, i32)>>,
}

#[cfg(target_os = "linux")]
impl TemporaryMouseMoveHandle {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<(i32, i32)>();
        let thread_handle = std::thread::spawn(move || {
            log::debug!("TemporaryMouseMoveHandle thread started");
            for (x, y) in rx {
                ENIGO.lock().unwrap().mouse_move_to(x, y);
            }
            log::debug!("TemporaryMouseMoveHandle thread exiting");
        });
        TemporaryMouseMoveHandle {
            thread_handle: Some(thread_handle),
            tx: Some(tx),
        }
    }

    pub fn move_mouse_to(&self, x: i32, y: i32) {
        if let Some(tx) = &self.tx {
            let _ = tx.send((x, y));
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for TemporaryMouseMoveHandle {
    fn drop(&mut self) {
        log::debug!("Dropping TemporaryMouseMoveHandle");
        // Close the channel to signal the thread to exit.
        self.tx.take();
        // Wait for the thread to finish.
        if let Some(thread_handle) = self.thread_handle.take() {
            if let Err(e) = thread_handle.join() {
                log::error!("Error joining TemporaryMouseMoveHandle thread: {:?}", e);
            }
        }
    }
}
