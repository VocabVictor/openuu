use super::*;
use hbb_common::tokio::{self, time::{sleep, Duration, Instant}};

    #[tokio::test]
    async fn test_udp_punch_deadline_survives_a_talkative_peer() {
        let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let (a_addr, b_addr) = (a.local_addr().unwrap(), b.local_addr().unwrap());
        a.connect(b_addr).await.unwrap();
        b.connect(a_addr).await.unwrap();
        // Empty datagrams answer no probe and match no return branch, so they only feed the loop.
        // Sent well past the punch deadline so a restarted timer would show up as a long run.
        let flooder = tokio::spawn(async move {
            let end = Instant::now() + Duration::from_secs(12);
            while Instant::now() < end {
                if b.send(&[]).await.is_err() {
                    break;
                }
                sleep(Duration::from_millis(5)).await;
            }
        });
        let start = Instant::now();
        let res = punch_udp(Arc::new(a), false).await;
        let elapsed = start.elapsed();
        flooder.abort();
        assert!(res.is_err(), "the punch should have timed out");
        assert!(
            elapsed < Duration::from_secs(6),
            "the punch ran for {elapsed:?}; its deadline did not hold"
        );
    }
