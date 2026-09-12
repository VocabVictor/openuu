use super::*;

impl PortForwardMux {
    pub fn new(tx: Sender, login_target: String) -> Self {
        Self {
            channels: HashMap::new(),
            tx,
            login_target,
            teardown: watch::channel(false).0,
        }
    }

    /// `tunnel_permitted` is consulted for `open` alone, so the lookup is not
    /// made per 64 KiB of data.
    pub fn handle(&mut self, frame: PortForwardChannel, tunnel_permitted: impl FnOnce() -> bool) {
        match frame.union {
            Some(port_forward_channel::Union::Open(open)) => {
                let permitted = tunnel_permitted();
                self.on_open(open, permitted)
            }
            Some(port_forward_channel::Union::Data(d)) => {
                let len = d.data.len();
                let Some(entry) = self.channels.get(&d.channel_id) else {
                    log::debug!("port forward data for unknown channel {}", d.channel_id);
                    return;
                };
                let accepted = entry.window.lock().unwrap().accept(len);
                let delivered = accepted && entry.inbound.send(Inbound::Data(d.data)).is_ok();
                if delivered {
                    return;
                }
                // Dropped here and now, so the peer cannot queue anything more
                // for this id while the task is still on its way out.
                let Some(entry) = self.channels.remove(&d.channel_id) else {
                    return;
                };
                if !accepted {
                    log::warn!("port forward channel {} overran its window", d.channel_id);
                    entry.inbound.send(Inbound::Violation).ok();
                }
            }
            Some(port_forward_channel::Union::Close(c)) => {
                if let Some(entry) = self.channels.remove(&c.channel_id) {
                    entry.inbound.send(Inbound::Close).ok();
                } else {
                    log::debug!("port forward close for unknown channel {}", c.channel_id);
                }
            }
            Some(port_forward_channel::Union::WindowUpdate(u)) => {
                match self.channels.get(&u.channel_id) {
                    Some(entry) => entry.credit.add(u.add),
                    None => log::debug!(
                        "port forward window update for unknown channel {}",
                        u.channel_id
                    ),
                }
            }
            Some(port_forward_channel::Union::Opened(o)) => {
                log::debug!("ignoring opened for channel {} on the controlled side", o.channel_id);
            }
            _ => {}
        }
    }

    pub(super) fn on_open(&mut self, open: PortForwardOpen, permitted: bool) {
        let id = open.channel_id;
        self.channels.retain(|_, e| !e.inbound.is_closed());
        if !permitted {
            self.reply(opened_msg(id, false, "No permission of IP tunneling", 0));
            return;
        }
        if self.channels.len() >= MAX_CHANNELS {
            self.reply(opened_msg(id, false, "Too many port forward channels", 0));
            return;
        }
        if self.channels.contains_key(&id) {
            log::debug!("ignoring open for live channel {}", id);
            return;
        }
        let mut pf = PortForward {
            host: open.host,
            port: open.port,
            ..Default::default()
        };
        let (addr, is_rdp) = Connection::normalize_port_forward_target(&mut pf);
        // Approval and permission checks saw the login's target; a tunnel
        // serves that one target and nothing else.
        if addr != self.login_target {
            log::warn!(
                "port forward channel {} asked for {} on a tunnel logged in for {}",
                id,
                addr,
                self.login_target
            );
            self.reply(opened_msg(id, false, "Port forward target not authorized", 0));
            return;
        }
        let (inbound_tx, inbound_rx) = mpsc::unbounded_channel();
        let credit = Arc::new(SendCredit::new(effective_window(open.window)));
        let window = Arc::new(Mutex::new(RecvWindow::new(INITIAL_WINDOW)));
        self.channels.insert(
            id,
            Entry {
                inbound: inbound_tx,
                credit: credit.clone(),
                window: window.clone(),
            },
        );
        tokio::spawn(run_controlled_channel(
            id,
            addr,
            is_rdp,
            credit,
            window,
            inbound_rx,
            FrameSink::Direct(self.tx.clone()),
            self.teardown.subscribe(),
        ));
    }

    pub(super) fn reply(&self, msg: Message) {
        self.tx
            .send((tokio::time::Instant::now(), Arc::new(msg)))
            .ok();
    }

    #[cfg(test)]
    pub fn live_channels(&self) -> usize {
        self.channels.len()
    }

    #[cfg(test)]
    pub fn recv_window_remaining(&self, id: i32) -> Option<u32> {
        self.channels.get(&id).map(|e| e.window.lock().unwrap().remaining())
    }

    /// Every task ends and drops its target socket: the queue's senders go for
    /// a task on the queue, `teardown` reaches one parked on the socket.
    pub fn close_all(&mut self) {
        self.channels.clear();
        // Not `send`: with no channel live it stores nothing, and one opened
        // as the tunnel closes would never see it.
        self.teardown.send_replace(true);
    }
}
