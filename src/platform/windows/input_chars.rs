use super::*;

#[inline]
pub fn get_char_from_vk(vk: u32) -> Option<char> {
    get_char_from_unicode(get_unicode_from_vk(vk)?)
}

pub fn get_char_from_unicode(unicode: u16) -> Option<char> {
    let buff = [unicode];
    if let Some(chr) = String::from_utf16(&buff[..1]).ok()?.chars().next() {
        if chr.is_control() {
            return None;
        } else {
            Some(chr)
        }
    } else {
        None
    }
}

pub fn get_unicode_from_vk(vk: u32) -> Option<u16> {
    const BUF_LEN: i32 = 32;
    let mut buff = [0_u16; BUF_LEN as usize];
    let buff_ptr = buff.as_mut_ptr();
    let len = unsafe {
        let current_window_thread_id = GetWindowThreadProcessId(GetForegroundWindow(), null_mut());
        let layout = GetKeyboardLayout(current_window_thread_id);

        // refs: https://github.com/rustdesk-org/rdev/blob/25a99ce71ab42843ad253dd51e6a35e83e87a8a4/src/windows/keyboard.rs#L115
        let press_state = 129;
        let mut state: [BYTE; 256] = [0; 256];
        let shift_left = rdev::get_modifier(rdev::Key::ShiftLeft);
        let shift_right = rdev::get_modifier(rdev::Key::ShiftRight);
        if shift_left {
            state[VK_LSHIFT as usize] = press_state;
        }
        if shift_right {
            state[VK_RSHIFT as usize] = press_state;
        }
        if shift_left || shift_right {
            state[VK_SHIFT as usize] = press_state;
        }
        ToUnicodeEx(vk, 0x00, &state as _, buff_ptr, BUF_LEN, 0, layout)
    };
    if len == 1 {
        Some(buff[0])
    } else {
        None
    }
}

pub fn is_process_consent_running() -> ResultType<bool> {
    let output = std::process::Command::new("cmd")
        .args(&["/C", "tasklist | findstr consent.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()?;
    Ok(output.status.success() && !output.stdout.is_empty())
}
