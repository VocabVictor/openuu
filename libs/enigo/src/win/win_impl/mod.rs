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

impl Enigo {
    /// Gets the (width, height) of the main display in screen coordinates
    /// (pixels).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut size = Enigo::main_display_size();
    /// ```
    pub fn main_display_size() -> (usize, usize) {
        let w = unsafe { GetSystemMetrics(SM_CXSCREEN) as usize };
        let h = unsafe { GetSystemMetrics(SM_CYSCREEN) as usize };
        (w, h)
    }

    /// Gets the location of mouse in screen coordinates (pixels).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut location = Enigo::mouse_location();
    /// ```
    pub fn mouse_location() -> (i32, i32) {
        let mut point = POINT { x: 0, y: 0 };
        let result = unsafe { GetCursorPos(&mut point) };
        if result != 0 {
            (point.x, point.y)
        } else {
            (0, 0)
        }
    }

    fn unicode_key_click(&self, unicode_char: u16) {
        self.unicode_key_down(unicode_char);
        self.unicode_key_up(unicode_char);
    }

    fn unicode_key_down(&self, unicode_char: u16) {
        keybd_event(KEYEVENTF_UNICODE, 0, unicode_char);
    }

    fn unicode_key_up(&self, unicode_char: u16) {
        keybd_event(KEYEVENTF_UNICODE | KEYEVENTF_KEYUP, 0, unicode_char);
    }

    fn key_to_keycode(&self, key: Key) -> u16 {
        // do not use the codes from crate winapi they're
        // wrongly typed with i32 instead of i16 use the
        // ones provided by win/keycodes.rs that are prefixed
        // with an 'E' infront of the original name
        #[allow(deprecated)]
        // I mean duh, we still need to support deprecated keys until they're removed
        match key {
            Key::Alt => EVK_MENU,
            Key::Backspace => EVK_BACK,
            Key::CapsLock => EVK_CAPITAL,
            Key::Control => EVK_LCONTROL,
            Key::Delete => EVK_DELETE,
            Key::DownArrow => EVK_DOWN,
            Key::End => EVK_END,
            Key::Escape => EVK_ESCAPE,
            Key::F1 => EVK_F1,
            Key::F10 => EVK_F10,
            Key::F11 => EVK_F11,
            Key::F12 => EVK_F12,
            Key::F2 => EVK_F2,
            Key::F3 => EVK_F3,
            Key::F4 => EVK_F4,
            Key::F5 => EVK_F5,
            Key::F6 => EVK_F6,
            Key::F7 => EVK_F7,
            Key::F8 => EVK_F8,
            Key::F9 => EVK_F9,
            Key::Home => EVK_HOME,
            Key::LeftArrow => EVK_LEFT,
            Key::Option => EVK_MENU,
            Key::PageDown => EVK_NEXT,
            Key::PageUp => EVK_PRIOR,
            Key::Return => EVK_RETURN,
            Key::RightArrow => EVK_RIGHT,
            Key::Shift => EVK_SHIFT,
            Key::Space => EVK_SPACE,
            Key::Tab => EVK_TAB,
            Key::UpArrow => EVK_UP,
            Key::Numpad0 => EVK_NUMPAD0,
            Key::Numpad1 => EVK_NUMPAD1,
            Key::Numpad2 => EVK_NUMPAD2,
            Key::Numpad3 => EVK_NUMPAD3,
            Key::Numpad4 => EVK_NUMPAD4,
            Key::Numpad5 => EVK_NUMPAD5,
            Key::Numpad6 => EVK_NUMPAD6,
            Key::Numpad7 => EVK_NUMPAD7,
            Key::Numpad8 => EVK_NUMPAD8,
            Key::Numpad9 => EVK_NUMPAD9,
            Key::Cancel => EVK_CANCEL,
            Key::Clear => EVK_CLEAR,
            Key::Pause => EVK_PAUSE,
            Key::Kana => EVK_KANA,
            Key::Hangul => EVK_HANGUL,
            Key::Junja => EVK_JUNJA,
            Key::Final => EVK_FINAL,
            Key::Hanja => EVK_HANJA,
            Key::Kanji => EVK_KANJI,
            Key::Convert => EVK_CONVERT,
            Key::Select => EVK_SELECT,
            Key::Print => EVK_PRINT,
            Key::Execute => EVK_EXECUTE,
            Key::Snapshot => EVK_SNAPSHOT,
            Key::Insert => EVK_INSERT,
            Key::Help => EVK_HELP,
            Key::Sleep => EVK_SLEEP,
            Key::Separator => EVK_SEPARATOR,
            Key::Mute => EVK_VOLUME_MUTE,
            Key::VolumeDown => EVK_VOLUME_DOWN,
            Key::VolumeUp => EVK_VOLUME_UP,
            Key::Scroll => EVK_SCROLL,
            Key::NumLock => EVK_NUMLOCK,
            Key::RWin => EVK_RWIN,
            Key::Apps => EVK_APPS,
            Key::Add => EVK_ADD,
            Key::Multiply => EVK_MULTIPLY,
            Key::Decimal => EVK_DECIMAL,
            Key::Subtract => EVK_SUBTRACT,
            Key::Divide => EVK_DIVIDE,
            Key::NumpadEnter => EVK_RETURN,
            Key::Equals => '=' as _,
            Key::RightShift => EVK_RSHIFT,
            Key::RightControl => EVK_RCONTROL,
            Key::RightAlt => EVK_RMENU,

            Key::Raw(raw_keycode) => raw_keycode,
            Key::Super | Key::Command | Key::Windows | Key::Meta => EVK_LWIN,
            Key::Layout(..) => {
                // unreachable
                0
            }
        }
    }

    fn get_layoutdependent_keycode(&self, chr: char) -> u16 {
        unsafe {
            LAYOUT = std::ptr::null_mut();
        }
        // NOTE VkKeyScanW uses the current keyboard LAYOUT
        // to specify a LAYOUT use VkKeyScanExW and GetKeyboardLayout
        // or load one with LoadKeyboardLayoutW
        let current_window_thread_id =
            unsafe { GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut()) };
        unsafe { LAYOUT = GetKeyboardLayout(current_window_thread_id) };
        unsafe { VkKeyScanExW(chr as _, LAYOUT) as _ }
    }
}
