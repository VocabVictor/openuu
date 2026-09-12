use base::config::keys::{self, *};
#[cfg(any(target_os = "android", target_os = "ios"))]
use hbb_common::password_security;
use hbb_common::{
    allow_err,
    bytes::Bytes,
    config::{self, Config, LocalConfig, PeerConfig, CONNECT_TIMEOUT, RENDEZVOUS_PORT},
    directories_next,
    futures::future::join_all,
    log,
    rendezvous_proto::*,
    tokio,
};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::{
    sleep,
    tokio::{sync::mpsc, time},
};
use serde_derive::Serialize;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use std::process::Child;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::common::SOFTWARE_UPDATE_URL;
#[cfg(feature = "flutter")]
use crate::hbbs_http::account;
#[cfg(not(any(target_os = "ios")))]
use crate::ipc;

mod install;
pub use install::*;
mod options;
pub use options::*;
mod status;
pub use status::*;
mod peers;
pub use peers::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use status::check_connect_status;

type Message = RendezvousMessage;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub type Children = Arc<Mutex<(bool, HashMap<(String, String), Child>)>>;

#[derive(Clone, Debug, Serialize)]
pub struct UiStatus {
    pub status_num: i32,
    #[cfg(not(feature = "flutter"))]
    pub key_confirmed: bool,
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    pub mouse_time: i64,
    #[cfg(not(feature = "flutter"))]
    pub id: String,
    #[cfg(feature = "flutter")]
    pub video_conn_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoginDeviceInfo {
    pub os: String,
    pub r#type: String,
    pub name: String,
}

lazy_static::lazy_static! {
    static ref UI_STATUS : Arc<Mutex<UiStatus>> = Arc::new(Mutex::new(UiStatus{
        status_num: 0,
        #[cfg(not(feature = "flutter"))]
        key_confirmed: false,
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        mouse_time: 0,
        #[cfg(not(feature = "flutter"))]
        id: "".to_owned(),
        #[cfg(feature = "flutter")]
        video_conn_count: 0,
    }));
    static ref ASYNC_JOB_STATUS : Arc<Mutex<String>> = Default::default();
    static ref ASYNC_HTTP_STATUS : Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref TEMPORARY_PASSWD : Arc<Mutex<String>> = Arc::new(Mutex::new("".to_owned()));
    static ref IS_REMOTE_MODIFY_ENABLED_BY_CONTROL_PERMISSIONS : Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
lazy_static::lazy_static! {
    static ref OPTION_SYNCED: Arc<Mutex<bool>> = Default::default();
    static ref OPTIONS : Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(Config::get_options()));
    pub static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(check_connect_status(true));
    static ref CHILDREN : Children = Default::default();
}

#[cfg(target_os = "windows")]
lazy_static::lazy_static! {
    pub static ref IS_FILE_TRANSFER_ENABLED: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
}

const INIT_ASYNC_JOB_STATUS: &str = " ";

#[inline]
#[cfg(feature = "flutter")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn get_sound_inputs() -> Vec<String> {
    let mut a = Vec::new();
    #[cfg(not(target_os = "linux"))]
    {
        fn get_sound_inputs_() -> Vec<String> {
            let mut out = Vec::new();
            use cpal::traits::{DeviceTrait, HostTrait};
            // Do not use `cpal::host_from_id(cpal::HostId::ScreenCaptureKit)` for feature = "screencapturekit"
            // Because we explicitly handle the "System Sound" device.
            let host = cpal::default_host();
            if let Ok(devices) = host.devices() {
                for device in devices {
                    if device.default_input_config().is_err() {
                        continue;
                    }
                    if let Ok(name) = device.name() {
                        out.push(name);
                    }
                }
            }
            out
        }

        let inputs = Arc::new(Mutex::new(Vec::new()));
        let cloned = inputs.clone();
        // can not call below in UI thread, because conflict with sciter sound com initialization
        std::thread::spawn(move || *cloned.lock().unwrap() = get_sound_inputs_())
            .join()
            .ok();
        for name in inputs.lock().unwrap().drain(..) {
            a.push(name);
        }
    }
    #[cfg(target_os = "linux")]
    {
        let inputs: Vec<String> = crate::platform::linux::get_pa_sources()
            .drain(..)
            .map(|x| x.1)
            .collect();

        for name in inputs {
            a.push(name);
        }
    }
    a
}

#[inline]
pub fn get_socks() -> Vec<String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let s = ipc::get_socks();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let s = Config::get_socks();
    match s {
        None => Vec::new(),
        Some(s) => {
            let mut v = Vec::new();
            v.push(s.proxy);
            v.push(s.username);
            v.push(s.password);
            v
        }
    }
}

#[inline]
pub fn set_socks(proxy: String, username: String, password: String) {
    let socks = config::Socks5Server {
        proxy,
        username,
        password,
    };
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc::set_socks(socks).ok();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _nat = crate::CheckTestNatType::new();
        if socks.proxy.is_empty() {
            Config::set_socks(None);
        } else {
            Config::set_socks(Some(socks));
        }
        log::info!("socks updated");
    }
    #[cfg(target_os = "android")]
    {
        crate::RendezvousMediator::restart();
    }
}

#[inline]
#[cfg(feature = "flutter")]
pub fn get_proxy_status() -> bool {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return ipc::get_proxy_status();

    // Currently, only the desktop version has proxy settings.
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return false;
}

#[inline]
pub fn is_process_trusted(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_process_trusted(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
pub fn is_can_screen_recording(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_can_screen_recording(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
pub fn is_installed_daemon(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_installed_daemon(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
#[cfg(feature = "flutter")]
pub fn is_can_input_monitoring(_prompt: bool) -> bool {
    #[cfg(target_os = "macos")]
    return crate::platform::macos::is_can_input_monitoring(_prompt);
    #[cfg(not(target_os = "macos"))]
    return true;
}

#[inline]
pub fn get_error() -> String {
    #[cfg(target_os = "linux")]
    {
        let dtype = crate::platform::linux::get_display_server();
        if crate::platform::linux::DISPLAY_SERVER_WAYLAND == dtype {
            return crate::server::wayland::common_get_error();
        }
        if dtype != crate::platform::linux::DISPLAY_SERVER_X11 {
            return format!(
                "{} {}, {}",
                crate::client::translate("Unsupported display server".to_owned()),
                dtype,
                crate::client::translate("x11 expected".to_owned()),
            );
        }
    }
    return "".to_owned();
}

#[inline]
pub fn is_login_wayland() -> bool {
    #[cfg(target_os = "linux")]
    return crate::platform::linux::is_login_wayland();
    #[cfg(not(target_os = "linux"))]
    return false;
}

#[inline]
pub fn current_is_wayland() -> bool {
    #[cfg(target_os = "linux")]
    return crate::platform::linux::current_is_wayland();
    #[cfg(not(target_os = "linux"))]
    return false;
}

#[inline]
pub fn get_new_version() -> String {
    (*SOFTWARE_UPDATE_URL
        .lock()
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap_or(""))
    .to_string()
}

#[inline]
pub fn get_version() -> String {
    crate::VERSION.to_owned()
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn get_app_name() -> String {
    crate::get_app_name()
}

#[cfg(windows)]
#[inline]
pub fn create_shortcut(_id: String) {
    crate::platform::windows::create_shortcut(&_id).ok();
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn discover() {
    std::thread::spawn(move || {
        allow_err!(crate::lan::discover());
    });
}

#[inline]
pub fn http_request(url: String, method: String, body: Option<String>, header: String) {
    // Respond to concurrent requests for resources
    let current_request = ASYNC_HTTP_STATUS.clone();
    current_request
        .lock()
        .unwrap()
        .insert(url.clone(), " ".to_owned());
    std::thread::spawn(move || {
        let res = match crate::http_request_sync(url.clone(), method, body, header) {
            Err(err) => {
                log::error!("{}", err);
                err.to_string()
            }
            Ok(text) => text,
        };
        current_request.lock().unwrap().insert(url, res);
    });
}

#[inline]
pub fn get_async_http_status(url: String) -> Option<String> {
    match ASYNC_HTTP_STATUS.lock().unwrap().get(&url) {
        None => None,
        Some(_str) => Some(_str.to_string()),
    }
}

#[inline]
#[cfg(not(feature = "flutter"))]
pub fn post_request(url: String, body: String, header: String) {
    *ASYNC_JOB_STATUS.lock().unwrap() = " ".to_owned();
    std::thread::spawn(move || {
        *ASYNC_JOB_STATUS.lock().unwrap() = match crate::post_request_sync(url, body, &header) {
            Err(err) => err.to_string(),
            Ok(text) => text,
        };
    });
}

#[inline]
pub fn get_async_job_status() -> String {
    ASYNC_JOB_STATUS.lock().unwrap().clone()
}

#[inline]
pub fn get_langs() -> String {
    use serde_json::json;
    let hide_cjk = crate::lang::cjk_ui_unavailable();
    let mut x: Vec<(&str, String)> = crate::lang::LANGS
        .iter()
        .filter(|a| !hide_cjk || !crate::lang::is_cjk_lang(a.0))
        .map(|a| (a.0, format!("{} ({})", a.1, a.0)))
        .collect();
    x.sort_by(|a, b| a.0.cmp(b.0));
    json!(x).to_string()
}

// Preserve relative paths for existing configurations and only remove accidental
// surrounding whitespace. Config values are not shell-expanded (for example, `~`).
fn trim_video_save_directory(value: &str) -> Option<&str> {
    let value = value.trim();
    if !value.is_empty() {
        Some(value)
    } else {
        None
    }
}

// A Windows service typically runs with System32 as its working directory, so
// require an absolute path to avoid resolving recordings there unexpectedly.
#[cfg(any(windows, test))]
fn validate_windows_service_video_save_directory(value: &str) -> Option<&str> {
    let value = trim_video_save_directory(value)?;
    if std::path::Path::new(value).is_absolute() {
        Some(value)
    } else {
        None
    }
}

#[inline]
pub fn video_save_directory(root: bool) -> String {
    let appname = crate::get_app_name();
    // ui process can show it correctly Once vidoe process created it.
    let try_create = |path: &std::path::Path| {
        if !path.exists() {
            std::fs::create_dir_all(path).ok();
        }
        if path.exists() {
            path.to_string_lossy().to_string()
        } else {
            "".to_string()
        }
    };

    if root {
        // Currently, only installed windows run as root
        #[cfg(windows)]
        {
            let dir = Config::get_option(OPTION_WINDOWS_SERVICE_VIDEO_SAVE_DIRECTORY);
            if let Some(dir) = validate_windows_service_video_save_directory(&dir) {
                return dir.to_owned();
            }
            if !dir.trim().is_empty() {
                log::warn!(
                    "Ignoring {OPTION_WINDOWS_SERVICE_VIDEO_SAVE_DIRECTORY}: path must be absolute"
                );
            }
            let drive = std::env::var("SystemDrive").unwrap_or("C:".to_owned());
            let dir =
                std::path::PathBuf::from(format!("{drive}\\ProgramData\\{appname}\\recording",));
            return dir.to_string_lossy().to_string();
        }
    }
    // Get directory from config file otherwise --server will use the old value from global var.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    let dir = LocalConfig::get_option_from_file(OPTION_VIDEO_SAVE_DIRECTORY);
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    let dir = LocalConfig::get_option(OPTION_VIDEO_SAVE_DIRECTORY);
    if let Some(dir) = trim_video_save_directory(&dir) {
        return dir.to_owned();
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    if let Ok(home) = config::APP_HOME_DIR.read() {
        let mut path = home.to_owned();
        path.push_str(format!("/{appname}/ScreenRecord").as_str());
        let dir = try_create(&std::path::Path::new(&path));
        if !dir.is_empty() {
            return dir;
        }
    }

    if let Some(user) = directories_next::UserDirs::new() {
        if let Some(video_dir) = user.video_dir() {
            let dir = try_create(&video_dir.join(&appname));
            if !dir.is_empty() {
                return dir;
            }
            if video_dir.exists() {
                return video_dir.to_string_lossy().to_string();
            }
        }
        if let Some(desktop_dir) = user.desktop_dir() {
            if desktop_dir.exists() {
                return desktop_dir.to_string_lossy().to_string();
            }
        }
        let home = user.home_dir();
        if home.exists() {
            return home.to_string_lossy().to_string();
        }
    }

    // same order as above
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Some(home) = crate::platform::get_active_user_home() {
        let name = if cfg!(target_os = "macos") {
            "Movies"
        } else {
            "Videos"
        };
        let video_dir = home.join(name);
        let dir = try_create(&video_dir.join(&appname));
        if !dir.is_empty() {
            return dir;
        }
        if video_dir.exists() {
            return video_dir.to_string_lossy().to_string();
        }
        let desktop_dir = home.join("Desktop");
        if desktop_dir.exists() {
            return desktop_dir.to_string_lossy().to_string();
        }
        if home.exists() {
            return home.to_string_lossy().to_string();
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let dir = try_create(&parent.join("videos"));
            if !dir.is_empty() {
                return dir;
            }
            // basically exist
            return parent.to_string_lossy().to_string();
        }
    }
    Default::default()
}

#[inline]
pub fn get_api_server() -> String {
    crate::get_api_server(
        get_option("api-server"),
        get_option("custom-rendezvous-server"),
    )
}

pub enum DeployResult {
    Ok,
    NotEnabled,
    InvalidInput,
    IdTaken(String),
    Error(String),
}

impl DeployResult {
    pub fn message(&self) -> String {
        match self {
            Self::Ok => "".to_owned(),
            Self::NotEnabled => "The server does not require explicit deployment.".to_owned(),
            Self::InvalidInput => "Invalid input.".to_owned(),
            Self::IdTaken(id) => {
                format!(
                    "Id `{}` is already used by another machine on the server.",
                    id
                )
            }
            Self::Error(err) => err.clone(),
        }
    }
}

pub fn deploy_device(token: String, new_id: Option<String>) -> DeployResult {
    if Config::no_register_device() {
        return DeployResult::Error("Cannot deploy an unregistrable device!".to_owned());
    }
    let token = token.trim();
    if token.is_empty() {
        return DeployResult::Error("token is required!".to_owned());
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let local_id = Config::get_id();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let local_id = ipc::get_id();
    let id_to_deploy = new_id.clone().unwrap_or_else(|| local_id.clone());
    let uuid = crate::encode64(hbb_common::get_uuid());
    let pk = crate::encode64(Config::get_key_pair().1);
    let body = serde_json::json!({
        "id": id_to_deploy,
        "uuid": uuid,
        "pk": pk,
    });
    let header = "Authorization: Bearer ".to_owned() + token;
    let url = get_api_server() + "/api/devices/deploy";
    let text = match crate::post_request_sync(url, body.to_string(), &header) {
        Ok(text) => text,
        Err(err) => return DeployResult::Error(format!("Request failed: {}", err)),
    };
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    match parsed["result"].as_str().unwrap_or("") {
        "OK" => {
            if let Some(new_id) = new_id {
                if new_id != local_id {
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    {
                        Config::set_key_confirmed(false);
                        Config::set_id(&new_id);
                    }
                    #[cfg(not(any(target_os = "android", target_os = "ios")))]
                    if let Err(err) = ipc::set_config("id", new_id) {
                        return DeployResult::Error(format!(
                            "Failed to persist deployed id locally: {}",
                            err
                        ));
                    }
                }
            }
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            if let Err(err) = ipc::notify_deployed() {
                log::warn!("Failed to notify deployed state: {}", err);
            }
            #[cfg(target_os = "android")]
            {
                crate::rendezvous_mediator::NEEDS_DEPLOY
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                crate::rendezvous_mediator::reset_needs_deploy_notification();
                crate::rendezvous_mediator::RendezvousMediator::restart();
            }
            DeployResult::Ok
        }
        "NOT_ENABLED" => DeployResult::NotEnabled,
        "INVALID_INPUT" => DeployResult::InvalidInput,
        "ID_TAKEN" => DeployResult::IdTaken(id_to_deploy),
        _ => {
            if text.is_empty() {
                DeployResult::Error("Unknown response.".to_owned())
            } else {
                DeployResult::Error(text)
            }
        }
    }
}

#[inline]
pub fn has_hwcodec() -> bool {
    // Has real hardware codec using gpu
    (cfg!(feature = "hwcodec") && cfg!(not(target_os = "ios"))) || cfg!(feature = "mediacodec")
}

#[inline]
pub fn has_vram() -> bool {
    cfg!(feature = "vram")
}

#[cfg(feature = "flutter")]
#[inline]
pub fn supported_hwdecodings() -> (bool, bool) {
    let decoding =
        scrap::codec::Decoder::supported_decodings(None, use_texture_render(), None, &vec![]);
    #[allow(unused_mut)]
    let (mut h264, mut h265) = (decoding.ability_h264 > 0, decoding.ability_h265 > 0);
    #[cfg(feature = "vram")]
    {
        // supported_decodings check runtime luid
        let vram = scrap::vram::VRamDecoder::possible_available_without_check();
        if vram.0 {
            h264 = true;
        }
        if vram.1 {
            h265 = true;
        }
    }
    (h264, h265)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[inline]
pub fn is_root() -> bool {
    crate::platform::is_root()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
#[inline]
pub fn is_root() -> bool {
    false
}

#[cfg(any(target_os = "android", target_os = "ios", feature = "flutter"))]
#[inline]
pub fn check_super_user_permission() -> bool {
    #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
    return crate::platform::check_super_user_permission().unwrap_or(false);
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    return true;
}

#[cfg(not(any(target_os = "android", target_os = "ios", feature = "flutter")))]
pub fn check_zombie() {
    let mut deads = Vec::new();
    loop {
        let mut lock = CHILDREN.lock().unwrap();
        let mut n = 0;
        for (id, c) in lock.1.iter_mut() {
            if let Ok(Some(_)) = c.try_wait() {
                deads.push(id.clone());
                n += 1;
            }
        }
        for ref id in deads.drain(..) {
            lock.1.remove(id);
        }
        if n > 0 {
            lock.0 = true;
        }
        drop(lock);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

#[inline]
#[cfg(not(any(target_os = "android", target_os = "ios", feature = "flutter")))]
pub fn recent_sessions_updated() -> bool {
    let mut children = CHILDREN.lock().unwrap();
    if children.0 {
        children.0 = false;
        true
    } else {
        false
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios", feature = "flutter")))]
pub fn new_remote(id: String, remote_type: String, force_relay: bool) {
    let mut lock = CHILDREN.lock().unwrap();
    let mut args = vec![format!("--{}", remote_type), id.clone()];
    if force_relay {
        args.push("".to_string()); // password
        args.push("--relay".to_string());
    }
    let key = (id.clone(), remote_type.clone());
    if let Some(c) = lock.1.get_mut(&key) {
        if let Ok(Some(_)) = c.try_wait() {
            lock.1.remove(&key);
        } else {
            if remote_type == "rdp" {
                allow_err!(c.kill());
                std::thread::sleep(std::time::Duration::from_millis(30));
                c.try_wait().ok();
                lock.1.remove(&key);
            } else {
                return;
            }
        }
    }
    match crate::run_me(args) {
        Ok(child) => {
            lock.1.insert(key, child);
        }
        Err(err) => {
            log::error!("Failed to spawn remote: {}", err);
        }
    }
}

#[cfg(feature = "flutter")]
pub fn account_auth(op: String, id: String, uuid: String, remember_me: bool) {
    account::OidcSession::account_auth(get_api_server(), op, id, uuid, remember_me);
}

#[cfg(feature = "flutter")]
pub fn account_auth_cancel() {
    account::OidcSession::auth_cancel();
}

#[cfg(feature = "flutter")]
pub fn account_auth_result() -> String {
    serde_json::to_string(&account::OidcSession::get_result()).unwrap_or_default()
}

#[cfg(feature = "flutter")]
pub fn set_user_default_option(key: String, value: String) {
    use hbb_common::config::UserDefaultConfig;
    UserDefaultConfig::load().set(key, value);
}

#[cfg(feature = "flutter")]
pub fn get_user_default_option(key: String) -> String {
    use hbb_common::config::UserDefaultConfig;
    UserDefaultConfig::load().get(&key)
}

pub fn get_fingerprint() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    if Config::get_key_confirmed() {
        return crate::common::pk_to_fingerprint(Config::get_key_pair().1);
    } else {
        return "".to_owned();
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return ipc::get_fingerprint();
}

#[inline]
pub fn get_login_device_info() -> LoginDeviceInfo {
    LoginDeviceInfo {
        // std::env::consts::OS is better than whoami::platform() here.
        os: std::env::consts::OS.to_owned(),
        r#type: "client".to_owned(),
        name: crate::common::hostname(),
    }
}

#[inline]
pub fn get_login_device_info_json() -> String {
    serde_json::to_string(&get_login_device_info()).unwrap_or("{}".to_string())
}

pub fn support_remove_wallpaper() -> bool {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    return crate::platform::WallPaperRemover::support();
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    return false;
}

pub fn has_valid_2fa() -> bool {
    let raw = get_option("2fa");
    crate::auth_2fa::get_2fa(Some(raw)).is_some()
}

pub fn generate2fa() -> String {
    crate::auth_2fa::generate2fa()
}

pub fn verify2fa(code: String) -> bool {
    let res = crate::auth_2fa::verify2fa(code);
    if res {
        refresh_options();
    }
    res
}

pub fn has_valid_bot() -> bool {
    crate::auth_2fa::TelegramBot::get().map_or(false, |bot| bot.is_some())
}

pub fn verify_bot(token: String) -> String {
    match crate::auth_2fa::get_chatid_telegram(&token) {
        Err(err) => err.to_string(),
        Ok(None) => {
            "To activate the bot, simply send a message beginning with a forward slash (\"/\") like \"/hello\" to its chat.".to_owned()
        }
        _ => "".to_owned(),
    }
}

pub fn check_hwcodec() {
    #[cfg(feature = "hwcodec")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        use std::sync::Once;
        static ONCE: Once = Once::new();

        ONCE.call_once(|| {
            if crate::platform::is_installed() {
                ipc::notify_server_to_check_hwcodec().ok();
                ipc::client_get_hwcodec_config_thread(3);
            } else {
                scrap::hwcodec::start_check_process();
            }
        })
    }
}

#[cfg(feature = "flutter")]
pub fn get_unlock_pin() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return String::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return ipc::get_unlock_pin();
}

#[cfg(feature = "flutter")]
pub fn set_unlock_pin(pin: String) -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return String::default();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    match ipc::set_unlock_pin(pin, true) {
        Ok(_) => String::default(),
        Err(err) => err.to_string(),
    }
}

#[cfg(feature = "flutter")]
pub fn get_trusted_devices() -> String {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Config::get_trusted_devices_json();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return ipc::get_trusted_devices();
}

#[cfg(feature = "flutter")]
pub fn remove_trusted_devices(json: &str) {
    let hwids = serde_json::from_str::<Vec<Bytes>>(json).unwrap_or_default();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Config::remove_trusted_devices(&hwids);
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc::remove_trusted_devices(hwids);
}

#[cfg(feature = "flutter")]
pub fn clear_trusted_devices() {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    Config::clear_trusted_devices();
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    ipc::clear_trusted_devices();
}

#[cfg(feature = "flutter")]
pub fn max_encrypt_len() -> usize {
    hbb_common::config::ENCRYPT_MAX_LEN
}

pub fn is_remote_modify_enabled_by_control_permissions() -> Option<bool> {
    *IS_REMOTE_MODIFY_ENABLED_BY_CONTROL_PERMISSIONS
        .lock()
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::{trim_video_save_directory, validate_windows_service_video_save_directory};

    #[test]
    fn trim_configured_video_save_directory() {
        assert_eq!(
            trim_video_save_directory("  relative/recordings  "),
            Some("relative/recordings")
        );
        assert_eq!(trim_video_save_directory("  "), None);
    }

    #[test]
    fn validate_service_video_save_directory() {
        let absolute = if cfg!(windows) {
            r"C:\recordings"
        } else {
            "/recordings"
        };
        let padded = format!("  {absolute}  ");

        assert_eq!(
            validate_windows_service_video_save_directory(&padded),
            Some(absolute)
        );
        assert_eq!(
            validate_windows_service_video_save_directory("recordings"),
            None
        );
        assert_eq!(
            validate_windows_service_video_save_directory(&format!("\"{absolute}\"")),
            None
        );
        assert_eq!(validate_windows_service_video_save_directory("  "), None);
    }
}
