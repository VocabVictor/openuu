use super::*;

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
