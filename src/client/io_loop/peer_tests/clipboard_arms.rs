use super::*;

fn clipboard_text(text: &str) -> Message {
    let mut msg = Message::new();
    msg.set_clipboard(Clipboard {
        content: text.as_bytes().to_vec().into(),
        ..Default::default()
    });
    msg
}

fn multi_clipboards(text: &str) -> Message {
    let mut msg = Message::new();
    msg.set_multi_clipboards(MultiClipboards {
        clipboards: vec![Clipboard {
            content: text.as_bytes().to_vec().into(),
            ..Default::default()
        }],
        ..Default::default()
    });
    msg
}

// Only the refusing paths are exercised: the accepting path writes the real
// system clipboard, which a unit test must not touch.

#[tokio::test]
async fn clipboard_messages_are_ignored_when_the_clipboard_is_disabled() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    parts.remote.handler.lc.write().unwrap().config.disable_clipboard.v = true;
    assert!(feed(&mut parts, &clipboard_text("secret")).await);
    assert!(feed(&mut parts, &multi_clipboards("secret")).await);
    assert!(parts.remote.handler.calls().is_empty());
    assert!(parts.rx_media.try_recv().is_err());
}

#[tokio::test]
async fn clipboard_messages_are_ignored_in_a_view_only_session() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    {
        let mut lc = parts.remote.handler.lc.write().unwrap();
        lc.view_only_session = true;
        lc.config.disable_clipboard.v = false;
    }
    assert!(parts.remote.handler.lc.read().unwrap().get_toggle_option("view-only"));
    assert!(feed(&mut parts, &clipboard_text("secret")).await);
    assert!(feed(&mut parts, &multi_clipboards("secret")).await);
    assert!(parts.remote.handler.calls().is_empty());
}
