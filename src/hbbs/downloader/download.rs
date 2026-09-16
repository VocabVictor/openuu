use super::*;

// The caller should check if the file is downloaded successfully and remove the job from the map.
pub fn download_file(
    url: String,
    path: Option<PathBuf>,
    auto_del_dur: Option<Duration>,
) -> ResultType<String> {
    let id = url.clone();
    // First pass: if a non-error downloader exists for this URL, reuse it.
    // If an errored downloader exists, remove it so this call can retry.
    let mut stale_path = None;
    {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        if let Some(downloader) = downloaders.get(&id) {
            if downloader.error.is_none() {
                return Ok(id);
            }
            stale_path = downloader.path.clone();
            downloaders.remove(&id);
        }
    }
    if let Some(p) = stale_path {
        if p.exists() {
            if let Err(e) = std::fs::remove_file(&p) {
                log::warn!("Failed to remove stale download file {}: {}", p.display(), e);
            }
        }
    }

    if let Some(path) = path.as_ref() {
        if path.exists() {
            bail!("File {} already exists", path.display());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let (tx, rx) = unbounded_channel();
    let downloader = Downloader {
        data: Vec::new(),
        path: path.clone(),
        total_size: None,
        downloaded_size: 0,
        error: None,
        tx_cancel: tx,
        finished: false,
    };
    // Second pass (atomic with insert) to avoid race with another concurrent caller.
    let mut stale_path_after_check = None;
    {
        let mut downloaders = DOWNLOADERS.lock().unwrap();
        if let Some(existing) = downloaders.get(&id) {
            if existing.error.is_none() {
                return Ok(id);
            }
            stale_path_after_check = existing.path.clone();
            downloaders.remove(&id);
        }
        downloaders.insert(id.clone(), downloader);
    }
    if let Some(p) = stale_path_after_check {
        if p.exists() {
            if let Err(e) = std::fs::remove_file(&p) {
                log::warn!("Failed to remove stale download file {}: {}", p.display(), e);
            }
        }
    }

    let id2 = id.clone();
    std::thread::spawn(
        move || match do_download(&id2, url, path, auto_del_dur, rx) {
            Ok(is_all_downloaded) => {
                let mut downloaded_size = 0;
                let mut total_size = 0;
                DOWNLOADERS.lock().unwrap().get_mut(&id2).map(|downloader| {
                    downloaded_size = downloader.downloaded_size;
                    total_size = downloader.total_size.unwrap_or(0);
                });
                log::info!(
                    "Download {} end, {}/{}, {:.2} %",
                    &id2,
                    downloaded_size,
                    total_size,
                    if total_size == 0 {
                        0.0
                    } else {
                        downloaded_size as f64 / total_size as f64 * 100.0
                    }
                );

                let is_canceled = !is_all_downloaded;
                if is_canceled {
                    if let Some(downloader) = DOWNLOADERS.lock().unwrap().remove(&id2) {
                        if let Some(p) = downloader.path {
                            if p.exists() {
                                std::fs::remove_file(p).ok();
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let err = e.to_string();
                log::error!("Download {}, failed: {}", &id2, &err);
                DOWNLOADERS.lock().unwrap().get_mut(&id2).map(|downloader| {
                    downloader.error = Some(err);
                });
            }
        },
    );

    Ok(id)
}

#[tokio::main(flavor = "current_thread")]
pub(super) async fn do_download(
    id: &str,
    url: String,
    path: Option<PathBuf>,
    auto_del_dur: Option<Duration>,
    mut rx_cancel: UnboundedReceiver<()>,
) -> ResultType<bool> {
    let client = create_http_client_async_with_url_strict(&url).await?;

    let mut is_all_downloaded = false;
    tokio::select! {
        _ = rx_cancel.recv() => {
            return Ok(is_all_downloaded);
        }
        head_resp = client.head(&url).send() => {
            match head_resp {
                Ok(resp) => {
                    if resp.status().is_success() {
                        let total_size = resp
                            .headers()
                            .get(reqwest::header::CONTENT_LENGTH)
                            .and_then(|ct_len| ct_len.to_str().ok())
                            .and_then(|ct_len| ct_len.parse::<u64>().ok());
                        let Some(total_size) = total_size else {
                            bail!("Failed to get content length");
                        };
                        DOWNLOADERS.lock().unwrap().get_mut(id).map(|downloader| {
                            downloader.total_size = Some(total_size);
                        });
                    } else {
                        bail!("Failed to get content length: {}", resp.status());
                    }
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
        }
    }

    let mut response;
    tokio::select! {
        _ = rx_cancel.recv() => {
            return Ok(is_all_downloaded);
        }
        resp = client.get(url).send() => {
            response = resp?;
        }
    }

    let mut dest: Option<File> = None;
    if let Some(p) = path {
        dest = Some(File::create(p).await?);
    }

    loop {
        tokio::select! {
            _ = rx_cancel.recv() => {
                break;
            }
            chunk = response.chunk() => {
                match chunk {
                    Ok(Some(chunk)) => {
                        match dest {
                            Some(ref mut f) => {
                                f.write_all(&chunk).await?;
                                f.flush().await?;
                                DOWNLOADERS.lock().unwrap().get_mut(id).map(|downloader| {
                                    downloader.downloaded_size += chunk.len() as u64;
                                });
                            }
                            None => {
                                DOWNLOADERS.lock().unwrap().get_mut(id).map(|downloader| {
                                    downloader.data.extend_from_slice(&chunk);
                                    downloader.downloaded_size += chunk.len() as u64;
                                });
                            }
                        }
                    }
                    Ok(None) => {
                        is_all_downloaded = true;
                        break;
                    },
                    Err(e) => {
                        log::error!("Download {} failed: {}", id, e);
                        return Err(e.into());
                    }
                }
            }
        }
    }

    if let Some(mut f) = dest.take() {
        f.flush().await?;
    }

    if let Some(ref mut downloader) = DOWNLOADERS.lock().unwrap().get_mut(id) {
        downloader.finished = true;
    }
    if is_all_downloaded {
        let id_del = id.to_string();
        if let Some(dur) = auto_del_dur {
            tokio::spawn(async move {
                tokio::time::sleep(dur).await;
                DOWNLOADERS.lock().unwrap().remove(&id_del);
            });
        }
    }
    Ok(is_all_downloaded)
}
