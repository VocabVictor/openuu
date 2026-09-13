use super::*;

#[inline]
pub fn remove_job(id: i32, jobs: &mut Vec<TransferJob>) -> Option<TransferJob> {
    jobs.iter()
        .position(|x| x.id() == id)
        .map(|index| jobs.remove(index))
}

#[inline]
pub fn get_job(id: i32, jobs: &mut [TransferJob]) -> Option<&mut TransferJob> {
    jobs.iter_mut().find(|x| x.id() == id)
}

#[inline]
pub fn get_job_immutable(id: i32, jobs: &[TransferJob]) -> Option<&TransferJob> {
    jobs.iter().find(|x| x.id() == id)
}

pub(super) async fn init_jobs<S: MsgSink + ?Sized>(jobs: &mut Vec<TransferJob>, stream: &mut S) -> ResultType<()> {
    for job in jobs.iter_mut() {
        if job.is_last_job || job.paused {
            continue;
        }
        if let Err(err) = job.init_data_stream(stream).await {
            stream
                .send_msg(new_error(job.id(), err, job.file_num()))
                .await?;
        }
    }
    Ok(())
}

pub async fn handle_read_jobs<S: MsgSink + ?Sized>(
    jobs: &mut Vec<TransferJob>,
    stream: &mut S,
) -> ResultType<String> {
    init_jobs(jobs, stream).await?;

    let mut job_log = Default::default();
    let mut finished = Vec::new();
    for job in jobs.iter_mut() {
        if job.is_last_job || job.paused {
            continue;
        }
        let started = std::time::Instant::now();
        for _ in 0..16 {
            match job.read().await {
                Err(err) => {
                    stream
                        .send_msg(new_error(job.id(), err, job.file_num()))
                        .await?;
                }
                Ok(Some(block)) => {
                    let file_ended = block.data.is_empty();
                    stream.send_msg(new_block(block)).await?;
                    // Bound each burst so control messages and cancellation get a turn.
                    if file_ended {
                        // Send the next digest immediately, but never bypass its confirmation.
                        if let Err(err) = job.init_data_stream(stream).await {
                            stream.send_msg(new_error(job.id(), err, job.file_num())).await?;
                            break;
                        }
                    }
                    if started.elapsed() < std::time::Duration::from_millis(2) {
                        continue;
                    }
                }
                Ok(None) => {
                    if job.job_completed() {
                        job_log = serialize_transfer_job(job, true, false, "");
                        finished.push(job.id());
                        match job.job_error() {
                            Some(err) => {
                                job_log = serialize_transfer_job(job, false, false, &err);
                                stream
                                    .send_msg(new_error(job.id(), err, job.file_num()))
                                    .await?
                            }
                            None => stream.send_msg(new_done(job.id(), job.file_num())).await?,
                        }
                    } else {
                        // waiting confirmation.
                    }
                }
            }
            break;
        }
        // Preserve sequential job ordering and overwrite confirmation.
        break;
    }
    for id in finished {
        let _ = remove_job(id, jobs);
    }
    Ok(job_log)
}
