use hbb_common::{
    anyhow::bail,
    config::{Config, CONNECT_TIMEOUT, READ_TIMEOUT},
    log,
    rendezvous_proto::*,
    sleep,
    socket_client::connect_tcp,
    ResultType, Stream,
};

pub async fn query_online_states<F: FnOnce(Vec<String>, Vec<String>)>(ids: Vec<String>, f: F) {
    let test = false;
    if test {
        sleep(1.5).await;
        let mut onlines = ids;
        let offlines = onlines.drain((onlines.len() / 2)..).collect();
        f(onlines, offlines)
    } else {
        let query_timeout = std::time::Duration::from_millis(3_000);
        match query_online_states_(&ids, query_timeout).await {
            Ok((onlines, offlines)) => {
                f(onlines, offlines);
            }
            Err(e) => {
                log::debug!("query onlines, {}", &e);
            }
        }
    }
}

async fn create_online_stream() -> ResultType<Stream> {
    let (rendezvous_server, _servers, _contained) =
        crate::get_rendezvous_server(READ_TIMEOUT).await;
    let tmp: Vec<&str> = rendezvous_server.split(":").collect();
    if tmp.len() != 2 {
        bail!("Invalid server address: {}", rendezvous_server);
    }
    let port: u16 = tmp[1].parse()?;
    if port == 0 {
        bail!("Invalid server address: {}", rendezvous_server);
    }
    let online_server = format!("{}:{}", tmp[0], port - 1);
    connect_tcp(online_server, CONNECT_TIMEOUT).await
}

async fn query_online_states_(
    ids: &Vec<String>,
    timeout: std::time::Duration,
) -> ResultType<(Vec<String>, Vec<String>)> {
    let mut msg_out = RendezvousMessage::new();
    msg_out.set_online_request(OnlineRequest {
        id: Config::get_id(),
        peers: ids.clone(),
        ..Default::default()
    });

    let mut socket = match create_online_stream().await {
        Ok(s) => s,
        Err(e) => {
            log::debug!("Failed to create peers online stream, {e}");
            return Ok((vec![], ids.clone()));
        }
    };
    // TODO: Use long connections to avoid socket creation
    // If we use a Arc<Mutex<Option<FramedStream>>> to hold and reuse the previous socket,
    // we may face the following error:
    // An established connection was aborted by the software in your host machine. (os error 10053)
    if let Err(e) = socket.send(&msg_out).await {
        log::debug!("Failed to send peers online states query, {e}");
        return Ok((vec![], ids.clone()));
    }
    // Retry for 2 times to get the online response
    for _ in 0..2 {
        if let Some(msg_in) =
            crate::get_next_nonkeyexchange_msg(&mut socket, Some(timeout.as_millis() as _))
                .await
        {
            match msg_in.union {
                Some(rendezvous_message::Union::OnlineResponse(online_response)) => {
                    let states = online_response.states;
                    let mut onlines = Vec::new();
                    let mut offlines = Vec::new();
                    for i in 0..ids.len() {
                        // bytes index from left to right
                        let bit_value = 0x01 << (7 - i % 8);
                        if (states[i / 8] & bit_value) == bit_value {
                            onlines.push(ids[i].clone());
                        } else {
                            offlines.push(ids[i].clone());
                        }
                    }
                    return Ok((onlines, offlines));
                }
                _ => {
                    // ignore
                }
            }
        } else {
            // TODO: Make sure socket closed?
            bail!("Online stream receives None");
        }
    }

    bail!("Failed to query online states, no online response");
}

#[cfg(test)]
mod tests {
    use crate::client::Client;
    use hbb_common::rendezvous_proto::IceCandidate;
    use hbb_common::tokio;

    #[test]
    fn accepts_webrtc_ice_by_session_key_only() {
        let ice = IceCandidate {
            session_key: "session-a".to_owned(),
            candidate: "candidate-json".to_owned(),
            ..Default::default()
        };

        assert!(Client::is_expected_webrtc_ice_candidate(&ice, "session-a"));
        assert!(!Client::is_expected_webrtc_ice_candidate(&ice, "session-b"));
    }

    #[tokio::test]
    async fn test_query_onlines() {
        super::query_online_states(
            vec![
                "152183996".to_owned(),
                "165782066".to_owned(),
                "155323351".to_owned(),
                "460952777".to_owned(),
            ],
            |onlines: Vec<String>, offlines: Vec<String>| {
                println!("onlines: {:?}, offlines: {:?}", &onlines, &offlines);
            },
        )
        .await;
    }
}
