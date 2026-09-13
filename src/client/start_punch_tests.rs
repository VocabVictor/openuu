use super::start_relay_tests::{rendezvous_pair, NoUi};
use super::*;

fn ctx(my_addr: SocketAddr) -> StartCtx<'static, NoUi> {
    StartCtx {
        peer: "123456789",
        key: "",
        token: "",
        conn_type: ConnType::DEFAULT_CONN,
        interface: NoUi,
        rendezvous_server: "rs.example.com:21116",
        my_addr,
        start: Instant::now(),
    }
}

fn state() -> PunchState {
    PunchState {
        peer_nat_type: NatType::UNKNOWN_NAT,
        is_local: false,
        signed_id_pk: Vec::new(),
        relay_server: String::new(),
        peer_addr: Config::get_any_listen_addr(true),
        feedback: 0,
        webrtc_sdp_answer: String::new(),
        pending_webrtc_ice: Vec::new(),
    }
}

fn request() -> RendezvousMessage {
    let mut msg = RendezvousMessage::new();
    msg.set_punch_hole_request(PunchHoleRequest {
        id: "123456789".to_owned(),
        ..Default::default()
    });
    msg
}

fn response(ph: PunchHoleResponse) -> RendezvousMessage {
    let mut msg = RendezvousMessage::new();
    msg.set_punch_hole_response(ph);
    msg
}

fn punched() -> RendezvousMessage {
    response(PunchHoleResponse {
        socket_addr: AddrMangle::encode("198.51.100.20:21118".parse().unwrap()).into(),
        pk: b"pk".to_vec().into(),
        relay_server: "rs.example.com".to_owned(),
        union: Some(punch_hole_response::Union::IsLocal(true)),
        feedback: 5,
        webrtc_sdp_answer: "v=0".to_owned(),
        ..Default::default()
    })
}

fn ice(session_key: &str, candidate: &str) -> RendezvousMessage {
    let mut msg = RendezvousMessage::new();
    msg.set_ice_candidate(IceCandidate {
        session_key: session_key.to_owned(),
        candidate: candidate.to_owned(),
        ..Default::default()
    });
    msg
}

/// A rendezvous server that answers the first punch request with `replies` and then keeps
/// the connection open until the caller is done with it.
fn fake_rendezvous(mut far_end: Stream, replies: Vec<RendezvousMessage>) -> hbb_common::tokio::task::JoinHandle<PunchHoleRequest> {
    tokio::spawn(async move {
        let bytes = far_end.next().await.expect("a request").expect("a frame");
        let req = RendezvousMessage::parse_from_bytes(&bytes).unwrap();
        let Some(rendezvous_message::Union::PunchHoleRequest(ph)) = req.union else {
            panic!("expected a PunchHoleRequest");
        };
        for reply in replies {
            far_end.send(&reply).await.unwrap();
        }
        hbb_common::sleep(2.0).await;
        ph
    })
}

async fn run(replies: Vec<RendezvousMessage>, session_key: &str, state: &mut PunchState) -> ResultType<PunchOutcome<'static, NoUi>> {
    let (socket, far_end, my_addr) = rendezvous_pair().await;
    let server = fake_rendezvous(far_end, replies);
    let msg_out = request();
    let result = Client::punch_hole_attempts(
        socket,
        &msg_out,
        "TCP",
        0,
        &mut None,
        &mut None,
        &mut None,
        session_key,
        state,
        ctx(my_addr),
    )
    .await;
    assert_eq!(server.await.unwrap().id, "123456789");
    result
}

#[tokio::test]
async fn an_offline_peer_is_reported_as_such() {
    let reply = response(PunchHoleResponse {
        failure: punch_hole_response::Failure::OFFLINE.into(),
        ..Default::default()
    });
    let err = run(vec![reply], "", &mut state()).await.err().expect("an error");
    assert_eq!(err.to_string(), "Remote desktop is offline");
}

#[tokio::test]
async fn an_unknown_id_and_a_free_text_failure_are_reported() {
    let reply = response(PunchHoleResponse {
        failure: punch_hole_response::Failure::ID_NOT_EXIST.into(),
        ..Default::default()
    });
    let err = run(vec![reply], "", &mut state()).await.err().expect("an error");
    assert_eq!(err.to_string(), "ID does not exist");
    let reply = response(PunchHoleResponse {
        other_failure: "boom".to_owned(),
        ..Default::default()
    });
    let err = run(vec![reply], "", &mut state()).await.err().expect("an error");
    assert_eq!(err.to_string(), "boom");
}

#[tokio::test]
async fn a_punched_hole_fills_the_state_and_hands_the_socket_back() {
    let mut st = state();
    match run(vec![punched()], "", &mut st).await.unwrap() {
        PunchOutcome::Punched { ctx, .. } => assert_eq!(ctx.peer, "123456789"),
        PunchOutcome::Connected(_) => panic!("expected Punched"),
    }
    assert_eq!(st.peer_addr, "198.51.100.20:21118".parse().unwrap());
    assert_eq!(st.signed_id_pk, b"pk".to_vec());
    assert_eq!(st.relay_server, "rs.example.com");
    assert!(st.is_local);
    assert_eq!(st.feedback, 5);
    assert_eq!(st.webrtc_sdp_answer, "v=0");
    assert!(st.pending_webrtc_ice.is_empty());
}

#[tokio::test]
async fn ice_candidates_are_buffered_for_the_session_and_the_oldest_are_evicted() {
    let mut replies: Vec<_> = (1..=66).map(|i| ice("sk", &format!("c{i}"))).collect();
    replies.push(ice("other", "stray"));
    replies.push(ice("sk", ""));
    replies.push(punched());
    let mut st = state();
    assert!(matches!(
        run(replies, "sk", &mut st).await.unwrap(),
        PunchOutcome::Punched { .. }
    ));
    assert_eq!(st.pending_webrtc_ice.len(), Client::MAX_PENDING_WEBRTC_ICE);
    assert_eq!(st.pending_webrtc_ice.first().unwrap(), "c3");
    assert_eq!(st.pending_webrtc_ice.last().unwrap(), "c66");
}

#[tokio::test]
async fn candidates_without_a_session_key_and_unexpected_messages_are_ignored() {
    let mut other = RendezvousMessage::new();
    other.set_register_pk_response(RegisterPkResponse::default());
    let mut st = state();
    assert!(matches!(
        run(vec![ice("sk", "c1"), other, punched()], "", &mut st).await.unwrap(),
        PunchOutcome::Punched { .. }
    ));
    assert!(st.pending_webrtc_ice.is_empty());
}
