use super::*;

fn mouse_event(x: i32, y: i32) -> Message {
    let mut msg = Message::new();
    msg.set_mouse_event(MouseEvent {
        mask: 0,
        x,
        y,
        ..Default::default()
    });
    msg
}

fn key_event(chr: u32, press: bool) -> Message {
    let mut ke = KeyEvent::new();
    ke.set_chr(chr);
    ke.press = press;
    ke.mode = KeyboardMode::Legacy.into();
    let mut msg = Message::new();
    msg.set_key_event(ke);
    msg
}

fn pointer_event() -> Message {
    let mut msg = Message::new();
    msg.set_pointer_device_event(PointerDeviceEvent::default());
    msg
}

#[tokio::test]
async fn mouse_event_is_forwarded_to_the_input_thread() {
    let mut parts = Connection::for_test_parts(9201).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.keyboard = true;
    parts.conn.disable_keyboard = false;
    parts.conn.lr.my_name = "ctrl".to_owned();

    assert!(parts.conn.on_message(mouse_event(11, 22)).await);

    match parts.rx_input.try_recv().expect("mouse forwarded") {
        MessageInput::Mouse(im) => {
            assert_eq!((im.msg.x, im.msg.y), (11, 22));
            assert_eq!(im.conn_id, 9201);
            assert_eq!(im.username, "ctrl");
            assert!(im.simulate);
        }
        _ => panic!("expected MessageInput::Mouse"),
    }
}

#[tokio::test]
async fn mouse_event_is_dropped_when_the_peer_disabled_the_keyboard() {
    let mut parts = Connection::for_test_parts(9202).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.keyboard = true;
    parts.conn.disable_keyboard = true;
    parts.conn.show_my_cursor = false;

    assert!(parts.conn.on_message(mouse_event(1, 1)).await);

    assert!(parts.rx_input.try_recv().is_err());
}

#[tokio::test]
async fn mouse_event_only_shows_the_cursor_when_input_is_off() {
    let mut parts = Connection::for_test_parts(9203).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.keyboard = false;
    parts.conn.show_my_cursor = true;

    assert!(parts.conn.on_message(mouse_event(3, 4)).await);

    match parts.rx_input.try_recv().expect("cursor forwarded") {
        MessageInput::Mouse(im) => {
            assert!(!im.simulate);
            assert!(im.show_cursor);
        }
        _ => panic!("expected MessageInput::Mouse"),
    }
}

#[tokio::test]
async fn key_press_and_release_are_forwarded_with_their_state() {
    let mut parts = Connection::for_test_parts(9204).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.keyboard = true;

    assert!(parts.conn.on_message(key_event(0x41, true)).await);
    assert!(parts.conn.on_message(key_event(0x41, false)).await);

    match parts.rx_input.try_recv().expect("press forwarded") {
        MessageInput::Key((ke, press)) => {
            assert_eq!(ke.chr(), 0x41);
            assert!(press);
        }
        _ => panic!("expected MessageInput::Key"),
    }
    match parts.rx_input.try_recv().expect("release forwarded") {
        MessageInput::Key((_, press)) => assert!(!press),
        _ => panic!("expected MessageInput::Key"),
    }
}

#[tokio::test]
async fn key_event_is_dropped_without_keyboard_permission() {
    let mut parts = Connection::for_test_parts(9205).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.keyboard = false;

    assert!(parts.conn.on_message(key_event(0x41, true)).await);

    assert!(parts.rx_input.try_recv().is_err());
}

#[tokio::test]
async fn pointer_event_is_forwarded_with_the_connection_id() {
    let mut parts = Connection::for_test_parts(9206).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.keyboard = true;

    assert!(parts.conn.on_message(pointer_event()).await);

    match parts.rx_input.try_recv().expect("pointer forwarded") {
        MessageInput::Pointer((_, conn_id)) => assert_eq!(conn_id, 9206),
        _ => panic!("expected MessageInput::Pointer"),
    }
}
