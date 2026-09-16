use hbb_common::{
    anyhow,
    bytes::{Bytes, BytesMut},
    bytes_codec::BytesCodec,
    config, log,
    tcp::{DynTcpStream, FramedStream},
    tokio::{self, net::UdpSocket, sync::mpsc, sync::oneshot},
    tokio_util, ResultType, Stream,
};
use kcp_sys::{
    endpoint::{ConnId, KcpEndpoint},
    packet_def::{KcpPacket, KcpPacketHeader},
    stream,
};
use std::{net::SocketAddr, sync::Arc};

pub struct KcpStream {
    endpoint: KcpEndpoint,
    conn_id: ConnId,
    stop_sender: Option<oneshot::Sender<()>>,
}

const KCP_IO_ERR_LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);
static KCP_SEND_ERR_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(KCP_IO_ERR_LOG_INTERVAL);
static KCP_RECV_ERR_LOG: hbb_common::log_throttle::LogThrottle =
    hbb_common::log_throttle::LogThrottle::new(KCP_IO_ERR_LOG_INTERVAL);

mod kcp_impl;
#[cfg(test)]
mod tests;
