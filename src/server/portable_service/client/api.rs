use super::*;

pub(super) fn get_cursor_info_(shmem: &mut SharedMemory, pci: PCURSORINFO) -> BOOL {
    unsafe {
        let shmem_addr_para = shmem.as_ptr().add(ADDR_CURSOR_PARA);
        if utils::counter_ready(shmem.as_ptr().add(ADDR_CURSOR_COUNTER)) {
            std::ptr::copy_nonoverlapping(shmem_addr_para, pci as _, size_of::<CURSORINFO>());
            return TRUE;
        }
        FALSE
    }
}

pub(super) fn handle_mouse_(
    evt: &MouseEvent,
    conn: i32,
    username: String,
    argb: u32,
    simulate: bool,
    show_cursor: bool,
) -> ResultType<()> {
    let mut v = vec![];
    evt.write_to_vec(&mut v)?;
    ipc_send(Data::DataPortableService(DataPortableService::Mouse((
        v,
        conn,
        username,
        argb,
        simulate,
        show_cursor,
    ))))
}

pub(super) fn handle_pointer_(evt: &PointerDeviceEvent, conn: i32) -> ResultType<()> {
    let mut v = vec![];
    evt.write_to_vec(&mut v)?;
    ipc_send(Data::DataPortableService(DataPortableService::Pointer((
        v, conn,
    ))))
}

pub(super) fn handle_key_(evt: &KeyEvent) -> ResultType<()> {
    let mut v = vec![];
    evt.write_to_vec(&mut v)?;
    ipc_send(Data::DataPortableService(DataPortableService::Key(v)))
}

pub fn create_capturer(
    current_display: usize,
    display: scrap::Display,
    portable_service_running: bool,
) -> ResultType<Box<dyn TraitCapturer>> {
    if portable_service_running != RUNNING.lock().unwrap().clone() {
        log::info!("portable service status mismatch");
    }
    if portable_service_running && display.is_primary() {
        log::info!("Create shared memory capturer");
        return Ok(Box::new(CapturerPortable::new(current_display)));
    } else {
        log::debug!("Create capturer dxgi|gdi");
        return Ok(Box::new(
            Capturer::new(display).with_context(|| "Failed to create capturer")?,
        ));
    }
}

pub fn get_cursor_info(pci: PCURSORINFO) -> BOOL {
    if RUNNING.lock().unwrap().clone() {
        let mut option = SHMEM.lock().unwrap();
        option
            .as_mut()
            .map_or(FALSE, |sheme| get_cursor_info_(sheme, pci))
    } else {
        unsafe { winuser::GetCursorInfo(pci) }
    }
}

pub fn handle_mouse(
    evt: &MouseEvent,
    conn: i32,
    username: String,
    argb: u32,
    simulate: bool,
    show_cursor: bool,
) {
    if RUNNING.lock().unwrap().clone() {
        crate::input_service::update_latest_input_cursor_time(conn);
        handle_mouse_(evt, conn, username, argb, simulate, show_cursor).ok();
    } else {
        crate::input_service::handle_mouse_(evt, conn, username, argb, simulate, show_cursor);
    }
}

pub fn handle_pointer(evt: &PointerDeviceEvent, conn: i32) {
    if RUNNING.lock().unwrap().clone() {
        crate::input_service::update_latest_input_cursor_time(conn);
        handle_pointer_(evt, conn).ok();
    } else {
        crate::input_service::handle_pointer_(evt, conn);
    }
}

pub fn handle_key(evt: &KeyEvent) {
    if RUNNING.lock().unwrap().clone() {
        handle_key_(evt).ok();
    } else {
        crate::input_service::handle_key_(evt);
    }
}

pub fn running() -> bool {
    RUNNING.lock().unwrap().clone()
}
