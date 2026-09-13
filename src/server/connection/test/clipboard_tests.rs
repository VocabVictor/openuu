use super::*;

#[cfg(target_os = "windows")]
#[tokio::test]
async fn cliprdr_monitor_ready_is_forwarded_to_the_connection_manager() {
    let mut parts = Connection::for_test_parts(9501).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);

    let mut clip = Cliprdr::new();
    clip.set_ready(CliprdrMonitorReady::default());
    let mut msg = Message::new();
    msg.set_cliprdr(clip);
    assert!(parts.conn.on_message(msg).await);

    match parts.rx_to_cm.try_recv().expect("forwarded to cm") {
        ipc::Data::ClipboardFile(clipboard::ClipboardFile::MonitorReady) => {}
        _ => panic!("expected ClipboardFile(MonitorReady)"),
    }
}

#[tokio::test]
async fn text_clipboard_is_ignored_when_the_peer_disabled_it() {
    let mut parts = Connection::for_test_parts(9502).await;
    parts.conn.authorize_for_test(AuthConnType::Remote);
    parts.conn.clipboard = true;
    parts.conn.disable_clipboard = true;

    let mut msg = Message::new();
    msg.set_clipboard(Clipboard {
        content: b"ignored".to_vec().into(),
        ..Default::default()
    });
    assert!(parts.conn.on_message(msg).await);

    assert!(parts.rx_to_cm.try_recv().is_err());
    assert!(!parts.conn.clipboard_enabled());
}
