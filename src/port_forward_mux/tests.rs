use super::*;

mod channel;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod tunnel;

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn effective_window_clamps_to_initial() {
    assert_eq!(effective_window(0), INITIAL_WINDOW);
    assert_eq!(effective_window(INITIAL_WINDOW - 1), INITIAL_WINDOW);
    assert_eq!(effective_window(INITIAL_WINDOW), INITIAL_WINDOW);
    assert_eq!(effective_window(CHANNEL_WINDOW), CHANNEL_WINDOW);
}

#[test]
fn recv_window_rejects_over_window_data() {
    // `new` clamps its grant to INITIAL_WINDOW, so the test must fill that.
    let mut w = RecvWindow::new(INITIAL_WINDOW);
    assert!(w.accept(INITIAL_WINDOW as usize - MIN_FRAME_CHARGE as usize));
    assert!(w.accept(MIN_FRAME_CHARGE as usize));
    assert!(!w.accept(1));
}

#[test]
fn tiny_frames_are_charged_at_the_minimum() {
    let mut w = RecvWindow::new(INITIAL_WINDOW);
    for _ in 0..(INITIAL_WINDOW / MIN_FRAME_CHARGE) {
        assert!(w.accept(1));
    }
    // A 64 KiB window holds 1024 one-byte frames, not 65536 of them.
    assert!(!w.accept(1));
}

#[test]
fn recv_window_updates_only_past_threshold() {
    let mut w = RecvWindow::new(CHANNEL_WINDOW);
    assert_eq!(
        w.drained(UPDATE_THRESHOLD as usize - MIN_FRAME_CHARGE as usize),
        None
    );
    assert_eq!(w.drained(MIN_FRAME_CHARGE as usize), Some(UPDATE_THRESHOLD));
    // The update re-grants what was drained, so the same amount is accepted again.
    assert!(w.accept(CHANNEL_WINDOW as usize));
    assert!(w.accept(UPDATE_THRESHOLD as usize));
    assert!(!w.accept(1));
}

#[test]
fn accounting_survives_a_transfer_far_larger_than_the_window() {
    // Cumulative counters used to overflow around 4 GiB on one channel and
    // read as a protocol violation mid-transfer.
    let mut w = RecvWindow::new(CHANNEL_WINDOW);
    let mut moved: u64 = 0;
    while moved < 8 * 1024 * 1024 * 1024 {
        assert!(w.accept(MAX_FRAME));
        w.drained(MAX_FRAME);
        moved += MAX_FRAME as u64;
    }
}

#[test]
fn grant_extends_a_window_that_has_been_used_up() {
    let mut w = RecvWindow::new(INITIAL_WINDOW);
    assert!(w.accept(INITIAL_WINDOW as usize));
    assert!(!w.accept(1));
    w.grant(CHANNEL_WINDOW - INITIAL_WINDOW);
    assert!(w.accept((CHANNEL_WINDOW - INITIAL_WINDOW) as usize));
    assert!(!w.accept(1));
}

#[test]
fn send_credit_blocks_at_zero_and_resumes_on_add() {
    rt().block_on(async {
        let credit = std::sync::Arc::new(SendCredit::new(MIN_FRAME_CHARGE + 4));
        assert_eq!(
            credit.take(MAX_FRAME).await,
            (MIN_FRAME_CHARGE + 4) as usize
        );
        let c = credit.clone();
        let waiter = tokio::spawn(async move { c.take(MAX_FRAME).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());
        // Below one minimum charge the taker stays parked: whatever it
        // reads next has to be payable.
        credit.add(MIN_FRAME_CHARGE - 1);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());
        credit.add(1);
        assert_eq!(waiter.await.unwrap(), MIN_FRAME_CHARGE as usize);
    });
}

#[test]
fn send_credit_is_capped_whatever_the_peer_advertises() {
    rt().block_on(async {
        let credit = SendCredit::new(u32::MAX);
        assert_eq!(credit.take(usize::MAX).await, MAX_SEND_CREDIT as usize);
        // A flood of window updates cannot lift it past the cap either.
        for _ in 0..10 {
            credit.add(u32::MAX);
        }
        assert_eq!(credit.take(usize::MAX).await, MAX_SEND_CREDIT as usize);
    });
}

#[test]
fn raise_initial_rebases_credit_from_initial_window() {
    rt().block_on(async {
        let credit = SendCredit::new(INITIAL_WINDOW);
        assert_eq!(credit.take(1000).await, 1000);
        credit.raise_initial(CHANNEL_WINDOW);
        // Credit is now CHANNEL_WINDOW - 1000, not CHANNEL_WINDOW - 1000 + INITIAL_WINDOW.
        assert_eq!(
            credit.take(usize::MAX).await,
            (CHANNEL_WINDOW - 1000) as usize
        );
        credit.raise_initial(0);
        // A zero or sub-INITIAL_WINDOW advertisement adds nothing.
        let c = std::sync::Arc::new(credit);
        let c2 = c.clone();
        let waiter = tokio::spawn(async move { c2.take(MAX_FRAME).await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!waiter.is_finished());
        c.add(MIN_FRAME_CHARGE);
        assert_eq!(waiter.await.unwrap(), MIN_FRAME_CHARGE as usize);
    });
}
