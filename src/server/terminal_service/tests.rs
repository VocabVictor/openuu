use super::{find_utf8_split_point, OutputBuffer, Utf8ChunkAccumulator, MAX_BUFFER_LINES};

#[test]
fn utf8_split_point_returns_full_len_for_complete_input() {
    assert_eq!(find_utf8_split_point(b"hello"), 5);
    assert_eq!(find_utf8_split_point("中文".as_bytes()), "中文".len());
    assert_eq!(find_utf8_split_point("😀".as_bytes()), "😀".len());
}

#[test]
fn utf8_split_point_detects_incomplete_trailing_sequence() {
    let data = [b'a', 0xE4, 0xB8];
    assert_eq!(find_utf8_split_point(&data), 1);
}

#[test]
fn utf8_split_point_keeps_malformed_prefix_but_buffers_trailing_lead_byte() {
    let data = [0xFF, 0xE4];
    assert_eq!(find_utf8_split_point(&data), 1);
}

#[test]
fn utf8_split_point_treats_orphan_continuations_as_complete() {
    let data = [0x80, 0x81, 0x82];
    assert_eq!(find_utf8_split_point(&data), data.len());
}

#[test]
fn utf8_chunk_accumulator_reassembles_split_multibyte_output() {
    let full = "你好世界".as_bytes();
    let mut chunker = Utf8ChunkAccumulator::default();
    let mut output = Vec::new();

    for chunk in full.chunks(5) {
        if let Some(data) = chunker.push_chunk(chunk.to_vec()) {
            output.extend_from_slice(&data);
        }
    }

    if let Some(data) = chunker.finish() {
        output.extend_from_slice(&data);
    }

    assert_eq!(output, full);
}

#[test]
fn utf8_chunk_accumulator_buffers_leading_split_multibyte_output() {
    let mut chunker = Utf8ChunkAccumulator::default();

    assert!(chunker.push_chunk(vec![0xE4]).is_none());
    assert!(chunker.push_chunk(vec![0xB8]).is_none());
    assert_eq!(
        chunker.push_chunk(vec![0xAD]),
        Some("中".as_bytes().to_vec())
    );
    assert!(chunker.finish().is_none());
}

#[test]
fn utf8_chunk_accumulator_flushes_incomplete_tail_on_finish() {
    let mut chunker = Utf8ChunkAccumulator::default();
    assert_eq!(chunker.push_chunk(vec![b'a', 0xE4]), Some(vec![b'a']));
    assert_eq!(chunker.finish(), Some(vec![0xE4]));
    assert!(chunker.finish().is_none());
}

#[test]
fn utf8_chunk_accumulator_does_not_stall_on_malformed_bytes() {
    let mut chunker = Utf8ChunkAccumulator::default();
    assert_eq!(chunker.push_chunk(vec![0xFF]), Some(vec![0xFF]));
    assert!(chunker.finish().is_none());
}

#[test]
fn utf8_chunk_accumulator_buffers_lone_utf8_lead_bytes() {
    let mut chunker = Utf8ChunkAccumulator::default();
    assert!(chunker.push_chunk(vec![0xE4]).is_none());
    assert_eq!(chunker.finish(), Some(vec![0xE4]));
}

#[test]
fn utf8_chunk_accumulator_does_not_hold_back_non_utf8_prefixes() {
    let mut chunker = Utf8ChunkAccumulator::default();
    assert_eq!(chunker.push_chunk(vec![0xFF, 0xE4]), Some(vec![0xFF, 0xE4]));
    assert!(chunker.finish().is_none());
}

#[test]
fn output_buffer_trim_after_incomplete_merge_does_not_underflow() {
    let mut buffer = OutputBuffer::new();

    // Create an incomplete line first.
    buffer.append(b"hello");

    // Merge a large chunk that contains the first newline at the tail.
    // This exercises the "append to last incomplete line" branch.
    let mut large = vec![b'a'; 30_000];
    large.push(b'\n');
    buffer.append(&large);

    // Exceed MAX_BUFFER_LINES so trim pops the first large merged line.
    for _ in 0..=MAX_BUFFER_LINES {
        buffer.append(b"x\n");
    }

    let actual_size: usize = buffer.lines.iter().map(|line| line.len()).sum();
    assert_eq!(buffer.total_size, actual_size);
}
