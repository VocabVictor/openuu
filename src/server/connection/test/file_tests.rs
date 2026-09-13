use super::*;

fn file_action(union: file_action::Union) -> Message {
    let mut fa = FileAction::new();
    fa.union = Some(union);
    let mut msg = Message::new();
    msg.set_file_action(fa);
    msg
}

fn file_response(union: file_response::Union) -> Message {
    let mut fr = FileResponse::new();
    fr.union = Some(union);
    let mut msg = Message::new();
    msg.set_file_response(fr);
    msg
}

#[tokio::test]
async fn read_dir_is_deferred_while_a_delayed_read_dir_is_pending() {
    let mut parts = Connection::for_test_parts(9601).await;
    parts.conn.authorize_for_test(AuthConnType::FileTransfer);
    parts.conn.file_transfer = Some(("".to_owned(), false));
    parts.conn.delayed_read_dir = Some(("C:\\old".to_owned(), false));

    let msg = file_action(file_action::Union::ReadDir(ReadDir {
        path: "C:\\new".to_owned(),
        include_hidden: true,
        ..Default::default()
    }));
    assert!(parts.conn.on_message(msg).await);

    assert_eq!(
        parts.conn.delayed_read_dir,
        Some(("C:\\new".to_owned(), true))
    );
    assert!(parts.rx_to_cm.try_recv().is_err(), "nothing reaches the cm yet");
}

#[tokio::test]
async fn file_response_block_is_handed_to_the_cm_writer() {
    let mut parts = Connection::for_test_parts(9602).await;
    parts.conn.authorize_for_test(AuthConnType::FileTransfer);
    parts.conn.file_transfer = Some(("".to_owned(), false));

    let msg = file_response(file_response::Union::Block(FileTransferBlock {
        id: 7,
        file_num: 2,
        data: b"abc".to_vec().into(),
        compressed: false,
        ..Default::default()
    }));
    assert!(parts.conn.on_message(msg).await);

    match parts.rx_to_cm.try_recv().expect("block forwarded") {
        ipc::Data::FS(ipc::FS::WriteBlock {
            id,
            file_num,
            data,
            compressed,
        }) => {
            assert_eq!((id, file_num), (7, 2));
            assert_eq!(&data[..], b"abc");
            assert!(!compressed);
        }
        _ => panic!("expected FS::WriteBlock"),
    }
}

#[tokio::test]
async fn file_response_done_is_handed_to_the_cm_writer() {
    let mut parts = Connection::for_test_parts(9603).await;
    parts.conn.authorize_for_test(AuthConnType::FileTransfer);
    parts.conn.file_transfer = Some(("".to_owned(), false));

    let msg = file_response(file_response::Union::Done(FileTransferDone {
        id: 7,
        file_num: 3,
        ..Default::default()
    }));
    assert!(parts.conn.on_message(msg).await);

    match parts.rx_to_cm.try_recv().expect("done forwarded") {
        ipc::Data::FS(ipc::FS::WriteDone { id, file_num }) => assert_eq!((id, file_num), (7, 3)),
        _ => panic!("expected FS::WriteDone"),
    }
}
