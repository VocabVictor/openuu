use super::*;

impl TransferJob {
    pub async fn write(&mut self, block: FileTransferBlock) -> ResultType<()> {
        if self.r#type == JobType::Printer {
            bail!("Unsupported transfer type");
        }
        if block.id != self.id {
            bail!("Wrong id");
        }
        match &self.data_source {
            DataSource::FilePath(p) => {
                let file_num = block.file_num as usize;
                if file_num >= self.files.len() {
                    bail!("Wrong file number");
                }
                if file_num != self.file_num as usize || self.data_stream.is_none() {
                    self.modify_time();
                    if let Some(DataStream::FileStream(file)) = self.data_stream.as_mut() {
                        file.sync_all().await?;
                    }
                    self.file_num = block.file_num;
                    if let Some(digest) = self.file_digests.remove(&block.file_num) { self.digest = digest; }
                    self.file_digests.retain(|number, _| *number >= block.file_num);
                    let entry = &self.files[file_num];
                    let (path, digest_path) = {
                        let path = join_validated_path(p, &entry.name)?;
                        // NOTE: We intentionally keep path-based validation + regular file open here.
                        // This still has a known TOCTOU window for symlink races, but avoids a large
                        // cross-platform rewrite for now.
                        // Revisit with descriptor/handle-based no-follow open in future hardening.
                        if let Some(pp) = path.parent() {
                            std::fs::create_dir_all(pp).ok();
                        }
                        let file_path = get_string(&path);
                        (
                            format!("{}.download", &file_path),
                            Some(format!("{}.digest", &file_path)),
                        )
                    };
                    if let Some(dp) = digest_path.as_ref() {
                        if Path::new(dp).exists() {
                            std::fs::remove_file(dp)?;
                        }
                    }
                    self.data_stream = Some(DataStream::FileStream(File::create(&path).await?));
                    if let Some(dp) = digest_path.as_ref() {
                        std::fs::write(dp, json!(self.digest).to_string()).ok();
                    }
                }
            }
            DataSource::MemoryCursor(c) => {
                if self.data_stream.is_none() {
                    self.data_stream = Some(DataStream::BufStream(TokioBufStream::new(c.clone())));
                }
            }
        }
        if block.compressed {
            let tmp = decompress(&block.data);
            self.data_stream
                .as_mut()
                .ok_or(anyhow!("data stream is None"))?
                .write_all(&tmp)
                .await?;
            self.finished_size += tmp.len() as u64;
        } else {
            self.data_stream
                .as_mut()
                .ok_or(anyhow!("file is None"))?
                .write_all(&block.data)
                .await?;
            self.finished_size += block.data.len() as u64;
        }
        self.transferred += block.data.len() as u64;
        Ok(())
    }

    #[inline]
    pub fn join(p: &PathBuf, name: &str) -> PathBuf {
        if name.is_empty() {
            p.clone()
        } else {
            p.join(name)
        }
    }

    /// Open the data stream for the current file.
    /// Returns Ok(true) if job is done, Ok(false) otherwise.
    pub(super) async fn open_data_stream(&mut self) -> ResultType<bool> {
        let file_num = self.file_num as usize;
        match &mut self.data_source {
            DataSource::FilePath(p) => {
                if file_num >= self.files.len() {
                    // job done
                    self.data_stream.take();
                    return Ok(true);
                };
                if self.data_stream.is_none() {
                    match File::open(Self::join(p, &self.files[file_num].name)).await {
                        Ok(file) => {
                            self.data_stream = Some(DataStream::FileStream(file));
                            self.file_confirmed = false;
                            self.file_is_waiting = false;
                        }
                        // On open error, behave the same as validation failure: advance
                        // to next file and return the error.
                        Err(err) => {
                            self.file_num += 1;
                            self.file_confirmed = false;
                            self.file_is_waiting = false;
                            return Err(err.into());
                        }
                    }
                }
            }
            DataSource::MemoryCursor(c) => {
                if self.data_stream.is_none() {
                    let mut t = std::io::Cursor::new(Vec::new());
                    std::mem::swap(&mut t, c);
                    self.data_stream = Some(DataStream::BufStream(TokioBufStream::new(t)));
                }
            }
        }
        Ok(false)
    }

    /// Get current file's digest (last_modified, file_size) for overwrite detection.
    pub(super) async fn get_current_digest(&self) -> ResultType<(u64, u64)> {
        let meta = match self.data_stream.as_ref().ok_or(anyhow!("file is None"))? {
            DataStream::FileStream(file) => file.metadata().await?,
            DataStream::BufStream(_) => bail!("No digest for buf stream"),
        };
        let last_modified = meta
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        Ok((last_modified, meta.len()))
    }

    pub(super) async fn init_data_stream<S: MsgSink + ?Sized>(&mut self, stream: &mut S) -> ResultType<()> {
        if let Some((last_modified, file_size)) = self.init_data_stream_for_cm().await? {
            let mut response = FileResponse::new();
            response.set_digest(FileTransferDigest { id: self.id, file_num: self.file_num,
                last_modified, file_size, is_resume: self.is_resume, ..Default::default() });
            let mut message = Message::new(); message.set_file_response(response);
            stream.send_msg(message).await?;
        }
        for digest in self.prefetch_digests().await? {
            let mut response = FileResponse::new(); response.set_digest(digest);
            let mut message = Message::new(); message.set_file_response(response);
            stream.send_msg(message).await?;
        }
        Ok(())
    }
}
