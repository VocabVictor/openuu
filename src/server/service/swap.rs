use super::*;

impl<T: Subscriber + From<ConnInner>> ServiceSwap<T> {
    #[inline]
    pub fn send(&self, msg: Message) {
        self.send_shared(Arc::new(msg));
    }

    #[inline]
    pub fn send_shared(&self, msg: Arc<Message>) {
        (self.0).0.write().unwrap().send_new_subscribes(msg);
    }

    #[inline]
    pub fn has_subscribes(&self) -> bool {
        (self.0).0.read().unwrap().subscribes.len() > 0
    }
}

impl<T: Subscriber + From<ConnInner>> Drop for ServiceSwap<T> {
    fn drop(&mut self) {
        (self.0).0.write().unwrap().swap_new_subscribes();
    }
}
