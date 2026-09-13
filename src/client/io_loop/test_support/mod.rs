//! Test-only construction of a `Remote` so the peer message handlers can be
//! driven without a rendezvous server, a UI or a remote peer.

use super::*;
use hbb_common::tokio::net::{TcpListener, TcpStream};

mod recording_ui;
pub(crate) use recording_ui::RecordingUi;

pub(super) struct RemoteTestParts {
    pub(super) remote: Remote<RecordingUi>,
    /// The stream `handle_msg_from_peer` writes replies to.
    pub(super) peer: Stream,
    /// The other end of `peer`: what the remote peer would receive.
    pub(super) far_end: Stream,
    /// Keeps the outgoing sender connected; what the io loop hands to it.
    pub(super) _rx_data: mpsc::UnboundedReceiver<Data>,
    /// What the io loop hands to the audio thread.
    pub(super) rx_media: std::sync::mpsc::Receiver<MediaData>,
}

impl Remote<RecordingUi> {
    /// Build a remote over a loopback TCP pair, a recording UI and in-memory
    /// channels in place of the audio thread and the UI sender.
    pub(super) async fn for_test() -> RemoteTestParts {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind loopback");
        let addr = listener.local_addr().expect("local addr");
        let client = TcpStream::connect(addr).await.expect("connect loopback");
        let (server_side, peer_addr) = listener.accept().await.expect("accept loopback");
        let peer = Stream::from(server_side, peer_addr);
        let far_end = Stream::from(client, addr);

        let (sender, rx_data) = mpsc::unbounded_channel::<Data>();
        let (_tx_to_loop, receiver) = mpsc::unbounded_channel::<Data>();
        let (audio_sender, rx_media) = std::sync::mpsc::channel();
        let handler: Session<RecordingUi> = Session {
            server_keyboard_enabled: Arc::new(RwLock::new(true)),
            server_file_transfer_enabled: Arc::new(RwLock::new(true)),
            server_clipboard_enabled: Arc::new(RwLock::new(true)),
            ..Default::default()
        };
        let remote = Remote {
            handler,
            audio_sender,
            receiver,
            sender,
            read_jobs: Vec::new(),
            write_jobs: Vec::new(),
            remove_jobs: Default::default(),
            timer: crate::rustdesk_interval(time::interval(SEC30)),
            last_update_jobs_status: (Instant::now(), Default::default()),
            is_connected: false,
            first_frame: false,
            #[cfg(any(target_os = "windows", feature = "unix-file-copy-paste"))]
            client_conn_id: 0,
            data_count: Arc::new(AtomicUsize::new(0)),
            video_format: CodecFormat::Unknown,
            stop_voice_call_sender: None,
            voice_call_request_timestamp: None,
            elevation_requested: false,
            peer_info: Default::default(),
            video_threads: Default::default(),
            chroma: Default::default(),
            last_record_state: false,
            sent_close_reason: false,
        };
        RemoteTestParts {
            remote,
            peer,
            far_end,
            _rx_data: rx_data,
            rx_media,
        }
    }
}

/// Serialize a message the way the peer would send it.
pub(super) fn bytes_of(msg: &Message) -> Vec<u8> {
    msg.write_to_bytes().expect("serialize message")
}

/// The next message the peer would receive, or a panic after `ms`.
pub(super) async fn next_message(far_end: &mut Stream) -> Message {
    try_next_message(far_end, 3_000)
        .await
        .expect("no message within 3 s")
}

/// Like `next_message`, but `None` when nothing arrives within `ms`.
pub(super) async fn try_next_message(far_end: &mut Stream, ms: u64) -> Option<Message> {
    let bytes = far_end.next_timeout(ms).await?.expect("stream error");
    Some(Message::parse_from_bytes(&bytes).expect("a protobuf message"))
}

#[cfg(test)]
mod smoke {
    use super::*;

    #[tokio::test]
    async fn a_cursor_id_reaches_the_ui() {
        let mut parts = Remote::<RecordingUi>::for_test().await;
        let mut msg = Message::new();
        msg.set_cursor_id(42);
        let keep_open = parts
            .remote
            .handle_msg_from_peer(&bytes_of(&msg), &mut parts.peer)
            .await;
        assert!(keep_open);
        assert_eq!(parts.remote.handler.calls(), vec!["set_cursor_id:42".to_owned()]);
    }
}
