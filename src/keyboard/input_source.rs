#[cfg(target_os = "macos")]
use hbb_common::log;
use hbb_common::SessionID;

use crate::ui_interface::{get_local_option, set_local_option};

pub const CONFIG_OPTION_INPUT_SOURCE: &str = "input-source";
// rdev grab mode
pub const CONFIG_INPUT_SOURCE_1: &str = "Input source 1";
pub const CONFIG_INPUT_SOURCE_1_TIP: &str = "input_source_1_tip";
// flutter grab mode
pub const CONFIG_INPUT_SOURCE_2: &str = "Input source 2";
pub const CONFIG_INPUT_SOURCE_2_TIP: &str = "input_source_2_tip";

pub const CONFIG_INPUT_SOURCE_DEFAULT: &str = CONFIG_INPUT_SOURCE_1;

pub fn init_input_source() {
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() {
        // If switching from X11 to Wayland, the grab loop will not be started.
        // Do not change the config here.
        return;
    }
    #[cfg(target_os = "macos")]
    if !crate::platform::macos::is_can_input_monitoring(false) {
        log::error!("init_input_source, is_can_input_monitoring() false");
        set_local_option(
            CONFIG_OPTION_INPUT_SOURCE.to_string(),
            CONFIG_INPUT_SOURCE_2.to_string(),
        );
        return;
    }
    let cur_input_source = get_cur_session_input_source();
    if cur_input_source == CONFIG_INPUT_SOURCE_1 {
        super::IS_RDEV_ENABLED.store(true, super::Ordering::SeqCst);
    }
    super::client::start_grab_loop();
}

pub fn change_input_source(session_id: SessionID, input_source: String) {
    let cur_input_source = get_cur_session_input_source();
    if cur_input_source == input_source {
        return;
    }
    if input_source == CONFIG_INPUT_SOURCE_1 {
        #[cfg(target_os = "macos")]
        if !crate::platform::macos::is_can_input_monitoring(false) {
            log::error!("change_input_source, is_can_input_monitoring() false");
            return;
        }
        // It is ok to start grab loop multiple times.
        super::client::start_grab_loop();
        super::IS_RDEV_ENABLED.store(true, super::Ordering::SeqCst);
        crate::flutter_ffi::session_enter_or_leave(session_id, true);
    } else if input_source == CONFIG_INPUT_SOURCE_2 {
        // No need to stop grab loop.
        crate::flutter_ffi::session_enter_or_leave(session_id, false);
        super::IS_RDEV_ENABLED.store(false, super::Ordering::SeqCst);
    }
    set_local_option(CONFIG_OPTION_INPUT_SOURCE.to_string(), input_source);
}

#[inline]
pub fn get_cur_session_input_source() -> String {
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() {
        return CONFIG_INPUT_SOURCE_2.to_string();
    }
    let input_source = get_local_option(CONFIG_OPTION_INPUT_SOURCE.to_string());
    if input_source.is_empty() {
        CONFIG_INPUT_SOURCE_DEFAULT.to_string()
    } else {
        input_source
    }
}

#[inline]
pub fn get_supported_input_source() -> Vec<(String, String)> {
    #[cfg(target_os = "linux")]
    if !crate::platform::linux::is_x11() {
        return vec![(
            CONFIG_INPUT_SOURCE_2.to_string(),
            CONFIG_INPUT_SOURCE_2_TIP.to_string(),
        )];
    }
    vec![
        (
            CONFIG_INPUT_SOURCE_1.to_string(),
            CONFIG_INPUT_SOURCE_1_TIP.to_string(),
        ),
        (
            CONFIG_INPUT_SOURCE_2.to_string(),
            CONFIG_INPUT_SOURCE_2_TIP.to_string(),
        ),
    ]
}
