use super::*;
use base::message_proto::{video_frame, Misc, SwitchDisplay, VideoFrame};
use hbb_common::{tcp::FramedStream, Stream};

const MS: std::time::Duration = std::time::Duration::from_millis(1);

/// A connection whose socket holds a single byte, so the first write blocks and everything
/// queued after it can be arranged before anything drains.
fn blocked_pair() -> (Writer, FramedStream) {
    let (ours, theirs) = hbb_common::tokio::io::duplex(1);
    let addr = "127.0.0.1:0".parse().unwrap();
    let (_reader, out) = crate::stream_split::split(Stream::Tcp(FramedStream::from(ours, addr)))
        .unwrap_or_else(|_| panic!("a TCP stream must be splittable"));
    (Writer::start(out), FramedStream::from(theirs, addr))
}

fn video(display: u32) -> Arc<Message> {
    let mut vf = VideoFrame::new();
    vf.display = display as _;
    vf.union = Some(video_frame::Union::Vp9s(Default::default()));
    let mut msg = Message::new();
    msg.set_video_frame(vf);
    Arc::new(msg)
}

fn switch_display(display: i32) -> Arc<Message> {
    let mut misc = Misc::new();
    misc.set_switch_display(SwitchDisplay {
        display,
        ..Default::default()
    });
    let mut msg = Message::new();
    msg.set_misc(misc);
    Arc::new(msg)
}

fn chat(text: &str) -> Arc<Message> {
    let mut msg = Message::new();
    msg.set_misc({
        let mut misc = Misc::new();
        misc.set_close_reason(text.to_owned());
        misc
    });
    Arc::new(msg)
}

/// Reads the next message and names it: a frame by its display, a switch by "switch:N", a
/// close reason by its text.
async fn next(peer: &mut FramedStream) -> String {
    let bytes = peer
        .next_timeout(2000)
        .await
        .expect("nothing was written")
        .expect("the frame did not decode");
    let msg = Message::parse_from_bytes(&bytes).expect("not a Message");
    match msg.union {
        Some(message::Union::VideoFrame(vf)) => format!("frame:{}", vf.display),
        Some(message::Union::Misc(m)) => match m.union {
            Some(misc::Union::SwitchDisplay(s)) => format!("switch:{}", s.display),
            Some(misc::Union::CloseReason(r)) => r,
            other => format!("misc:{other:?}"),
        },
        other => format!("other:{other:?}"),
    }
}

/// The point of the change: a control message does not wait behind a backlog of pictures.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn a_control_message_overtakes_a_video_backlog() {
    let (w, mut peer) = blocked_pair();
    w.send_video(Instant::now(), video(0));
    hbb_common::tokio::time::sleep(20 * MS).await; // the first frame is now in flight
    for _ in 0..4 {
        w.send_video(Instant::now(), video(0));
    }
    w.send(chat("urgent"));

    assert_eq!(next(&mut peer).await, "frame:0", "the frame already in flight");
    assert_eq!(
        next(&mut peer).await,
        "urgent",
        "the control message must not wait for the four frames behind it"
    );
    for _ in 0..4 {
        assert_eq!(next(&mut peer).await, "frame:0");
    }
}

/// The ordering that must **not** be optimised. `SwitchDisplay` rides the video queue so
/// it lands after the last frame of the old display and before the first of the new one;
/// a peer that saw it early would decode the remaining old frames with the new
/// parameters.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn switch_display_stays_between_the_old_and_new_frames() {
    let (w, mut peer) = blocked_pair();
    w.send_video(Instant::now(), video(0));
    hbb_common::tokio::time::sleep(20 * MS).await;
    w.send_video(Instant::now(), video(0));
    w.send_video(Instant::now(), switch_display(1));
    w.send_video(Instant::now(), video(1));
    // Queued last and on the other queue: it may overtake, and that is allowed.
    w.send(chat("unrelated"));

    let mut seen = Vec::new();
    for _ in 0..5 {
        seen.push(next(&mut peer).await);
    }
    seen.retain(|s| s != "unrelated");
    assert_eq!(
        seen,
        vec!["frame:0", "frame:0", "switch:1", "frame:1"],
        "the switch must sit between the old display's frames and the new display's"
    );
}

/// A link that cannot keep up loses the oldest pictures, not the newest, and the loss is
/// counted so the connection loop can react to it.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn a_full_video_queue_drops_the_oldest_and_counts_it() {
    let (w, mut peer) = blocked_pair();
    w.send_video(Instant::now(), video(9)); // goes in flight, never queued
    hbb_common::tokio::time::sleep(20 * MS).await;
    for i in 0..(queue::VIDEO_QUEUE_CAP + 3) {
        w.send_video(Instant::now(), video(i as u32));
    }

    let report = w.take_report();
    assert_eq!(report.dropped, 3, "three of them had to go");
    assert_eq!(report.queued, queue::VIDEO_QUEUE_CAP);

    assert_eq!(next(&mut peer).await, "frame:9");
    assert_eq!(
        next(&mut peer).await,
        "frame:3",
        "the three oldest were dropped, so the fourth is next"
    );
    assert_eq!(w.take_report().dropped, 0, "the counter is cleared by a read");
}

/// The measurement the bitrate controller depends on. Asserted through the report the
/// connection loop actually reads, so moving the measurement to the wrong place fails
/// here rather than going silently missing.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn the_report_carries_the_time_a_video_write_spent_blocked() {
    let (w, mut peer) = blocked_pair();
    w.send_video(Instant::now(), video(0));
    hbb_common::tokio::time::sleep(60 * MS).await;
    assert_eq!(
        w.take_report(),
        SendReport::default(),
        "nothing is reported while the write is still blocked"
    );

    assert_eq!(next(&mut peer).await, "frame:0");
    // Let the writer finish accounting for the write the read above unblocked.
    hbb_common::tokio::time::sleep(20 * MS).await;

    let report = w.take_report();
    assert_eq!(report.count, 1);
    assert!(report.bits > 0, "the frame's size is what the controller sizes the link with");
    assert!(
        report.blocked_ms >= 40,
        "a write held for 60 ms was reported as {} ms",
        report.blocked_ms
    );
    assert_eq!(report.max_ms, report.blocked_ms, "one write, so the worst is the total");
}

/// A control message is never dropped, however long the queue gets.
#[hbb_common::tokio::test(flavor = "current_thread")]
async fn control_messages_are_never_dropped() {
    let (w, mut peer) = blocked_pair();
    w.send_video(Instant::now(), video(0));
    hbb_common::tokio::time::sleep(20 * MS).await;
    for i in 0..(queue::VIDEO_QUEUE_CAP * 2) {
        w.send(chat(&format!("c{i}")));
    }
    assert_eq!(w.take_report().dropped, 0);

    assert_eq!(next(&mut peer).await, "frame:0");
    for i in 0..(queue::VIDEO_QUEUE_CAP * 2) {
        assert_eq!(next(&mut peer).await, format!("c{i}"));
    }
}
