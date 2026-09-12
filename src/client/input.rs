use super::*;

/// Handle latency test.
///
/// # Arguments
///
/// * `t` - The latency test message.
/// * `peer` - The peer.
pub async fn handle_test_delay(t: TestDelay, peer: &mut Stream) {
    if !t.from_client {
        let mut msg_out = Message::new();
        msg_out.set_test_delay(t);
        allow_err!(peer.send(&msg_out).await);
    }
}

/// Send mouse data.
///
/// # Arguments
///
/// * `mask` - Mouse event.
///     * mask = buttons << 3 | type
///     * type, 1: down, 2: up, 3: wheel, 4: trackpad
///     * buttons, 1: left, 2: right, 4: middle
/// * `x` - X coordinate.
/// * `y` - Y coordinate.
/// * `alt` - Whether the alt key is pressed.
/// * `ctrl` - Whether the ctrl key is pressed.
/// * `shift` - Whether the shift key is pressed.
/// * `command` - Whether the command key is pressed.
/// * `interface` - The interface for sending data.
#[inline]
pub fn send_mouse(
    mask: i32,
    x: i32,
    y: i32,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
    interface: &impl Interface,
) {
    let mut msg_out = Message::new();
    let mut mouse_event = MouseEvent {
        mask,
        x,
        y,
        ..Default::default()
    };
    if alt {
        mouse_event.modifiers.push(ControlKey::Alt.into());
    }
    if shift {
        mouse_event.modifiers.push(ControlKey::Shift.into());
    }
    if ctrl {
        mouse_event.modifiers.push(ControlKey::Control.into());
    }
    if command {
        mouse_event.modifiers.push(ControlKey::Meta.into());
    }
    interface.swap_modifier_mouse(&mut mouse_event);
    msg_out.set_mouse_event(mouse_event);
    interface.send(Data::Message(msg_out));
}

#[inline]
pub fn send_pointer_device_event(
    mut evt: PointerDeviceEvent,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
    interface: &impl Interface,
) {
    let mut msg_out = Message::new();
    if alt {
        evt.modifiers.push(ControlKey::Alt.into());
    }
    if shift {
        evt.modifiers.push(ControlKey::Shift.into());
    }
    if ctrl {
        evt.modifiers.push(ControlKey::Control.into());
    }
    if command {
        evt.modifiers.push(ControlKey::Meta.into());
    }
    msg_out.set_pointer_device_event(evt);
    interface.send(Data::Message(msg_out));
}

/// Activate OS by sending mouse movement.
///
/// # Arguments
///
/// * `interface` - The interface for sending data.
/// * `send_left_click` - Whether to send a click event.
pub(super) fn activate_os(interface: &impl Interface, send_left_click: bool) {
    let left_down = MOUSE_BUTTON_LEFT << 3 | MOUSE_TYPE_DOWN;
    let left_up = MOUSE_BUTTON_LEFT << 3 | MOUSE_TYPE_UP;
    let right_down = MOUSE_BUTTON_RIGHT << 3 | MOUSE_TYPE_DOWN;
    let right_up = MOUSE_BUTTON_RIGHT << 3 | MOUSE_TYPE_UP;
    send_mouse(left_up, 0, 0, false, false, false, false, interface);
    std::thread::sleep(Duration::from_millis(50));
    send_mouse(0, 0, 0, false, false, false, false, interface);
    std::thread::sleep(Duration::from_millis(50));
    send_mouse(0, 3, 3, false, false, false, false, interface);
    let (click_down, click_up) = if send_left_click {
        (left_down, left_up)
    } else {
        (right_down, right_up)
    };
    std::thread::sleep(Duration::from_millis(50));
    send_mouse(click_down, 0, 0, false, false, false, false, interface);
    send_mouse(click_up, 0, 0, false, false, false, false, interface);
    /*
    let mut key_event = KeyEvent::new();
    // do not use Esc, which has problem with Linux
    key_event.set_control_key(ControlKey::RightArrow);
    key_event.press = true;
    let mut msg_out = Message::new();
    msg_out.set_key_event(key_event.clone());
    interface.send(Data::Message(msg_out.clone()));
    */
}

/// Input the OS's password.
///
/// # Arguments
///
/// * `p` - The password.
/// * `activate` - Whether to activate OS.
/// * `interface` - The interface for sending data.
pub fn input_os_password(p: String, activate: bool, interface: impl Interface) {
    std::thread::spawn(move || {
        _input_os_password(p, activate, interface);
    });
}

/// Input the OS's password.
///
/// # Arguments
///
/// * `p` - The password.
/// * `activate` - Whether to activate OS.
/// * `interface` - The interface for sending data.
pub(super) fn _input_os_password(p: String, activate: bool, interface: impl Interface) {
    let input_password = !p.is_empty();
    if activate {
        // Click event is used to bring up the password input box.
        activate_os(&interface, input_password);
        std::thread::sleep(Duration::from_millis(1200));
    }
    if !input_password {
        return;
    }
    let mut key_event = KeyEvent::new();
    key_event.mode = KeyboardMode::Legacy.into();
    key_event.press = true;
    let mut msg_out = Message::new();
    key_event.set_seq(p);
    msg_out.set_key_event(key_event.clone());
    interface.send(Data::Message(msg_out.clone()));
    key_event.set_control_key(ControlKey::Return);
    msg_out.set_key_event(key_event);
    interface.send(Data::Message(msg_out));
}
