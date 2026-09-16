use super::*;

/// Start a read job in CM for file transfer from server to client (Windows only).
///
/// This creates a `TransferJob` using `new_read()`, validates it, and sends the
/// initial file list back to Connection via IPC.
///
/// NOTE: This is the CM-side equivalent of `create_and_start_read_job()` in
/// `src/server/connection.rs`. On non-Windows platforms, Connection handles
/// read jobs directly. Both use `TransferJob::new_read()` with similar logic.
/// When modifying job creation or validation, ensure both paths stay in sync.
#[cfg(not(any(target_os = "ios")))]
pub(super) async fn start_read_job(
    path: String,
    file_num: i32,
    include_hidden: bool,
    id: i32,
    conn_id: i32,
    overwrite_detection: bool,
    read_jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
) {
    let path_clone = path.clone();
    let result = spawn_blocking(move || -> ResultType<fs::TransferJob> {
        let data_source = fs::DataSource::FilePath(PathBuf::from(&path));
        fs::TransferJob::new_read(
            id,
            fs::JobType::Generic,
            "".to_string(),
            data_source,
            file_num,
            include_hidden,
            true,
            overwrite_detection,
        )
    })
    .await;

    match result {
        Ok(Ok(mut job)) => {
            // Optional: enforce file count limit for CM-side jobs to avoid
            // excessive I/O. This is applied on the job's file list produced
            // by `new_read`, similar to how AllFiles uses the same helper.
            if let Err(msg) = check_file_count_limit(job.files().len()) {
                if let Err(e) = tx.send(Data::ReadJobInitResult {
                    id,
                    file_num,
                    include_hidden,
                    conn_id,
                    result: Err(msg),
                }) {
                    log::error!("error sending ReadJobInitResult via IPC: {}", e);
                }
                return;
            }

            // Build FileDirectory from the job's file list and serialize
            let files = job.files().to_owned();
            let mut dir = FileDirectory::new();
            dir.id = id;
            dir.path = path_clone.clone();
            dir.entries = files.clone().into();

            let dir_bytes = match dir.write_to_bytes() {
                Ok(bytes) => bytes,
                Err(e) => {
                    if let Err(e) = tx.send(Data::ReadJobInitResult {
                        id,
                        file_num,
                        include_hidden,
                        conn_id,
                        result: Err(format!("serialize failed: {}", e)),
                    }) {
                        log::error!("error sending ReadJobInitResult via IPC: {}", e);
                    }
                    return;
                }
            };

            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Ok(dir_bytes),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }

            // Attach connection id so CM can route read blocks back correctly
            job.conn_id = conn_id;
            read_jobs.push(job);
        }
        Ok(Err(e)) => {
            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Err(format!("validation failed: {}", e)),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }
        }
        Err(e) => {
            if let Err(e) = tx.send(Data::ReadJobInitResult {
                id,
                file_num,
                include_hidden,
                conn_id,
                result: Err(format!("validation task failed: {}", e)),
            }) {
                log::error!("error sending ReadJobInitResult via IPC: {}", e);
            }
        }
    }
}

/// Process read jobs periodically, reading file blocks and sending them via IPC.
///
/// NOTE: This is the CM-side equivalent of `handle_read_jobs()` in
/// `libs/base/src/fs.rs`. The logic mirrors that implementation
/// but communicates via IPC instead of direct network stream.
/// When modifying job processing logic, ensure both implementations stay in sync.
#[cfg(not(any(target_os = "ios")))]
pub(super) async fn handle_read_jobs_tick(
    jobs: &mut Vec<fs::TransferJob>,
    tx: &UnboundedSender<Data>,
    conn_id: i32,
) -> ResultType<()> {
    let mut finished = Vec::new();

    for job in jobs.iter_mut() {
        if job.is_last_job || job.paused {
            continue;
        }

        // Initialize data stream if needed (opens file, sends digest for overwrite detection)
        if let Err(err) = init_read_job_for_cm(job, tx, conn_id).await {
            if let Err(e) = tx.send(Data::FileReadError {
                id: job.id,
                file_num: job.file_num(),
                err: format!("{}", err),
                conn_id,
            }) {
                log::error!("error sending FileReadError via IPC: {}", e);
            }
            finished.push(job.id);
            continue;
        }

        // Bound bursts just like the direct sender, including small-file EOF blocks.
        let started = std::time::Instant::now();
        for _ in 0..16 {
        match job.read().await {
            Err(err) => {
                if let Err(e) = tx.send(Data::FileReadError {
                    id: job.id,
                    file_num: job.file_num(),
                    err: format!("{}", err),
                    conn_id,
                }) {
                    log::error!("error sending FileReadError via IPC: {}", e);
                }
                // Mark job as finished to prevent infinite retries.
                // Connection side will have already removed cm_read_job_ids
                // after receiving FileReadError, so continuing would be pointless.
                finished.push(job.id);
            }
            Ok(Some(block)) => {
                let file_ended = block.data.is_empty();
                if let Err(e) = tx.send(Data::FileBlockFromCM {
                    id: block.id,
                    file_num: block.file_num,
                    data: block.data,
                    compressed: block.compressed,
                    conn_id,
                }) {
                    log::error!("error sending FileBlockFromCM via IPC: {}", e);
                    break;
                }
                if file_ended {
                    if let Err(err) = init_read_job_for_cm(job, tx, conn_id).await {
                        tx.send(Data::FileReadError { id: job.id, file_num: job.file_num(),
                            err: err.to_string(), conn_id })?;
                        finished.push(job.id);
                        break;
                    }
                }
                if started.elapsed() < std::time::Duration::from_millis(2) {
                    continue;
                }
            }
            Ok(None) => {
                if job.job_completed() {
                    finished.push(job.id);
                    match job.job_error() {
                        Some(err) => {
                            if let Err(e) = tx.send(Data::FileReadError {
                                id: job.id,
                                file_num: job.file_num(),
                                err,
                                conn_id,
                            }) {
                                log::error!("error sending FileReadError via IPC: {}", e);
                            }
                        }
                        None => {
                            if let Err(e) = tx.send(Data::FileReadDone {
                                id: job.id,
                                file_num: job.file_num(),
                                conn_id,
                            }) {
                                log::error!("error sending FileReadDone via IPC: {}", e);
                            }
                        }
                    }
                }
                // else: waiting for confirmation from peer
            }
        }
        break;
        }
        // Break to handle jobs one by one.
        break;
    }

    for id in finished {
        let _ = fs::remove_job(id, jobs);
    }

    Ok(())
}

/// Initialize a read job's data stream and handle digest sending for overwrite detection.
///
/// NOTE: This is the CM-side equivalent of `TransferJob::init_data_stream()` in
/// `libs/base/src/fs.rs`. It calls `init_data_stream_for_cm()` and sends
/// digest via IPC instead of direct network stream.
/// When modifying initialization or digest logic, ensure both paths stay in sync.
#[cfg(not(any(target_os = "ios")))]
pub(super) async fn init_read_job_for_cm(
    job: &mut fs::TransferJob,
    tx: &UnboundedSender<Data>,
    conn_id: i32,
) -> ResultType<()> {
    // Initialize data stream and get digest info if overwrite detection is needed
    match job.init_data_stream_for_cm().await? {
        Some((last_modified, file_size)) => {
            // Send digest via IPC for overwrite detection
            if let Err(e) = tx.send(Data::FileDigestFromCM {
                id: job.id,
                file_num: job.file_num(),
                last_modified,
                file_size,
                is_resume: job.is_resume,
                conn_id,
            }) {
                log::error!("error sending FileDigestFromCM via IPC: {}", e);
            }
        }
        None => {
            // Job done or already initialized, nothing to do
        }
    }
    for digest in job.prefetch_digests().await? {
        tx.send(Data::FileDigestFromCM { id: digest.id, file_num: digest.file_num,
            last_modified: digest.last_modified, file_size: digest.file_size,
            is_resume: digest.is_resume, conn_id })?;
    }

    Ok(())
}
