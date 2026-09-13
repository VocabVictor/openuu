//! How fast the bitrate comes back once a confirmed cut has cleared.
use super::recovery::{second, session};

#[test]
fn a_cut_recovers_exponentially_within_half_a_minute() {
    let mut qos = session(true);
    for _ in 0..90 {
        second(&mut qos, 10, true);
    }
    let target = qos.latest_quality().ratio();
    assert_eq!(qos.ratio(), target);
    for _ in 0..12 {
        second(&mut qos, 800, true);
    }
    let after_cut = qos.ratio();
    assert!(after_cut < target * 0.5, "fixture must cut: {after_cut}");
    let mut back = None;
    for s in 1..=90u32 {
        second(&mut qos, 10, true);
        assert!(qos.ratio() <= target + 1e-4, "never above the quality: {}", qos.ratio());
        if qos.ratio() >= target * 0.9 {
            back = Some(s);
            break;
        }
    }
    // From a quarter of the target, 15 percent a 3 s window needs ten windows; the
    // fixed 150 kbps step (modeled bitrate 6000 kbps) needed seventeen.
    assert!(matches!(back, Some(s) if s <= 30), "{back:?}");
}
