use super::*;

impl TunnelHandle {
    /// `open` leaves from inside the channel's own task, down the ordered
    /// data queue, ahead of the channel's first `data`. On the control
    /// queue it could be overtaken by that `data` whenever the tunnel loop
    /// resumes with both queues non-empty.
    pub fn open(
        self: &Arc<Self>,
        host: &str,
        port: i32,
        socket: TcpStream,
        prebuf: Vec<u8>,
    ) -> ResultType<()> {
        if self.sink.is_closed() {
            hbb_common::bail!("port forward tunnel is gone");
        }
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let credit = Arc::new(SendCredit::new(INITIAL_WINDOW));
        let window = Arc::new(Mutex::new(RecvWindow::new(CHANNEL_WINDOW)));
        let id = {
            let mut channels = self.channels.lock().unwrap();
            if channels.len() >= MAX_CHANNELS {
                hbb_common::bail!("too many port forward channels");
            }
            // Once the counter has come all the way round it can land on
            // a channel still up; fewer than MAX_CHANNELS are, so a free
            // id is a few steps away.
            let mut id = self.next_id.fetch_add(1, Ordering::Relaxed);
            while channels.contains_key(&id) {
                id = self.next_id.fetch_add(1, Ordering::Relaxed);
            }
            channels.insert(
                id,
                ChannelEntry {
                    inbound: inbound_tx,
                    credit: credit.clone(),
                    window: window.clone(),
                    opened: false,
                },
            );
            id
        };
        let open = open_msg(id, host, port, CHANNEL_WINDOW);
        let (reader, writer) = socket.into_split();
        let sink = self.sink.clone();
        let teardown = self.teardown.subscribe();
        let handle = self.clone();
        tokio::spawn(async move {
            if sink.send_ordered(open).await.is_ok() {
                run_channel(id, reader, writer, prebuf, Vec::new(), credit, window, inbound_rx, sink, teardown).await;
            }
            handle.channels.lock().unwrap().remove(&id);
        });
        Ok(())
    }

    pub(in crate::port_forward_mux) fn on_frame(&self, ch: PortForwardChannel) -> Option<String> {
        match ch.union {
            Some(port_forward_channel::Union::Opened(o)) => {
                let refused = {
                    let mut channels = self.channels.lock().unwrap();
                    if o.success {
                        // A repeated `opened` must not raise the credit again.
                        if let Some(e) = channels.get_mut(&o.channel_id) {
                            if !e.opened {
                                e.opened = true;
                                e.credit.raise_initial(o.window);
                            }
                        }
                        None
                    } else if let Some(e) = channels.remove(&o.channel_id) {
                        log::debug!("port forward channel {} refused: {}", o.channel_id, o.message);
                        e.inbound.send(Inbound::Close).ok();
                        Some(o.message)
                    } else {
                        None
                    }
                };
                refused.and_then(|message| self.first_report(message))
            }
            Some(port_forward_channel::Union::Data(d)) => {
                let mut channels = self.channels.lock().unwrap();
                let Some(e) = channels.get(&d.channel_id) else {
                    log::debug!("port forward data for unknown channel {}", d.channel_id);
                    return None;
                };
                let accepted = e.window.lock().unwrap().accept(d.data.len());
                let delivered = accepted && e.inbound.send(Inbound::Data(d.data)).is_ok();
                if delivered {
                    return None;
                }
                // Dropped here and now, so the peer cannot queue anything more
                // for this id while the task is still on its way out.
                if let Some(e) = channels.remove(&d.channel_id) {
                    if !accepted {
                        log::warn!("port forward channel {} overran its window", d.channel_id);
                        e.inbound.send(Inbound::Violation).ok();
                    }
                }
                None
            }
            Some(port_forward_channel::Union::Close(c)) => {
                if let Some(e) = self.channels.lock().unwrap().remove(&c.channel_id) {
                    e.inbound.send(Inbound::Close).ok();
                }
                None
            }
            Some(port_forward_channel::Union::WindowUpdate(u)) => {
                if let Some(e) = self.channels.lock().unwrap().get(&u.channel_id) {
                    e.credit.add(u.add);
                }
                None
            }
            Some(port_forward_channel::Union::Open(o)) => {
                log::debug!("ignoring open for channel {} on the controller", o.channel_id);
                None
            }
            _ => None,
        }
    }

    /// The peer's reason for refusing a channel, unless it was reported
    /// within `REPORT_AGAIN_AFTER`. One page load can have a dozen
    /// connections refused for the same reason, and the user needs one
    /// dialog, not a dozen; a burst that keeps going keeps it quiet.
    fn first_report(&self, message: String) -> Option<String> {
        self.first_report_at(message, Instant::now())
    }

    pub(in crate::port_forward_mux) fn first_report_at(&self, message: String, now: Instant) -> Option<String> {
        if message.is_empty() {
            return None;
        }
        let mut reported = self.reported.lock().unwrap();
        reported.retain(|_, last| now.duration_since(*last) < REPORT_AGAIN_AFTER);
        if let Some(last) = reported.get_mut(&message) {
            *last = now;
            return None;
        }
        if reported.len() >= MAX_REPORTED_OPEN_ERRORS {
            return None;
        }
        reported.insert(message.clone(), now);
        Some(message)
    }

    pub(super) fn close_all(&self) {
        self.channels.lock().unwrap().clear();
        // Not `send`: with no channel live it stores nothing, and one
        // opened as the tunnel closes would never see it.
        self.teardown.send_replace(true);
    }

    #[cfg(test)]
    pub fn live_channels(&self) -> usize {
        self.channels.lock().unwrap().len()
    }

    #[cfg(test)]
    pub fn set_next_id(&self, id: i32) {
        self.next_id.store(id, Ordering::Relaxed);
    }
}
