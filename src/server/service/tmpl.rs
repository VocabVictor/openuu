use super::*;

impl<T: Subscriber + From<ConnInner>> Clone for ServiceTmpl<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: Subscriber + From<ConnInner>> ServiceTmpl<T> {
    pub fn new(name: String, need_snapshot: bool) -> Self {
        Self(Arc::new(RwLock::new(ServiceInner::<T> {
            name,
            active: true,
            need_snapshot,
            ..Default::default()
        })))
    }

    #[inline]
    pub fn is_option_true(&self, opt: &str) -> bool {
        self.get_option(opt)
            .map_or(false, |v| v == SERVICE_OPTION_VALUE_TRUE)
    }

    #[inline]
    pub fn set_option_bool(&self, opt: &str, val: bool) {
        if val {
            self.set_option(opt, SERVICE_OPTION_VALUE_TRUE);
        } else {
            self.set_option(opt, SERVICE_OPTION_VALUE_FALSE);
        }
    }

    #[inline]
    pub fn has_subscribes(&self) -> bool {
        self.0.read().unwrap().has_subscribes()
    }

    /// Sleep until there is something to do. A subscriber arriving or the service being
    /// stopped ends it at once; anything else waits out [`IDLE_TIMEOUT`].
    fn hibernate(&self) {
        let (wakeup, wakeups) = {
            let lock = self.0.read().unwrap();
            (lock.wakeup.clone(), lock.wakeups.clone())
        };
        wakeups.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // Read the generation before looking at the state: a subscriber arriving between
        // the two changes the generation, so the wait below returns at once.
        let since = wakeup.generation();
        if self.has_subscribes() || !self.active() {
            return;
        }
        wakeup.wait(since, time::Duration::from_millis(IDLE_TIMEOUT));
    }

    /// Times the loop has come round, idle or not.
    pub fn wakeups(&self) -> u64 {
        self.0
            .read()
            .unwrap()
            .wakeups
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    #[inline]
    fn note_wakeup(&self) {
        self.0
            .read()
            .unwrap()
            .wakeups
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn snapshot<F>(&self, callback: F) -> ResultType<()>
    where
        F: FnMut(ServiceSwap<T>) -> ResultType<()>,
    {
        if self.0.read().unwrap().new_subscribes.len() > 0 {
            log::info!("Call snapshot of {} service", self.name());
            let mut callback = callback;
            callback(ServiceSwap::<T>(self.clone()))?;
        }
        Ok(())
    }

    #[inline]
    pub fn send(&self, msg: Message) {
        self.send_shared(Arc::new(msg));
    }

    pub fn send_to(&self, msg: Message, id: i32) {
        if let Some(s) = self.0.write().unwrap().subscribes.get_mut(&id) {
            s.send(Arc::new(msg));
        }
    }

    pub fn send_to_others(&self, msg: Message, id: i32) {
        let msg = Arc::new(msg);
        let mut lock = self.0.write().unwrap();
        for (sid, s) in lock.subscribes.iter_mut() {
            if *sid != id {
                s.send(msg.clone());
            }
        }
    }

    pub fn send_shared(&self, msg: Arc<Message>) {
        let mut lock = self.0.write().unwrap();
        for s in lock.subscribes.values_mut() {
            s.send(msg.clone());
        }
    }

    pub fn send_video_frame(&self, msg: Message) -> HashSet<i32> {
        self.send_video_frame_shared(Arc::new(msg))
    }

    pub fn send_video_frame_shared(&self, msg: Arc<Message>) -> HashSet<i32> {
        let mut conn_ids = HashSet::new();
        let mut lock = self.0.write().unwrap();
        for s in lock.subscribes.values_mut() {
            s.send(msg.clone());
            conn_ids.insert(s.id());
        }
        conn_ids
    }

    pub fn send_without(&self, msg: Message, sub: i32) {
        let mut lock = self.0.write().unwrap();
        let msg = Arc::new(msg);
        for s in lock.subscribes.values_mut() {
            if sub != s.id() {
                s.send(msg.clone());
            }
        }
    }

    pub fn repeat<S, F, Svc>(svc: &Svc, interval_ms: u64, callback: F)
    where
        F: 'static + FnMut(Svc, &mut S) -> ResultType<()> + Send,
        S: 'static + Default + Reset,
        Svc: 'static + Clone + Send + DerefMut<Target = ServiceTmpl<T>>,
    {
        let interval = time::Duration::from_millis(interval_ms);
        let mut callback = callback;
        let sp = svc.clone();
        let thread = thread::spawn(move || {
            let mut state = S::default();
            let mut may_reset = false;
            while sp.active() {
                if !sp.has_subscribes() {
                    if may_reset {
                        state.reset();
                        may_reset = false;
                    }
                    sp.hibernate();
                    continue;
                }
                sp.note_wakeup();
                let now = time::Instant::now();
                {
                    if !may_reset {
                        may_reset = true;
                        state.init();
                    }
                    if let Err(err) = callback(sp.clone(), &mut state) {
                        log::error!("Error of {} service: {}", sp.name(), err);
                        thread::sleep(time::Duration::from_millis(MAX_ERROR_TIMEOUT));
                        #[cfg(windows)]
                        crate::platform::windows::try_change_desktop();
                    }
                }
                let elapsed = now.elapsed();
                if elapsed < interval {
                    thread::sleep(interval - elapsed);
                }
            }
            log::info!("Service {} exit", sp.name());
        });
        svc.0.write().unwrap().handle = Some(thread);
    }

    pub fn run<F, Svc>(svc: &Svc, callback: F)
    where
        F: 'static + FnMut(Svc) -> ResultType<()> + Send,
        Svc: 'static + Clone + Send + DerefMut<Target = ServiceTmpl<T>>,
    {
        let sp = svc.clone();
        let mut callback = callback;
        let thread = thread::spawn(move || {
            let mut error_timeout = HIBERNATE_TIMEOUT;
            while sp.active() {
                if !sp.has_subscribes() {
                    sp.hibernate();
                    continue;
                }
                sp.note_wakeup();
                {
                    log::debug!("Enter {} service inner loop", sp.name());
                    let tm = time::Instant::now();
                    if let Err(err) = callback(sp.clone()) {
                        log::error!("Error of {} service: {}", sp.name(), err);
                        if tm.elapsed() > time::Duration::from_millis(MAX_ERROR_TIMEOUT) {
                            error_timeout = HIBERNATE_TIMEOUT;
                        } else {
                            error_timeout *= 2;
                        }
                        if error_timeout > MAX_ERROR_TIMEOUT {
                            error_timeout = MAX_ERROR_TIMEOUT;
                        }
                        thread::sleep(time::Duration::from_millis(error_timeout));
                        #[cfg(windows)]
                        crate::platform::windows::try_change_desktop();
                    } else {
                        log::debug!("Exit {} service inner loop", sp.name());
                    }
                }
                thread::sleep(time::Duration::from_millis(HIBERNATE_TIMEOUT));
            }
            log::info!("Service {} exit", sp.name());
        });
        svc.0.write().unwrap().handle = Some(thread);
    }

    #[inline]
    pub fn active(&self) -> bool {
        self.0.read().unwrap().active
    }
}
