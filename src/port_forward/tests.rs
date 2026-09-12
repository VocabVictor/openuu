use super::*;

#[test]
fn peer_supports_mux_reads_the_features_bit() {
    let mut pi = PeerInfo::new();
    assert!(!peer_supports_mux(&pi));
    pi.features = Some(Features { port_forward_mux: false, ..Default::default() }).into();
    assert!(!peer_supports_mux(&pi));
    pi.features = Some(Features { port_forward_mux: true, ..Default::default() }).into();
    assert!(peer_supports_mux(&pi));
}

#[test]
fn port_forward_mux_defaults_to_on() {
    use hbb_common::config::option2bool;
    use base::config::keys;
    // option2bool's fallback branch is also "on unless N", so the value
    // assertions below would pass for a prefixless key too. The `enable-`
    // prefix is what actually guarantees the default, and renaming the key
    // to an `allow-` one would silently flip it — pin the prefix itself.
    assert!(keys::OPTION_ENABLE_PORT_FORWARD_MUX.starts_with("enable-"));
    assert!(option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, ""));
    assert!(option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, "Y"));
    assert!(!option2bool(keys::OPTION_ENABLE_PORT_FORWARD_MUX, "N"));
}

#[test]
fn take_socket_hands_back_a_working_socket_and_the_prebuf() {
    use hbb_common::tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let mut client = TcpStream::connect(addr).await.unwrap();
        let (server, _) = l.accept().await.unwrap();
        let mut framed = Framed::new(server, BytesCodec::new());
        client.write_all(b"abc").await.unwrap();
        // Read through the codec, as connect_and_login does during login.
        let pulled = framed.next().await.unwrap().unwrap();
        assert_eq!(&pulled[..], b"abc");
        let (mut sock, prebuf) = take_socket(framed, pulled.to_vec());
        assert_eq!(prebuf, b"abc".to_vec());
        // Bytes written after the handoff arrive on the bare socket.
        client.write_all(b"def").await.unwrap();
        let mut buf = [0u8; 3];
        sock.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"def");
    });
}
