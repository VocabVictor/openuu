use super::*;

impl ConnInner {
    pub fn new(id: i32, tx: Option<Sender>, tx_video: Option<Sender>) -> Self {
        Self { id, tx, tx_video }
    }
}

impl Subscriber for ConnInner {
    #[inline]
    fn id(&self) -> i32 {
        self.id
    }

    #[inline]
    fn send(&mut self, msg: Arc<Message>) {
        // Send SwitchDisplay on the same channel as VideoFrame to avoid send order problems.
        let tx_by_video = match &msg.union {
            Some(message::Union::VideoFrame(_)) => true,
            Some(message::Union::Misc(misc)) => match &misc.union {
                Some(misc::Union::SwitchDisplay(_)) => true,
                _ => false,
            },
            _ => false,
        };
        let tx = if tx_by_video {
            self.tx_video.as_mut()
        } else {
            self.tx.as_mut()
        };
        tx.map(|tx| {
            allow_err!(tx.send((Instant::now(), msg)));
        });
    }
}
