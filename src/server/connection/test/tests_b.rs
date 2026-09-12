use super::*;

#[cfg(target_os = "macos")]
#[test]
pub(super) fn retina() {
    let mut retina = Retina {
        displays: vec![DisplayInfo {
            x: 10,
            y: 10,
            width: 1000,
            height: 1000,
            scale: 2.0,
            ..Default::default()
        }],
    };
    let mut mouse: MouseEvent = MouseEvent {
        x: 510,
        y: 510,
        ..Default::default()
    };
    retina.on_mouse_event(&mut mouse, 0);
    assert_eq!(mouse.x, 260);
    assert_eq!(mouse.y, 260);
    let pos = CursorPosition {
        x: 260,
        y: 260,
        ..Default::default()
    };
    let msg = retina.on_cursor_pos(&pos, 0).unwrap();
    let pos = msg.cursor_position();
    assert_eq!(pos.x, 510);
    assert_eq!(pos.y, 510);
}

#[test]
pub(super) fn ipv6() {
    assert!(Ipv6Addr::from_str("::1").is_ok());
    assert!(Ipv6Addr::from_str("127.0.0.1").is_err());
    assert!(Ipv6Addr::from_str("0").is_err());
}

pub(super) fn msg(set: impl FnOnce(&mut Message)) -> Message {
    let mut msg = Message::new();
    set(&mut msg);
    msg
}

pub(super) fn misc_msg(set: impl FnOnce(&mut Misc)) -> Message {
    msg(|msg| {
        let mut misc = Misc::new();
        set(&mut misc);
        msg.set_misc(misc);
    })
}

pub(super) fn option_msg(set: impl FnOnce(&mut OptionMessage)) -> Message {
    misc_msg(|misc| {
        let mut option = OptionMessage::new();
        set(&mut option);
        misc.set_option(option);
    })
}

pub(super) fn set_supported_decoding(option: &mut OptionMessage) {
    option.supported_decoding = hbb_common::protobuf::MessageField::some(Default::default());
}

pub(super) fn assert_scopes(
    conn_type: AuthConnType,
    cases: impl IntoIterator<Item = (Message, Option<&'static str>)>,
) {
    for (msg, expected) in cases {
        assert_eq!(
            Connection::authorized_message_scope_violation(conn_type, &msg),
            expected
        );
    }
}
