use super::*;

/// Start uinput service.
pub(super) async fn start_service<F: FnOnce(ipc::Connection) + Copy>(postfix: &str, handler: F) {
    match new_listener(postfix).await {
        Ok(mut incoming) => {
            while let Some(result) = incoming.next().await {
                match result {
                    Ok(stream) => {
                        #[cfg(target_os = "linux")]
                        if !authorize_uinput_peer(postfix, &stream) {
                            continue;
                        }
                        log::debug!("Got new connection of uinput ipc {}", postfix);
                        handler(Connection::new(stream));
                    }
                    Err(err) => {
                        log::error!("Couldn't get uinput mouse client: {:?}", err);
                    }
                }
            }
        }
        Err(err) => {
            log::error!("Failed to start uinput mouse ipc service: {}", err);
        }
    }
}

/// Start uinput keyboard service.
#[tokio::main(flavor = "current_thread")]
pub async fn start_service_keyboard() {
    log::info!("start uinput keyboard service");
    start_service(IPC_POSTFIX_KEYBOARD, spawn_keyboard_handler).await;
}

/// Start uinput mouse service.
#[tokio::main(flavor = "current_thread")]
pub async fn start_service_mouse() {
    log::info!("start uinput mouse service");
    start_service(IPC_POSTFIX_MOUSE, spawn_mouse_handler).await;
}

/// Start uinput mouse service.
#[tokio::main(flavor = "current_thread")]
pub async fn start_service_control() {
    log::info!("start uinput control service");
    start_service(IPC_POSTFIX_CONTROL, spawn_controller_handler).await;
}

pub fn stop_service_keyboard() {
    log::info!("stop uinput keyboard service");
}
pub fn stop_service_mouse() {
    log::info!("stop uinput mouse service");
}
pub fn stop_service_control() {
    log::info!("stop uinput control service");
}
