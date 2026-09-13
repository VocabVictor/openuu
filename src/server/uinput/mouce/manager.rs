use super::*;

pub struct UInputMouseManager {
    pub(super) uinput_file: File,
}

impl UInputMouseManager {
    pub fn new(rng_x: (i32, i32), rng_y: (i32, i32)) -> Result<Self> {
        let manager = UInputMouseManager {
            uinput_file: File::options()
                .write(true)
                .custom_flags(O_NONBLOCK)
                .open("/dev/uinput")?,
        };
        let fd = manager.uinput_file.as_raw_fd();
        unsafe {
            // For press events (also needed for mouse movement)
            ioctl(fd, UI_SET_EVBIT, EV_KEY);
            ioctl(fd, UI_SET_KEYBIT, BTN_LEFT);
            ioctl(fd, UI_SET_KEYBIT, BTN_RIGHT);
            ioctl(fd, UI_SET_KEYBIT, BTN_MIDDLE);

            // For mouse movement
            ioctl(fd, UI_SET_EVBIT, EV_ABS);
            ioctl(fd, UI_SET_ABSBIT, ABS_X);
            ioctl(
                fd,
                UI_ABS_SETUP,
                &UinputAbsSetup {
                    code: ABS_X as _,
                    absinfo: InputAbsinfo {
                        value: 0,
                        minimum: rng_x.0,
                        maximum: rng_x.1,
                        fuzz: 0,
                        flat: 0,
                        resolution: 0,
                    },
                },
            );
            ioctl(fd, UI_SET_ABSBIT, ABS_Y);
            ioctl(
                fd,
                UI_ABS_SETUP,
                &UinputAbsSetup {
                    code: ABS_Y as _,
                    absinfo: InputAbsinfo {
                        value: 0,
                        minimum: rng_y.0,
                        maximum: rng_y.1,
                        fuzz: 0,
                        flat: 0,
                        resolution: 0,
                    },
                },
            );

            ioctl(fd, UI_SET_EVBIT, EV_REL);
            ioctl(fd, UI_SET_RELBIT, REL_X);
            ioctl(fd, UI_SET_RELBIT, REL_Y);
            ioctl(fd, UI_SET_RELBIT, REL_WHEEL);
            ioctl(fd, UI_SET_RELBIT, REL_HWHEEL);
        }

        let mut usetup = UInputSetup {
            id: InputId {
                bustype: BUS_USB,
                // Random vendor and product
                vendor: 0x2222,
                product: 0x3333,
                version: 0,
            },
            name: [0; UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        };

        let mut device_bytes: Vec<c_char> = "mouce-library-fake-mouse"
            .chars()
            .map(|ch| ch as c_char)
            .collect();

        // Fill the rest of the name buffer with empty chars
        for _ in 0..UINPUT_MAX_NAME_SIZE - device_bytes.len() {
            device_bytes.push('\0' as c_char);
        }

        usetup.name.copy_from_slice(&device_bytes);

        unsafe {
            ioctl(fd, UI_DEV_SETUP, &usetup);
            ioctl(fd, UI_DEV_CREATE);
        }

        // On UI_DEV_CREATE the kernel will create the device node for this
        // device. We are inserting a pause here so that userspace has time
        // to detect, initialize the new device, and can start listening to
        // the event, otherwise it will not notice the event we are about to send.
        thread::sleep(Duration::from_millis(300));

        Ok(manager)
    }

    /// Write the given event to the uinput file
    pub(super) fn emit(&self, r#type: c_int, code: c_int, value: c_int) -> Result<()> {
        let mut event = InputEvent {
            time: TimeVal {
                tv_sec: 0,
                tv_usec: 0,
            },
            r#type: r#type as c_ushort,
            code: code as c_ushort,
            value,
        };
        let fd = self.uinput_file.as_raw_fd();

        unsafe {
            let count = size_of::<InputEvent>();
            let written_bytes = write(fd, &mut event, count);
            if written_bytes == -1 || written_bytes != count as c_long {
                return Err(Error::new(
                    ErrorKind::Other,
                    format!("failed while trying to write to a file"),
                ));
            }
        }

        Ok(())
    }

    /// Syncronize the device
    pub(super) fn syncronize(&self) -> Result<()> {
        self.emit(EV_SYN, SYN_REPORT, 0)?;
        // Give uinput some time to update the mouse location,
        // otherwise it fails to move the mouse on release mode
        // A delay of 1 milliseconds seems to be enough for it
        thread::sleep(Duration::from_millis(1));
        Ok(())
    }

    /// Move the mouse relative to the current position
    pub(super) fn move_relative_(&self, x: i32, y: i32) -> Result<()> {
        // uinput does not move the mouse in pixels but uses `units`. I couldn't
        // find information regarding to this uinput `unit`, but according to
        // my findings 1 unit corresponds to exactly 2 pixels.
        //
        // To achieve the expected behavior; divide the parameters by 2
        //
        // This seems like there is a bug in this crate, but the
        // behavior is the same on other projects that make use of
        // uinput. e.g. `ydotool`. When you try to move your mouse,
        // it will move 2x further pixels
        self.emit(EV_REL, REL_X as c_int, (x as f32 / 2.).ceil() as c_int)?;
        self.emit(EV_REL, REL_Y as c_int, (y as f32 / 2.).ceil() as c_int)?;
        self.syncronize()
    }

    pub(super) fn map_btn(button: &MouseButton) -> c_int {
        match button {
            MouseButton::Left => BTN_LEFT,
            MouseButton::Right => BTN_RIGHT,
            MouseButton::Middle => BTN_MIDDLE,
            MouseButton::Side => BTN_SIDE,
            MouseButton::Extra => BTN_EXTRA,
            MouseButton::Forward => BTN_FORWARD,
            MouseButton::Back => BTN_BACK,
            MouseButton::Task => BTN_TASK,
        }
    }

    pub fn move_to(&self, x: usize, y: usize) -> Result<()> {
        // // For some reason, absolute mouse move events are not working on uinput
        // // (as I understand those events are intended for touch events)
        // //
        // // As a work around solution; first set the mouse to top left, then
        // // call relative move function to simulate an absolute move event
        //self.move_relative(i32::MIN, i32::MIN)?;
        //self.move_relative(x as i32, y as i32)

        self.emit(EV_ABS, ABS_X as c_int, x as c_int)?;
        self.emit(EV_ABS, ABS_Y as c_int, y as c_int)?;
        self.syncronize()
    }

    pub fn move_relative(&self, x_offset: i32, y_offset: i32) -> Result<()> {
        self.move_relative_(x_offset, y_offset)
    }

    pub fn press_button(&self, button: &MouseButton) -> Result<()> {
        self.emit(EV_KEY, Self::map_btn(button), 1)?;
        self.syncronize()
    }

    pub fn release_button(&self, button: &MouseButton) -> Result<()> {
        self.emit(EV_KEY, Self::map_btn(button), 0)?;
        self.syncronize()
    }

    pub fn click_button(&self, button: &MouseButton) -> Result<()> {
        self.press_button(button)?;
        self.release_button(button)
    }

    pub fn scroll_wheel(&self, direction: &ScrollDirection) -> Result<()> {
        let (code, scroll_value) = match direction {
            ScrollDirection::Up => (REL_WHEEL, 1),
            ScrollDirection::Down => (REL_WHEEL, -1),
            ScrollDirection::Left => (REL_HWHEEL, -1),
            ScrollDirection::Right => (REL_HWHEEL, 1),
        };
        self.emit(EV_REL, code as c_int, scroll_value)?;
        self.syncronize()
    }
}

impl Drop for UInputMouseManager {
    fn drop(&mut self) {
        let fd = self.uinput_file.as_raw_fd();
        unsafe {
            // Destroy the device, the file is closed automatically by the File module
            ioctl(fd, UI_DEV_DESTROY as c_ulong);
        }
    }
}
