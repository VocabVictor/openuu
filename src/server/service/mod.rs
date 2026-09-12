use super::*;
use std::{
    collections::HashSet,
    ops::{Deref, DerefMut},
    thread::{self, JoinHandle},
    time,
};

pub trait Service: Send + Sync {
    fn name(&self) -> String;
    fn on_subscribe(&self, sub: ConnInner);
    fn on_unsubscribe(&self, id: i32);
    fn is_subed(&self, id: i32) -> bool;
    fn join(&self);
    fn get_option(&self, opt: &str) -> Option<String>;
    fn set_option(&self, opt: &str, val: &str) -> Option<String>;
    fn ok(&self) -> bool;
}

pub trait Subscriber: Default + Send + Sync + 'static {
    fn id(&self) -> i32;
    fn send(&mut self, msg: Arc<Message>);
}

#[derive(Default)]
pub struct ServiceInner<T: Subscriber + From<ConnInner>> {
    name: String,
    handle: Option<JoinHandle<()>>,
    subscribes: HashMap<i32, T>,
    new_subscribes: HashMap<i32, T>,
    active: bool,
    need_snapshot: bool,
    options: HashMap<String, String>,
}

pub trait Reset {
    fn reset(&mut self);
    fn init(&mut self) {}
}

pub struct ServiceTmpl<T: Subscriber + From<ConnInner>>(Arc<RwLock<ServiceInner<T>>>);
pub struct ServiceSwap<T: Subscriber + From<ConnInner>>(ServiceTmpl<T>);
pub type GenericService = ServiceTmpl<ConnInner>;
pub const HIBERNATE_TIMEOUT: u64 = 30;
pub const MAX_ERROR_TIMEOUT: u64 = 1_000;
pub const SERVICE_OPTION_VALUE_TRUE: &str = "1";
pub const SERVICE_OPTION_VALUE_FALSE: &str = "0";

mod inner;
mod tmpl;
mod swap;

#[derive(Clone)]
pub struct EmptyExtraFieldService {
    pub sp: GenericService,
}

impl Deref for EmptyExtraFieldService {
    type Target = ServiceTmpl<ConnInner>;

    fn deref(&self) -> &Self::Target {
        &self.sp
    }
}

impl DerefMut for EmptyExtraFieldService {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sp
    }
}

impl EmptyExtraFieldService {
    pub fn new(name: String, need_snapshot: bool) -> Self {
        Self {
            sp: GenericService::new(name, need_snapshot),
        }
    }
}
