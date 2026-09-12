use super::*;
use hbb_common::protobuf::Message as _;
use std::collections::HashSet;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("openuu-network-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// Delays confirmation responses only: this models the per-file round-trip
// bottleneck, not a complete WAN emulator or a bandwidth benchmark.
async fn run_transfer(count: usize, confirmation_delay_ms: u64, pipeline: bool) -> Duration {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    let destination = fixture.0.join("destination");
    for i in 0..count {
        let folder = source.join(format!("group{}", i % 5));
        std::fs::create_dir_all(&folder).unwrap();
        let bytes: Vec<u8> = (0..if i % 11 == 0 { 0 } else { 1024 + i % 127 })
            .map(|n| ((n + i) % 251) as u8)
            .collect();
        std::fs::write(folder.join(format!("file{i:04}.bin")), bytes).unwrap();
    }
    let job = TransferJob::new_read(
        901,
        JobType::Generic,
        destination.to_string_lossy().into(),
        DataSource::FilePath(source.clone()),
        0,
        false,
        false,
        true,
    )
    .unwrap();
    let entries = job.files().clone();
    assert_eq!(entries.len(), count);
    let skipped = 7usize;
    let preserved = destination.join(&entries[skipped].name);
    std::fs::create_dir_all(preserved.parent().unwrap()).unwrap();
    std::fs::write(&preserved, b"existing file: do not overwrite").unwrap();
    let mut writer = TransferJob::new_write(
        901,
        JobType::Generic,
        String::new(),
        DataSource::FilePath(destination.clone()),
        0,
        false,
        true,
        true,
    )
    .with_files(entries.clone())
    .unwrap();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let socket = tokio::net::TcpStream::connect(address).await.unwrap();
    socket.set_nodelay(true).unwrap();
    let (socket_rx, peer) = listener.accept().await.unwrap();
    socket_rx.set_nodelay(true).unwrap();
    let mut sender = Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, address));
    let receiver = tokio::spawn(async move {
        let mut stream = hbb_common::tcp::FramedStream::from(socket_rx, peer);
        let mut confirmed = HashSet::new();
        let mut ended = HashSet::new();
        let mut digests = 0;
        let mut pending = std::collections::VecDeque::new();
        loop {
            let deadline = pending.front().map(|(deadline, _): &(tokio::time::Instant, Message)| *deadline)
                .unwrap_or_else(|| tokio::time::Instant::now() + Duration::from_secs(60));
            let bytes = tokio::select! {
                _ = tokio::time::sleep_until(deadline), if !pending.is_empty() => {
                    let (_, reply) = pending.pop_front().unwrap();
                    stream.send(&reply).await.unwrap();
                    continue;
                }
                bytes = stream.next() => bytes.unwrap().unwrap(),
            };
            let message = Message::parse_from_bytes(&bytes).unwrap();
            if let Some(message::Union::FileResponse(response)) = message.union {
                match response.union {
                    Some(file_response::Union::Digest(digest)) => {
                        digests += 1;
                        writer.set_file_digest(digest.file_num, digest.file_size, digest.last_modified);
                        let skip = digest.file_num as usize == skipped;
                        if !skip {
                            assert!(confirmed.insert(digest.file_num));
                        }
                        let mut confirm = FileTransferSendConfirmRequest {
                            id: digest.id,
                            file_num: digest.file_num,
                            ..Default::default()
                        };
                        confirm.set_skip(skip);
                        confirm.confirmation_window = if pipeline { 16 } else { 0 };
                        let mut action = FileAction::new();
                        action.set_send_confirm(confirm);
                        let mut reply = Message::new();
                        reply.set_file_action(action);
                        pending.push_back((tokio::time::Instant::now() + Duration::from_millis(confirmation_delay_ms), reply));
                    }
                    Some(file_response::Union::Block(block)) => {
                        assert!(
                            confirmed.contains(&block.file_num),
                            "data sent before confirmation"
                        );
                        assert_ne!(block.file_num as usize, skipped);
                        if block.data.is_empty() {
                            assert!(ended.insert(block.file_num));
                        }
                        writer.write(block).await.unwrap();
                    }
                    Some(file_response::Union::Done(_)) => {
                        writer.modify_time();
                        drop(writer);
                        assert_eq!(ended.len(), count - 1, "missing file end markers");
                        assert_eq!(digests, count, "every file must retain overwrite checking");
                        return;
                    }
                    Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                    _ => {}
                }
            }
        }
    });
    let mut jobs = vec![job];
    let start = std::time::Instant::now();
    let mut pauses = 0;
    let mut rounds = 0;
    while !jobs.is_empty() {
        if pauses == 0 && jobs[0].file_num() >= 3 {
            jobs[0].paused = true;
            let size = jobs[0].finished_size();
            let file_num = jobs[0].file_num();
            for _ in 0..3 {
                handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
            }
            assert_eq!(jobs[0].finished_size(), size);
            assert_eq!(jobs[0].file_num(), file_num);
            jobs[0].paused = false;
            pauses += 1;
        }
        handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
        rounds += 1;
        if !jobs.is_empty() && jobs[0].file_is_waiting() && !jobs[0].file_confirmed() {
            let bytes = sender.next().await.unwrap().unwrap();
            let message = Message::parse_from_bytes(&bytes).unwrap();
            match message.union {
                Some(message::Union::FileAction(action)) => match action.union {
                    Some(file_action::Union::SendConfirm(confirm)) => {
                        jobs[0].confirm(&confirm).await;
                    }
                    _ => panic!("unexpected action"),
                },
                _ => panic!("unexpected confirmation response"),
            }
        }
    }
    receiver.await.unwrap();
    assert_eq!(pauses, 1);
    for (index, entry) in entries.iter().enumerate() {
        let actual = std::fs::read(destination.join(&entry.name)).unwrap();
        if index == skipped {
            assert_eq!(actual, b"existing file: do not overwrite");
        } else {
            assert_eq!(
                actual,
                std::fs::read(source.join(&entry.name)).unwrap(),
                "{}",
                entry.name
            );
        }
    }
    println!("pipeline={pipeline}, files={count}, confirmation_delay_ms={confirmation_delay_ms}, rounds={rounds}, elapsed_ms={}", start.elapsed().as_millis());
    start.elapsed()
}

#[tokio::test]
async fn thousand_small_files_over_tcp_preserve_files_and_pause() {
    tokio::time::timeout(Duration::from_secs(60), run_transfer(1000, 0, true))
        .await
        .unwrap();
}

#[tokio::test]
async fn small_files_with_delayed_confirmations_preserve_overwrite_and_pause() {
    let legacy = tokio::time::timeout(Duration::from_secs(30), run_transfer(100, 40, false)).await.unwrap();
    let pipelined = tokio::time::timeout(Duration::from_secs(30), run_transfer(100, 40, true)).await.unwrap();
    assert!(pipelined < legacy.mul_f64(0.75), "pipeline {:?}, legacy {:?}", pipelined, legacy);
}
