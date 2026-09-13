//! The TCP half of the split. See the module header for what this stands on.

use bytes::{Bytes, BytesMut};
use hbb_common::{
    bytes_codec::BytesCodec,
    futures::{SinkExt, StreamExt},
    protobuf,
    tcp::{DynTcpStream, Encrypt, FramedStream},
    tokio::io::{AsyncRead, AsyncReadExt, ReadHalf, WriteHalf},
    tokio_util::codec::{Framed, FramedRead, FramedWrite},
    ResultType,
};
use std::io::{Cursor, Error};

/// The read side, with whatever had already been pulled off the socket put back in front
/// of it. Boxed because the chaining adaptor's type is not nameable from outside tokio.
type ReadSide = Box<dyn AsyncRead + Send + Sync + Unpin>;

pub struct TcpReader {
    framed: FramedRead<ReadSide, BytesCodec>,
    /// Only the receive counter of this copy is ever advanced.
    decrypt: Option<Encrypt>,
}

pub struct TcpWriter {
    framed: FramedWrite<WriteHalf<DynTcpStream>, BytesCodec>,
    /// Only the send counter of this copy is ever advanced.
    encrypt: Option<Encrypt>,
    send_timeout: u64,
}

/// Take a `FramedStream` apart into two halves that do not share a lock.
///
/// The split is taken at the byte-stream level rather than at the framed layer, because
/// splitting the framed layer through its sink and stream halves puts both behind one
/// lock — which would reinstate exactly the blocking this is for.
pub(super) fn split(stream: FramedStream) -> Result<(TcpReader, TcpWriter), FramedStream> {
    let FramedStream(framed, addr, key, send_timeout) = stream;
    let parts = framed.into_parts();
    if !parts.write_buf.is_empty() {
        // Bytes are half-written to the socket. Splitting here would either lose them or
        // interleave them with whatever the writer sends first, and neither is worth a
        // recovery path: it cannot happen at the one point the caller splits, which is
        // before the loop starts. Hand the stream back untouched instead of guessing.
        return Err(FramedStream(
            Framed::from_parts(parts),
            addr,
            key,
            send_timeout,
        ));
    }
    let (read_io, write_io) = hbb_common::tokio::io::split(parts.io);

    // Bytes already pulled off the socket but not yet framed. Dropping them would lose
    // whatever arrived in the same packet as the last message read before the split, so
    // they are read out first and the socket only after.
    let buffered = Cursor::new(parts.read_buf.to_vec());

    Ok((
        TcpReader {
            framed: FramedRead::new(Box::new(buffered.chain(read_io)) as ReadSide, BytesCodec::new()),
            decrypt: key.clone(),
        },
        TcpWriter {
            framed: FramedWrite::new(write_io, BytesCodec::new()),
            encrypt: key,
            send_timeout,
        },
    ))
}

impl TcpReader {
    pub async fn next(&mut self) -> Option<Result<BytesMut, Error>> {
        let mut res = self.framed.next().await;
        if let Some(Ok(bytes)) = res.as_mut() {
            if let Some(key) = self.decrypt.as_mut() {
                if let Err(err) = key.dec(bytes) {
                    return Some(Err(err));
                }
            }
        }
        res
    }
}

impl TcpWriter {
    pub async fn send(&mut self, msg: &impl protobuf::Message) -> ResultType<()> {
        self.send_raw(msg.write_to_bytes()?).await
    }

    pub async fn send_raw(&mut self, msg: Vec<u8>) -> ResultType<()> {
        let mut msg = msg;
        if let Some(key) = self.encrypt.as_mut() {
            msg = key.enc(&msg);
        }
        self.send_bytes(Bytes::from(msg)).await
    }

    pub async fn send_bytes(&mut self, bytes: Bytes) -> ResultType<()> {
        if self.send_timeout > 0 {
            hbb_common::timeout(self.send_timeout, self.framed.send(bytes)).await??;
        } else {
            self.framed.send(bytes).await?;
        }
        Ok(())
    }

    pub fn set_send_timeout(&mut self, ms: u64) {
        self.send_timeout = ms;
    }
}
