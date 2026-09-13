use super::*;

fn file_response(union: file_response::Union) -> Message {
    let mut msg = Message::new();
    msg.set_file_response(FileResponse {
        union: Some(union),
        ..Default::default()
    });
    msg
}

fn entry(name: &str) -> FileEntry {
    FileEntry {
        name: name.to_owned(),
        ..Default::default()
    }
}

fn listing(id: i32, path: &str, names: &[&str]) -> Message {
    file_response(file_response::Union::Dir(FileDirectory {
        id,
        path: path.to_owned(),
        entries: names.iter().map(|n| entry(n)).collect(),
        ..Default::default()
    }))
}

/// A write job receiving into a fresh directory under the system temp dir.
fn write_job(id: i32, tag: &str) -> fs::TransferJob {
    let dir = std::env::temp_dir().join(format!("openuu-peer-file-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    fs::TransferJob::new_write(
        id,
        fs::JobType::Generic,
        String::new(),
        fs::DataSource::FilePath(dir),
        0,
        false,
        false,
        false,
    )
}

fn sent_file_action(msg: Message) -> file_action::Union {
    match msg.union {
        Some(message::Union::FileAction(action)) => action.union.unwrap(),
        other => panic!("expected a FileAction, got {other:?}"),
    }
}

#[tokio::test]
async fn empty_dirs_reach_the_ui() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let msg = file_response(file_response::Union::EmptyDirs(ReadEmptyDirsResponse {
        path: "/data".to_owned(),
        ..Default::default()
    }));
    assert!(feed(&mut parts, &msg).await);
    assert_eq!(parts.remote.handler.calls(), vec!["update_empty_dirs:/data"]);
}

#[tokio::test]
async fn a_listing_without_a_job_is_forwarded_as_is() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    assert!(feed(&mut parts, &listing(7, "/data", &["a.txt", "b.txt"])).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["update_folder_files:7,2,/data,false,false"]
    );
}

#[tokio::test]
async fn a_listing_fills_the_matching_write_job() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    parts.remote.write_jobs.push(write_job(7, "fill"));
    assert!(feed(&mut parts, &listing(7, "/data", &["a.txt"])).await);
    assert_eq!(parts.remote.write_jobs[0].files().len(), 1);
    assert!(parts
        .remote
        .handler
        .has_call("update_folder_files:7,1,/data,false,false"));
    assert!(try_next_message(&mut parts.far_end, 100).await.is_none());
}

#[tokio::test]
async fn an_unsafe_listing_cancels_the_write_job() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    parts.remote.write_jobs.push(write_job(7, "unsafe"));
    assert!(feed(&mut parts, &listing(7, "/data", &["../escape.txt"])).await);
    assert!(parts.remote.write_jobs.is_empty());
    assert!(parts.remote.handler.has_call("job_error:7,-1,"));
    match sent_file_action(next_message(&mut parts.far_end).await) {
        file_action::Union::Cancel(c) => assert_eq!(c.id, 7),
        other => panic!("expected a cancel, got {other:?}"),
    }
}

#[tokio::test]
async fn a_digest_for_a_missing_file_asks_for_the_whole_file() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let job = write_job(7, "digest").with_files(vec![entry("a.txt")]).unwrap();
    parts.remote.write_jobs.push(job);
    let msg = file_response(file_response::Union::Digest(FileTransferDigest {
        id: 7,
        file_num: 0,
        file_size: 3,
        ..Default::default()
    }));
    assert!(feed(&mut parts, &msg).await);
    match sent_file_action(next_message(&mut parts.far_end).await) {
        file_action::Union::SendConfirm(c) => {
            assert_eq!((c.id, c.file_num), (7, 0));
            assert_eq!(
                c.union,
                Some(file_transfer_send_confirm_request::Union::OffsetBlk(0))
            );
        }
        other => panic!("expected a send confirm, got {other:?}"),
    }
    assert!(parts.remote.handler.calls().is_empty());
}

#[tokio::test]
async fn a_digest_without_a_job_is_ignored() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let msg = file_response(file_response::Union::Digest(FileTransferDigest {
        id: 7,
        ..Default::default()
    }));
    assert!(feed(&mut parts, &msg).await);
    assert!(try_next_message(&mut parts.far_end, 100).await.is_none());
    assert!(parts.remote.handler.calls().is_empty());
}

#[tokio::test]
async fn done_and_error_without_a_job_report_to_the_ui() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let done = file_response(file_response::Union::Done(FileTransferDone {
        id: 9,
        file_num: 2,
        ..Default::default()
    }));
    let error = file_response(file_response::Union::Error(FileTransferError {
        id: 9,
        file_num: 3,
        error: "boom".to_owned(),
        ..Default::default()
    }));
    assert!(feed(&mut parts, &done).await);
    assert!(feed(&mut parts, &error).await);
    assert_eq!(
        parts.remote.handler.calls(),
        vec!["job_done:9,2", "job_error:9,3,boom"]
    );
}

#[tokio::test]
async fn a_send_confirm_without_a_read_job_is_ignored() {
    let mut parts = Remote::<RecordingUi>::for_test().await;
    let mut action = FileAction::new();
    action.set_send_confirm(FileTransferSendConfirmRequest {
        id: 5,
        ..Default::default()
    });
    let mut msg = Message::new();
    msg.set_file_action(action);
    assert!(feed(&mut parts, &msg).await);
    assert!(parts.remote.handler.calls().is_empty());
    assert!(try_next_message(&mut parts.far_end, 100).await.is_none());
}
