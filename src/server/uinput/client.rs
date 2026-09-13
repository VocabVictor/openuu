use super::*;

pub struct UInputKeyboard {
    conn: Connection,
    rt: Runtime,
}

impl UInputKeyboard {
    pub async fn new() -> ResultType<Self> {
        let conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_KEYBOARD).await?;
        let rt = Runtime::new()?;
        Ok(Self { conn, rt })
    }

    fn send(&mut self, data: Data) -> ResultType<()> {
        self.rt.block_on(self.conn.send(&data))
    }

    fn send_get_key_state(&mut self, data: Data) -> ResultType<bool> {
        self.rt.block_on(self.conn.send(&data))?;

        match self
            .rt
            .block_on(self.conn.next_timeout(IPC_REQUEST_TIMEOUT))
        {
            Ok(Some(Data::KeyboardResponse(ipc::DataKeyboardResponse::GetKeyState(state)))) => {
                Ok(state)
            }
            Ok(Some(resp)) => {
                // FATAL error!!!
                bail!(
                    "FATAL error, wait keyboard result other response: {:?}",
                    &resp
                );
            }
            Ok(None) => {
                // FATAL error!!!
                // Maybe wait later
                bail!("FATAL error, wait keyboard result, receive None",);
            }
            Err(e) => {
                // FATAL error!!!
                bail!(
                    "FATAL error, wait keyboard result timeout {}, {}",
                    &e,
                    IPC_REQUEST_TIMEOUT
                );
            }
        }
    }
}

impl KeyboardControllable for UInputKeyboard {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_key_state(&mut self, key: Key) -> bool {
        match self.send_get_key_state(Data::Keyboard(DataKeyboard::GetKeyState(key))) {
            Ok(state) => state,
            Err(e) => {
                // unreachable!()
                log::error!("Failed to get key state {}", &e);
                false
            }
        }
    }

    fn key_sequence(&mut self, sequence: &str) {
        // Sequence events are normally handled in the --server process before reaching here.
        // Forward via IPC as a fallback — input_text_wayland can still handle ASCII chars
        // via keysym/uinput, though non-ASCII will be skipped (no clipboard in --service).
        log::debug!(
            "UInputKeyboard::key_sequence called (len={})",
            sequence.len()
        );
        allow_err!(self.send(Data::Keyboard(DataKeyboard::Sequence(sequence.to_string()))));
    }

    // TODO: handle error???
    fn key_down(&mut self, key: Key) -> enigo::ResultType {
        allow_err!(self.send(Data::Keyboard(DataKeyboard::KeyDown(key))));
        Ok(())
    }
    fn key_up(&mut self, key: Key) {
        allow_err!(self.send(Data::Keyboard(DataKeyboard::KeyUp(key))));
    }
    fn key_click(&mut self, key: Key) {
        allow_err!(self.send(Data::Keyboard(DataKeyboard::KeyClick(key))));
    }
}

pub struct UInputMouse {
    conn: Connection,
    rt: Runtime,
}

impl UInputMouse {
    pub async fn new() -> ResultType<Self> {
        let conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_MOUSE).await?;
        let rt = Runtime::new()?;
        Ok(Self { conn, rt })
    }

    fn send(&mut self, data: Data) -> ResultType<()> {
        self.rt.block_on(self.conn.send(&data))
    }

    pub fn send_refresh(&mut self) -> ResultType<()> {
        self.rt
            .block_on(self.conn.send(&Data::Mouse(DataMouse::Refresh)))?;
        // Wait for the service to confirm it recreated the device, so a
        // failed refresh is distinguishable from a good one.
        match self.rt.block_on(self.conn.next_timeout(IPC_REQUEST_TIMEOUT)) {
            Ok(Some(Data::Empty)) => Ok(()),
            Ok(Some(resp)) => bail!("unexpected uinput mouse refresh response: {:?}", &resp),
            Ok(None) => bail!("uinput mouse refresh failed, connection closed"),
            Err(e) => bail!("uinput mouse refresh timeout {}, {}", IPC_REQUEST_TIMEOUT, e),
        }
    }
}

impl MouseControllable for UInputMouse {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn mouse_move_to(&mut self, x: i32, y: i32) {
        allow_err!(self.send(Data::Mouse(DataMouse::MoveTo(x, y))));
    }
    fn mouse_move_relative(&mut self, x: i32, y: i32) {
        allow_err!(self.send(Data::Mouse(DataMouse::MoveRelative(x, y))));
    }
    // TODO: handle error???
    fn mouse_down(&mut self, button: MouseButton) -> enigo::ResultType {
        allow_err!(self.send(Data::Mouse(DataMouse::Down(button))));
        Ok(())
    }
    fn mouse_up(&mut self, button: MouseButton) {
        allow_err!(self.send(Data::Mouse(DataMouse::Up(button))));
    }
    fn mouse_click(&mut self, button: MouseButton) {
        allow_err!(self.send(Data::Mouse(DataMouse::Click(button))));
    }
    fn mouse_scroll_x(&mut self, length: i32) {
        allow_err!(self.send(Data::Mouse(DataMouse::ScrollX(length))));
    }
    fn mouse_scroll_y(&mut self, length: i32) {
        allow_err!(self.send(Data::Mouse(DataMouse::ScrollY(length))));
    }
}

pub async fn set_resolution(minx: i32, maxx: i32, miny: i32, maxy: i32) -> ResultType<()> {
    let mut conn = ipc::connect(IPC_CONN_TIMEOUT, IPC_POSTFIX_CONTROL).await?;
    conn.send(&Data::Control(ipc::DataControl::Resolution {
        minx,
        maxx,
        miny,
        maxy,
    }))
    .await?;
    let _ = conn.next().await?;
    Ok(())
}
