use super::*;

pub fn apply_openuu_app_name() {
    // Runs before any config, IPC or registry access so OpenUU never shares
    // %APPDATA%\RustDesk, the RustDesk named pipes or install keys with an
    // upstream RustDesk on the same machine.
    *config::APP_NAME.write().unwrap() = "OpenUU".to_owned();
}

pub fn load_custom_client() {
    apply_openuu_app_name();
    #[cfg(debug_assertions)]
    if let Ok(data) = std::fs::read_to_string("./custom.txt") {
        read_custom_client(data.trim());
        return;
    }
    let Some(path) = std::env::current_exe().map_or(None, |x| x.parent().map(|x| x.to_path_buf()))
    else {
        return;
    };
    #[cfg(target_os = "macos")]
    let path = path.join("../Resources");
    let path = path.join("custom.txt");
    if path.is_file() {
        let Ok(data) = std::fs::read_to_string(&path) else {
            log::error!("Failed to read custom client config");
            return;
        };
        read_custom_client(&data.trim());
    }
}

fn read_custom_client_advanced_settings(
    settings: serde_json::Value,
    map_display_settings: &HashMap<String, &&str>,
    map_local_settings: &HashMap<String, &&str>,
    map_settings: &HashMap<String, &&str>,
    map_buildin_settings: &HashMap<String, &&str>,
    is_override: bool,
) {
    let mut display_settings = if is_override {
        config::OVERWRITE_DISPLAY_SETTINGS.write().unwrap()
    } else {
        config::DEFAULT_DISPLAY_SETTINGS.write().unwrap()
    };
    let mut local_settings = if is_override {
        config::OVERWRITE_LOCAL_SETTINGS.write().unwrap()
    } else {
        config::DEFAULT_LOCAL_SETTINGS.write().unwrap()
    };
    let mut server_settings = if is_override {
        config::OVERWRITE_SETTINGS.write().unwrap()
    } else {
        config::DEFAULT_SETTINGS.write().unwrap()
    };
    let mut buildin_settings = config::BUILTIN_SETTINGS.write().unwrap();

    if let Some(settings) = settings.as_object() {
        for (k, v) in settings {
            let Some(v) = v.as_str() else {
                continue;
            };
            if let Some(k2) = map_display_settings.get(k) {
                display_settings.insert(k2.to_string(), v.to_owned());
            } else if let Some(k2) = map_local_settings.get(k) {
                local_settings.insert(k2.to_string(), v.to_owned());
            } else if let Some(k2) = map_settings.get(k) {
                server_settings.insert(k2.to_string(), v.to_owned());
            } else if let Some(k2) = map_buildin_settings.get(k) {
                buildin_settings.insert(k2.to_string(), v.to_owned());
            } else {
                let k2 = k.replace("_", "-");
                let k = k2.replace("-", "_");
                // display
                display_settings.insert(k.clone(), v.to_owned());
                display_settings.insert(k2.clone(), v.to_owned());
                // local
                local_settings.insert(k.clone(), v.to_owned());
                local_settings.insert(k2.clone(), v.to_owned());
                // server
                server_settings.insert(k.clone(), v.to_owned());
                server_settings.insert(k2.clone(), v.to_owned());
                // buildin
                buildin_settings.insert(k.clone(), v.to_owned());
                buildin_settings.insert(k2.clone(), v.to_owned());
            }
        }
    }
}

#[inline]
#[cfg(target_os = "macos")]
pub fn get_dst_align_rgba() -> usize {
    // https://developer.apple.com/forums/thread/712709
    // Memory alignment should be multiple of 64.
    if crate::ui_interface::use_texture_render() {
        64
    } else {
        1
    }
}

#[inline]
#[cfg(not(target_os = "macos"))]
pub fn get_dst_align_rgba() -> usize {
    1
}

pub fn read_custom_client(config: &str) {
    let Ok(data) = decode64(config) else {
        log::error!("Failed to decode custom client config");
        return;
    };
    const KEY: &str = "5Qbwsde3unUcJBtrx9ZkvUmwFNoExHzpryHuPUdqlWM=";
    let Some(pk) = get_rs_pk(KEY) else {
        log::error!("Failed to parse public key of custom client");
        return;
    };
    let Ok(data) = sign::verify(&data, &pk) else {
        log::error!("Failed to dec custom client config");
        return;
    };
    let Ok(mut data) =
        serde_json::from_slice::<std::collections::HashMap<String, serde_json::Value>>(&data)
    else {
        log::error!("Failed to parse custom client config");
        return;
    };

    if let Some(app_name) = data.remove("app-name") {
        if let Some(app_name) = app_name.as_str() {
            *config::APP_NAME.write().unwrap() = app_name.to_owned();
        }
    }

    let mut map_display_settings = HashMap::new();
    for s in keys::KEYS_DISPLAY_SETTINGS {
        map_display_settings.insert(s.replace("_", "-"), s);
    }
    let mut map_local_settings = HashMap::new();
    for s in keys::KEYS_LOCAL_SETTINGS {
        map_local_settings.insert(s.replace("_", "-"), s);
    }
    let mut map_settings = HashMap::new();
    for s in keys::KEYS_SETTINGS {
        map_settings.insert(s.replace("_", "-"), s);
    }
    let mut buildin_settings = HashMap::new();
    for s in keys::KEYS_BUILDIN_SETTINGS {
        buildin_settings.insert(s.replace("_", "-"), s);
    }
    if let Some(default_settings) = data.remove("default-settings") {
        read_custom_client_advanced_settings(
            default_settings,
            &map_display_settings,
            &map_local_settings,
            &map_settings,
            &buildin_settings,
            false,
        );
    }
    if let Some(overwrite_settings) = data.remove("override-settings") {
        read_custom_client_advanced_settings(
            overwrite_settings,
            &map_display_settings,
            &map_local_settings,
            &map_settings,
            &buildin_settings,
            true,
        );
    }
    for (k, v) in data {
        if let Some(v) = v.as_str() {
            config::HARD_SETTINGS
                .write()
                .unwrap()
                .insert(k, v.to_owned());
        };
    }
}
