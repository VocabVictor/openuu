use super::*;

impl<T: Subscriber + From<ConnInner>> ServiceInner<T> {
    pub(super) fn send_new_subscribes(&mut self, msg: Arc<Message>) {
        for s in self.new_subscribes.values_mut() {
            s.send(msg.clone());
        }
    }

    pub(super) fn swap_new_subscribes(&mut self) {
        for (_, s) in self.new_subscribes.drain() {
            self.subscribes.insert(s.id(), s);
        }
        debug_assert!(self.new_subscribes.is_empty());
    }

    #[inline]
    pub(super) fn has_subscribes(&self) -> bool {
        self.subscribes.len() > 0 || self.new_subscribes.len() > 0
    }
}

impl<T: Subscriber + From<ConnInner>> Service for ServiceTmpl<T> {
    #[inline]
    fn name(&self) -> String {
        self.0.read().unwrap().name.clone()
    }

    fn is_subed(&self, id: i32) -> bool {
        self.0.read().unwrap().subscribes.get(&id).is_some()
            || self.0.read().unwrap().new_subscribes.get(&id).is_some()
    }

    fn on_subscribe(&self, sub: ConnInner) {
        let mut lock = self.0.write().unwrap();
        if lock.subscribes.get(&sub.id()).is_some() {
            return;
        }
        if lock.need_snapshot {
            lock.new_subscribes.insert(sub.id(), sub.into());
        } else {
            lock.subscribes.insert(sub.id(), sub.into());
        }
        lock.wakeup.notify();
    }

    fn on_unsubscribe(&self, id: i32) {
        let mut lock = self.0.write().unwrap();
        if let None = lock.subscribes.remove(&id) {
            lock.new_subscribes.remove(&id);
        }
        lock.wakeup.notify();
    }

    fn join(&self) {
        {
            let mut lock = self.0.write().unwrap();
            lock.active = false;
            lock.wakeup.notify();
        }
        let handle = self.0.write().unwrap().handle.take();
        if let Some(handle) = handle {
            if let Err(e) = handle.join() {
                log::error!("Failed to join thread for service {}, {:?}", self.name(), e);
            }
        }
    }

    fn get_option(&self, opt: &str) -> Option<String> {
        self.0.read().unwrap().options.get(opt).cloned()
    }

    fn set_option(&self, opt: &str, val: &str) -> Option<String> {
        self.0
            .write()
            .unwrap()
            .options
            .insert(opt.to_string(), val.to_string())
    }

    #[inline]
    fn ok(&self) -> bool {
        let lock = self.0.read().unwrap();
        lock.active && lock.has_subscribes()
    }
}
