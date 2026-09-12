use self::winapi::ctypes::c_int;
use self::winapi::shared::{basetsd::ULONG_PTR, minwindef::*, windef::*};
use self::winapi::um::winbase::*;
use self::winapi::um::winuser::*;
use winapi;

use crate::win::keycodes::*;
use crate::{Key, KeyboardControllable, MouseButton, MouseControllable};
use std::mem::*;

extern "system" {
    pub fn GetLastError() -> DWORD;
}

/// The main struct for handling the event emitting
#[derive(Default)]
pub struct Enigo;
static mut LAYOUT: HKL = std::ptr::null_mut();

/// The dwExtraInfo value in keyboard and mouse structure that used in SendInput()
pub const ENIGO_INPUT_EXTRA_VALUE: ULONG_PTR = 100;

mod mouse;
mod keyboard;
mod enigo_impl;

fn mouse_event(flags: u32, data: u32, dx: i32, dy: i32) -> DWORD {
    let mut u = INPUT_u::default();
    unsafe {
        *u.mi_mut() = MOUSEINPUT {
            dx,
            dy,
            mouseData: data,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: ENIGO_INPUT_EXTRA_VALUE,
        };
    }
    let mut input = INPUT {
        type_: INPUT_MOUSE,
        u,
    };
    unsafe { SendInput(1, &mut input as LPINPUT, size_of::<INPUT>() as c_int) }
}

fn keybd_event(mut flags: u32, vk: u16, scan: u16) -> DWORD {
    let mut scan = scan;
    unsafe {
        // https://github.com/rustdesk/rustdesk/issues/366
        if scan == 0 {
            if LAYOUT.is_null() {
                let current_window_thread_id =
                    GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut());
                LAYOUT = GetKeyboardLayout(current_window_thread_id);
            }
            scan = MapVirtualKeyExW(vk as _, 0, LAYOUT) as _;
        }
    }

    if flags & KEYEVENTF_UNICODE == 0 {
        if scan >> 8 == 0xE0 || scan >> 8 == 0xE1 {
            flags |= winapi::um::winuser::KEYEVENTF_EXTENDEDKEY;
        }
    }
    let mut union: INPUT_u = unsafe { std::mem::zeroed() };
    unsafe {
        *union.ki_mut() = KEYBDINPUT {
            wVk: vk,
            wScan: scan,
            dwFlags: flags,
            time: 0,
            dwExtraInfo: ENIGO_INPUT_EXTRA_VALUE,
        };
    }
    let mut inputs = [INPUT {
        type_: INPUT_KEYBOARD,
        u: union,
    }; 1];
    unsafe {
        SendInput(
            inputs.len() as UINT,
            inputs.as_mut_ptr(),
            size_of::<INPUT>() as c_int,
        )
    }
}

fn get_error() -> String {
    unsafe {
        let buff_size = 256;
        let mut buff: Vec<u16> = Vec::with_capacity(buff_size);
        buff.resize(buff_size, 0);
        let errno = GetLastError();
        let chars_copied = FormatMessageW(
            FORMAT_MESSAGE_IGNORE_INSERTS
                | FORMAT_MESSAGE_FROM_SYSTEM
                | FORMAT_MESSAGE_ARGUMENT_ARRAY,
            std::ptr::null(),
            errno,
            0,
            buff.as_mut_ptr(),
            (buff_size + 1) as u32,
            std::ptr::null_mut(),
        );
        if chars_copied == 0 {
            return "".to_owned();
        }
        let mut curr_char: usize = chars_copied as usize;
        while curr_char > 0 {
            let ch = buff[curr_char];

            if ch >= ' ' as u16 {
                break;
            }
            curr_char -= 1;
        }
        let sl = std::slice::from_raw_parts(buff.as_ptr(), curr_char);
        let err_msg = String::from_utf16(sl);
        return err_msg.unwrap_or("".to_owned());
    }
}
