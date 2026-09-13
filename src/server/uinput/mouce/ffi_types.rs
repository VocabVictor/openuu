use super::*;

pub const O_NONBLOCK: c_int = 2048;

/// ioctl and uinput definitions
pub(super) const UI_ABS_SETUP: c_ulong = 1075598596;
pub(super) const UI_SET_EVBIT: c_ulong = 1074025828;
pub(super) const UI_SET_KEYBIT: c_ulong = 1074025829;
pub(super) const UI_SET_RELBIT: c_ulong = 1074025830;
pub(super) const UI_SET_ABSBIT: c_ulong = 1074025831;
pub(super) const UI_DEV_SETUP: c_ulong = 1079792899;
pub(super) const UI_DEV_CREATE: c_ulong = 21761;
pub(super) const UI_DEV_DESTROY: c_uint = 21762;

pub const EV_KEY: c_int = 0x01;
pub const EV_REL: c_int = 0x02;
pub const EV_ABS: c_int = 0x03;
pub const REL_X: c_uint = 0x00;
pub const REL_Y: c_uint = 0x01;
pub const ABS_X: c_uint = 0x00;
pub const ABS_Y: c_uint = 0x01;
pub const REL_WHEEL: c_uint = 0x08;
pub const REL_HWHEEL: c_uint = 0x06;
pub const BTN_LEFT: c_int = 0x110;
pub const BTN_RIGHT: c_int = 0x111;
pub const BTN_MIDDLE: c_int = 0x112;
pub const BTN_SIDE: c_int = 0x113;
pub const BTN_EXTRA: c_int = 0x114;
pub const BTN_FORWARD: c_int = 0x115;
pub const BTN_BACK: c_int = 0x116;
pub const BTN_TASK: c_int = 0x117;
pub(super) const SYN_REPORT: c_int = 0x00;
pub(super) const EV_SYN: c_int = 0x00;
pub(super) const BUS_USB: c_ushort = 0x03;

/// uinput types
#[repr(C)]
pub(super) struct UInputSetup {
    pub(super) id: InputId,
    pub(super) name: [c_char; UINPUT_MAX_NAME_SIZE],
    pub(super) ff_effects_max: c_ulong,
}

#[repr(C)]
pub(super) struct InputId {
    pub(super) bustype: c_ushort,
    pub(super) vendor: c_ushort,
    pub(super) product: c_ushort,
    pub(super) version: c_ushort,
}

#[repr(C)]
pub struct InputEvent {
    pub time: TimeVal,
    pub r#type: c_ushort,
    pub code: c_ushort,
    pub value: c_int,
}

#[repr(C)]
pub struct TimeVal {
    pub tv_sec: c_ulong,
    pub tv_usec: c_ulong,
}

#[repr(C)]
pub struct UinputAbsSetup {
    pub code: c_ushort,
    pub absinfo: InputAbsinfo,
}

#[repr(C)]
pub struct InputAbsinfo {
    pub value: c_int,
    pub minimum: c_int,
    pub maximum: c_int,
    pub fuzz: c_int,
    pub flat: c_int,
    pub resolution: c_int,
}

extern "C" {
    pub(super) fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
    pub(super) fn write(fd: c_int, buf: *mut InputEvent, count: usize) -> c_long;
}

#[derive(Debug, Copy, Clone)]
pub enum MouseButton {
    Left,
    Middle,
    Side,
    Extra,
    Right,
    Back,
    Forward,
    Task,
}

#[derive(Debug, Copy, Clone)]
pub enum ScrollDirection {
    Up,
    Down,
    Right,
    Left,
}

pub(super) const UINPUT_MAX_NAME_SIZE: usize = 80;
