use super::*;

/// Output buffer for terminal session
pub(super) struct OutputBuffer {
    pub(super) lines: VecDeque<Vec<u8>>,
    pub(super) total_size: usize,
    pub(super) last_line_incomplete: bool,
}

impl OutputBuffer {
    pub(super) fn new() -> Self {
        Self {
            lines: VecDeque::new(),
            total_size: 0,
            last_line_incomplete: false,
        }
    }

    pub(super) fn append(&mut self, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        // Handle incomplete lines
        let mut start = 0;
        if self.last_line_incomplete {
            if let Some(last_line) = self.lines.back_mut() {
                // Find first newline in new data
                if let Some(newline_pos) = data.iter().position(|&b| b == b'\n') {
                    last_line.extend_from_slice(&data[..=newline_pos]);
                    self.total_size += newline_pos + 1;
                    start = newline_pos + 1;
                    self.last_line_incomplete = false;
                } else {
                    // Still no newline, append all
                    last_line.extend_from_slice(data);
                    self.total_size += data.len();
                    return;
                }
            }
        }

        // Process remaining data
        let remaining = &data[start..];
        let ends_with_newline = remaining.last() == Some(&b'\n');

        // Split by lines
        let lines: Vec<&[u8]> = remaining.split(|&b| b == b'\n').collect();

        for (i, line) in lines.iter().enumerate() {
            if i == lines.len() - 1 && !ends_with_newline && !line.is_empty() {
                // Last line without newline
                self.last_line_incomplete = true;
            }

            if !line.is_empty() || i < lines.len() - 1 {
                let mut line_data = line.to_vec();
                if i < lines.len() - 1 || ends_with_newline {
                    line_data.push(b'\n');
                }

                self.total_size += line_data.len();
                self.lines.push_back(line_data);
            }
        }

        // Trim old data if buffer is too large
        while self.total_size > MAX_OUTPUT_BUFFER_SIZE || self.lines.len() > MAX_BUFFER_LINES {
            if let Some(removed) = self.lines.pop_front() {
                if removed.len() > self.total_size {
                    log::error!(
                        "OutputBuffer total_size underflow avoided: total_size={}, removed_len={}, lines_len={}",
                        self.total_size,
                        removed.len(),
                        self.lines.len()
                    );
                    self.total_size = self.lines.iter().map(|line| line.len()).sum();
                } else {
                    self.total_size -= removed.len();
                }
                if self.lines.is_empty() {
                    self.last_line_incomplete = false;
                }
            } else {
                log::error!(
                    "OutputBuffer trim invariant broken: total_size={}, lines_len=0",
                    self.total_size
                );
                self.total_size = 0;
                self.last_line_incomplete = false;
                break;
            }
        }
    }

    pub(super) fn get_recent(&self, max_bytes: usize) -> Vec<u8> {
        if max_bytes == 0 {
            return Vec::new();
        }
        let mut chunks: Vec<&[u8]> = Vec::new();
        let mut size = 0;

        // Collect whole chunks from newest to oldest, preserving chronological continuity.
        // If the newest chunk alone exceeds max_bytes, take its tail (truncation may split
        // an ANSI escape, but the terminal will self-correct on subsequent output).
        for line in self.lines.iter().rev() {
            if size + line.len() > max_bytes {
                if size == 0 && line.len() > max_bytes {
                    // Single oversized chunk: take the tail to preserve the most recent content.
                    // Align offset forward to a UTF-8 char boundary so that downstream
                    // clients (e.g. Dart) that decode the payload as UTF-8 text don't
                    // encounter split code points. The protobuf bytes field itself allows
                    // arbitrary bytes; this is a best-effort mitigation for client-side decoding.
                    let mut offset = line.len() - max_bytes;
                    // Skip at most 3 continuation bytes (UTF-8 max 4-byte sequence).
                    // Prevents runaway skipping on non-UTF-8 binary data.
                    let mut skipped = 0u8;
                    while skipped < 3
                        && offset < line.len()
                        && (line[offset] & 0b1100_0000) == 0b1000_0000
                    {
                        offset += 1;
                        skipped += 1;
                    }
                    // If we skipped past all remaining bytes (degenerate data), drop the
                    // chunk entirely rather than emitting a slice that decodes poorly on the client.
                    if offset < line.len() {
                        chunks.push(&line[offset..]);
                        size = line.len() - offset;
                    }
                }
                break;
            }
            size += line.len();
            chunks.push(line);
        }

        // Reverse to restore chronological order and concatenate
        chunks.reverse();
        let mut result = Vec::with_capacity(size);
        for chunk in chunks {
            result.extend_from_slice(chunk);
        }

        result
    }
}

/// Find the largest prefix of `buf` that does not end in the middle of a UTF-8
/// code point. Invalid bytes are treated as complete so they can continue
/// downstream and be rendered with replacement characters if needed.
pub(super) fn find_utf8_split_point(buf: &[u8]) -> usize {
    if buf.is_empty() {
        return 0;
    }

    let start = buf.len().saturating_sub(3);
    for i in (start..buf.len()).rev() {
        let b = buf[i];
        if b & 0x80 == 0 {
            return buf.len();
        }
        if b & 0xC0 == 0x80 {
            continue;
        }

        let seq_len = if b & 0xE0 == 0xC0 {
            2
        } else if b & 0xF0 == 0xE0 {
            3
        } else if b & 0xF8 == 0xF0 {
            4
        } else {
            return buf.len();
        };

        return if buf.len() - i >= seq_len {
            buf.len()
        } else {
            i
        };
    }

    buf.len()
}

// Terminal output currently follows a UTF-8 text model end to end: the service
// keeps replay buffers on UTF-8 boundaries, and Flutter decodes payload bytes as
// UTF-8 before writing to xterm. This accumulator only prevents splitting a
// trailing UTF-8 code point across PTY reads. Supporting non-UTF-8 terminals
// would need a separate design covering remote encoding detection, Flutter
// decoding, replay truncation, and input transcoding.
#[derive(Default)]
pub(super) struct Utf8ChunkAccumulator {
    pub(super) remainder: Vec<u8>,
}

impl Utf8ChunkAccumulator {
    pub(super) fn push_chunk(&mut self, mut data: Vec<u8>) -> Option<Vec<u8>> {
        if data.is_empty() {
            return None;
        }

        let had_remainder = !self.remainder.is_empty();
        if had_remainder {
            let mut combined = std::mem::take(&mut self.remainder);
            combined.extend_from_slice(&data);
            data = combined;
        }

        let split = find_utf8_split_point(&data);
        if split == data.len() {
            return Some(data);
        }

        // Only hold back a candidate incomplete suffix when we have evidence that
        // the bytes before it are already UTF-8 text. If split is 0, the whole
        // read may be the start of a UTF-8 character, so keep it for the next read.
        if !had_remainder && split > 0 && std::str::from_utf8(&data[..split]).is_err() {
            return Some(data);
        }

        self.remainder = data.split_off(split);
        if data.is_empty() {
            None
        } else {
            Some(data)
        }
    }

    pub(super) fn finish(&mut self) -> Option<Vec<u8>> {
        if self.remainder.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.remainder))
        }
    }
}

/// Try to send data through the output channel with rate-limited drop logging.
/// Returns `true` if the caller should break out of the read loop (channel disconnected).
pub(super) fn try_send_output(
    output_tx: &mpsc::SyncSender<Vec<u8>>,
    data: Vec<u8>,
    terminal_id: i32,
    label: &str,
    drop_count: &mut u64,
    last_drop_warn: &mut Instant,
) -> bool {
    match output_tx.try_send(data) {
        Ok(_) => {
            if *drop_count > 0 {
                log::trace!(
                    "Terminal {}{} output channel recovered, dropped {} chunks since last report",
                    terminal_id,
                    label,
                    *drop_count
                );
                *drop_count = 0;
            }
            false
        }
        Err(mpsc::TrySendError::Full(_)) => {
            *drop_count += 1;
            if last_drop_warn.elapsed() >= Duration::from_secs(5) {
                log::trace!(
                    "Terminal {}{} output channel full, dropped {} chunks in last {:?}",
                    terminal_id,
                    label,
                    *drop_count,
                    last_drop_warn.elapsed()
                );
                *drop_count = 0;
                *last_drop_warn = Instant::now();
            }
            false
        }
        Err(mpsc::TrySendError::Disconnected(_)) => {
            log::debug!(
                "Terminal {}{} output channel disconnected",
                terminal_id,
                label
            );
            true
        }
    }
}
