//! An inert socket, used only as the thing left behind when a real one is taken out.
//!
//! `Stream` implements `Drop`, so its variants cannot be destructured; the socket has to be
//! swapped out through a `&mut`, and a swap needs something to put in. Nothing ever reads
//! or writes this: the husk holding it is dropped immediately.

use hbb_common::{
    bytes_codec::BytesCodec,
    tcp::{DynTcpStream, FramedStream},
    tokio::io::{AsyncRead, AsyncWrite, ReadBuf},
    tokio_util::codec::Framed,
};
use std::{
    io::Result as IoResult,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    pin::Pin,
    task::{Context, Poll},
};

struct NullIo;

impl AsyncRead for NullIo {
    fn poll_read(self: Pin<&mut Self>, _: &mut Context<'_>, _: &mut ReadBuf<'_>) -> Poll<IoResult<()>> {
        // End of stream, so anything that did read it would stop rather than hang.
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for NullIo {
    fn poll_write(self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8]) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<IoResult<()>> {
        Poll::Ready(Ok(()))
    }
}

pub(super) fn null_framed_stream() -> FramedStream {
    let addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0));
    FramedStream(
        Framed::new(DynTcpStream(Box::new(NullIo)), BytesCodec::new()),
        addr,
        None,
        0,
    )
}
