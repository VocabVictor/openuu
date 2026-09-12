use std::collections::HashMap;

#[cfg(any(target_os = "android"))]
use hbb_common::log;
#[cfg(any(target_os = "android"))]
use scrap::android::call_main_service_set_by_name;
use serde_json::json;

use crate::ui_cm_interface::InvokeUiCM;

use super::GLOBAL_EVENT_STREAM;

#[derive(Clone)]
struct FlutterHandler {}

impl InvokeUiCM for FlutterHandler {
    //TODO port_forward
    fn add_connection(&self, client: &crate::ui_cm_interface::Client) {
        let client_json = serde_json::to_string(&client).unwrap_or("".into());
        // send to Android service, active notification no matter UI is shown or not.
        #[cfg(target_os = "android")]
        if let Err(e) =
            call_main_service_set_by_name("add_connection", Some(&client_json), None)
        {
            log::debug!("call_main_service_set_by_name fail,{}", e);
        }
        // send to UI, refresh widget
        self.push_event("add_connection", &[("client", &client_json)]);
    }

    fn remove_connection(&self, id: i32, close: bool) {
        self.push_event(
            "on_client_remove",
            &[("id", &id.to_string()), ("close", &close.to_string())],
        );
    }

    fn new_message(&self, id: i32, text: String) {
        self.push_event(
            "chat_server_mode",
            &[("id", &id.to_string()), ("text", &text)],
        );
    }

    fn change_theme(&self, dark: String) {
        self.push_event("theme", &[("dark", &dark)]);
    }

    fn change_language(&self) {
        self.push_event::<&str>("language", &[]);
    }

    fn show_elevation(&self, show: bool) {
        self.push_event("show_elevation", &[("show", &show.to_string())]);
    }

    fn update_voice_call_state(&self, client: &crate::ui_cm_interface::Client) {
        let client_json = serde_json::to_string(&client).unwrap_or("".into());
        // send to Android service, active notification no matter UI is shown or not.
        #[cfg(target_os = "android")]
        if let Err(e) =
            call_main_service_set_by_name("update_voice_call_state", Some(&client_json), None)
        {
            log::debug!("call_main_service_set_by_name fail,{}", e);
        }
        self.push_event("update_voice_call_state", &[("client", &client_json)]);
    }

    fn file_transfer_log(&self, action: &str, log: &str) {
        self.push_event("cm_file_transfer_log", &[(action, log)]);
    }
}

impl FlutterHandler {
    fn push_event<V>(&self, name: &str, event: &[(&str, V)])
    where
        V: Sized + serde::Serialize + Clone,
    {
        let mut h: HashMap<&str, serde_json::Value> =
            event.iter().map(|(k, v)| (*k, json!(*v))).collect();
        debug_assert!(h.get("name").is_none());
        h.insert("name", json!(name));

        if let Some(s) = GLOBAL_EVENT_STREAM.read().unwrap().get(super::APP_TYPE_CM) {
            s.add(serde_json::ser::to_string(&h).unwrap_or("".to_owned()));
        } else {
            println!(
                "Push event {} failed. No {} event stream found.",
                name,
                super::APP_TYPE_CM
            );
        };
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn start_listen_ipc() {
    use crate::ui_cm_interface::{start_ipc, ConnectionManager};

    #[cfg(target_os = "linux")]
    std::thread::spawn(crate::ipc::start_pa);

    let cm = ConnectionManager {
        ui_handler: FlutterHandler {},
    };
    std::thread::spawn(move || start_ipc(cm));
}

#[inline]
pub fn cm_init() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    start_listen_ipc();
}

#[cfg(target_os = "android")]
use hbb_common::tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[cfg(target_os = "android")]
pub fn start_channel(
    rx: UnboundedReceiver<crate::ipc::Data>,
    tx: UnboundedSender<crate::ipc::Data>,
) {
    use crate::ui_cm_interface::start_listen;
    let cm = crate::ui_cm_interface::ConnectionManager {
        ui_handler: FlutterHandler {},
    };
    std::thread::spawn(move || start_listen(cm, rx, tx));
}
