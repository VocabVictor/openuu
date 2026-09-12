use super::*;

#[test]
pub(super) fn obsolete_print_jobs_cannot_read_files() {
    let result = TransferJob::new_read(
        1, JobType::Printer, String::new(),
        DataSource::MemoryCursor(Cursor::new(vec![1, 2, 3])),
        0, false, false, false,
    );
    assert!(matches!(result, Err(e) if e.to_string() == "Unsupported transfer type"));
}

#[tokio::test]
pub(super) async fn obsolete_print_jobs_cannot_write_files() {
    let dir = TestTempDir::new("openuu_obsolete_print");
    let mut job = TransferJob::new_write(
        1, JobType::Printer, String::new(),
        DataSource::FilePath(dir.path.clone()), 0, false, true, false,
    );
    let result = job.write(FileTransferBlock { id: 1, ..Default::default() }).await;
    assert!(matches!(result, Err(e) if e.to_string() == "Unsupported transfer type"));
    assert!(!dir.path.exists());
}

#[test]
pub(super) fn adaptive_compression_recovers_after_incompressible_data() {
    let mut policy = TransferCompression::default();
    let mut seed = 123456789u32;
    let noise: Vec<u8> = (0..128 * 1024).map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as u8
    }).collect();
    assert!(policy.encode(&noise).is_none());
    let text = vec![b'a'; 128 * 1024];
    for _ in 0..31 {
        assert!(policy.encode(&text).is_none());
    }
    let encoded = policy.encode(&text).unwrap();
    assert_eq!(decompress(&encoded), text);
    assert!(is_compressed_file("archive.ZIP"));
}

#[tokio::test]
pub(super) async fn batched_transfer_preserves_payload_and_completion() {
    let dir = TestTempDir::new("openuu_transfer_batch");
    std::fs::create_dir_all(&dir.path).unwrap();
    let data: Vec<u8> = (0..1024 * 1024 + 37).map(|i| (i % 251) as u8).collect();
    let path = dir.join("payload.bin");
    std::fs::write(&path, &data).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (receiver, peer) = listener.accept().await.unwrap();
    let mut sender = hbb_common::Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, addr));
    let receive = tokio::spawn(async move {
        let mut receiver = hbb_common::tcp::FramedStream::from(receiver, peer);
        let mut actual = Vec::new();
        loop {
            let bytes = receiver.next().await.unwrap().unwrap();
            let msg = Message::parse_from_bytes(&bytes).unwrap();
            if let Some(message::Union::FileResponse(response)) = msg.union {
                match response.union {
                    Some(file_response::Union::Block(block)) => {
                        if block.compressed { actual.extend(decompress(&block.data)); }
                        else { actual.extend_from_slice(&block.data); }
                    }
                    Some(file_response::Union::Done(_)) => return actual,
                    Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                    _ => {}
                }
            }
        }
    });
    let job = TransferJob::new_read(7, JobType::Generic, String::new(),
        DataSource::FilePath(path), 0, false, false, false).unwrap();
    let mut jobs = vec![job];
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !jobs.is_empty() { handle_read_jobs(&mut jobs, &mut sender).await.unwrap(); }
        assert_eq!(receive.await.unwrap(), data);
    }).await.unwrap();
}

#[tokio::test]
pub(super) async fn small_files_batch_preserves_empty_files_and_order() {
    let dir = TestTempDir::new("openuu_small_files");
    std::fs::create_dir_all(&dir.path).unwrap();
    for i in 0..300 {
        let folder = dir.join(&format!("group{}", i % 3));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join(format!("file{i}.bin")), vec![(i % 251) as u8; if i % 10 == 0 { 0 } else { 1024 }]).unwrap();
    }
    let job = TransferJob::new_read(7, JobType::Generic, String::new(),
        DataSource::FilePath(dir.path.clone()), 0, false, false, false).unwrap();
    let mut data = Vec::new();
    for entry in job.files() { data.extend(std::fs::read(dir.path.join(&entry.name)).unwrap()); }
    let file_count = job.files().len();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (receiver, peer) = listener.accept().await.unwrap();
    let mut sender = hbb_common::Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, addr));
    let receive = tokio::spawn(async move {
        let mut receiver = hbb_common::tcp::FramedStream::from(receiver, peer);
        let mut actual = Vec::new();
        let mut ended = std::collections::HashSet::new();
        loop {
            let bytes = receiver.next().await.unwrap().unwrap();
            let msg = Message::parse_from_bytes(&bytes).unwrap();
            if let Some(message::Union::FileResponse(response)) = msg.union {
                match response.union {
                    Some(file_response::Union::Block(block)) => {
                        if block.data.is_empty() { assert!(ended.insert(block.file_num)); }
                        if block.compressed { actual.extend(decompress(&block.data)); }
                        else { actual.extend_from_slice(&block.data); }
                    }
                    Some(file_response::Union::Done(_)) => { assert_eq!(ended.len(), file_count); return actual; },
                    Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                    _ => {}
                }
            }
        }
    });
    let mut jobs = vec![job];
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let mut ticks = 0;
        let start = std::time::Instant::now();
        while !jobs.is_empty() {
            handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
            ticks += 1;
        }
        println!("small files: {file_count}, scheduler rounds: {ticks}, elapsed: {:?}", start.elapsed());
        assert_eq!(receive.await.unwrap(), data);
    }).await.unwrap();
}

#[tokio::test]
pub(super) async fn paused_transfer_preserves_offset_and_payload() {
    let dir = TestTempDir::new("openuu_transfer_pause");
    std::fs::create_dir_all(&dir.path).unwrap();
    let data: Vec<u8> = (0..8 * 1024 * 1024 + 37).map(|i| (i % 251) as u8).collect();
    let path = dir.join("payload.bin");
    std::fs::write(&path, &data).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let socket = tokio::net::TcpStream::connect(addr).await.unwrap();
    let (receiver, peer) = listener.accept().await.unwrap();
    let mut sender = hbb_common::Stream::Tcp(hbb_common::tcp::FramedStream::from(socket, addr));
    let receive = tokio::spawn(async move {
        let mut receiver = hbb_common::tcp::FramedStream::from(receiver, peer);
        let mut actual = Vec::new();
        loop {
            let bytes = receiver.next().await.unwrap().unwrap();
            let msg = Message::parse_from_bytes(&bytes).unwrap();
            if let Some(message::Union::FileResponse(response)) = msg.union {
                match response.union {
                    Some(file_response::Union::Block(block)) => {
                        if block.compressed { actual.extend(decompress(&block.data)); }
                        else { actual.extend_from_slice(&block.data); }
                    }
                    Some(file_response::Union::Done(_)) => return actual,
                    Some(file_response::Union::Error(error)) => panic!("{:?}", error),
                    _ => {}
                }
            }
        }
    });
    let job = TransferJob::new_read(7, JobType::Generic, String::new(),
        DataSource::FilePath(path), 0, false, false, false).unwrap();
    let mut jobs = vec![job];
    jobs[0].paused = true;
    handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
    assert_eq!(jobs[0].finished_size(), 0);
    assert!(jobs[0].data_stream.is_none());
    jobs[0].paused = false;
    handle_read_jobs(&mut jobs, &mut sender).await.unwrap();
    assert!(!jobs.is_empty());
    let offset = jobs[0].finished_size();
    assert!(offset > 0);
    jobs[0].paused = true;
    for _ in 0..5 { handle_read_jobs(&mut jobs, &mut sender).await.unwrap(); }
    assert_eq!(jobs[0].finished_size(), offset);
    assert!(jobs[0].data_stream.is_some());
    jobs[0].paused = false;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while !jobs.is_empty() { handle_read_jobs(&mut jobs, &mut sender).await.unwrap(); }
        assert_eq!(receive.await.unwrap(), data);
    }).await.unwrap();
}

#[tokio::test]
pub(super) async fn small_file_batch_still_requires_each_overwrite_confirmation() {
    let dir = TestTempDir::new("openuu_small_file_confirm");
    std::fs::create_dir_all(&dir.path).unwrap();
    std::fs::write(dir.join("a.bin"), b"first").unwrap();
    std::fs::write(dir.join("b.bin"), b"second").unwrap();
    let mut job = TransferJob::new_read(77, JobType::Generic, String::new(),
        DataSource::FilePath(dir.path.clone()), 0, false, false, true).unwrap();
    for file_num in 0..2 {
        assert!(job.init_data_stream_for_cm().await.unwrap().is_some());
        assert!(job.read().await.unwrap().is_none());
        assert!(!job.job_completed());
        let mut confirm = FileTransferSendConfirmRequest { id: 77, file_num, ..Default::default() };
        confirm.set_skip(false);
        job.confirm(&confirm).await;
        let mut actual = Vec::new();
        loop {
            let block = job.read().await.unwrap().unwrap();
            if block.data.is_empty() { break; }
            if block.compressed { actual.extend(decompress(&block.data)); }
            else { actual.extend_from_slice(&block.data); }
        }
        assert_eq!(actual, std::fs::read(dir.path.join(&job.files()[file_num as usize].name)).unwrap());
    }
}

#[tokio::test]
pub(super) async fn small_file_buffer_does_not_truncate_a_growing_file() {
    let dir = TestTempDir::new("openuu_small_file_growth");
    std::fs::create_dir_all(&dir.path).unwrap();
    let path = dir.join("growing.bin");
    std::fs::write(&path, b"x").unwrap();
    let mut job = TransferJob::new_read(78, JobType::Generic, String::new(),
        DataSource::FilePath(path.clone()), 0, false, false, false).unwrap();
    std::fs::write(&path, b"expanded contents").unwrap();
    job.init_data_stream_for_cm().await.unwrap();
    let mut actual = Vec::new();
    loop {
        let block = job.read().await.unwrap().unwrap();
        if block.data.is_empty() { break; }
        actual.extend_from_slice(&block.data);
    }
    assert_eq!(actual, b"expanded contents");
}
