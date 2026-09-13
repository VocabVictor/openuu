use super::*;

#[inline]
pub fn check_port<T: std::string::ToString>(host: T, port: i32) -> String {
    hbb_common::socket_client::check_port(host, port)
}

#[inline]
pub fn increase_port<T: std::string::ToString>(host: T, offset: i32) -> String {
    hbb_common::socket_client::increase_port(host, offset)
}

pub const POSTFIX_SERVICE: &'static str = "_service";

pub fn get_custom_rendezvous_server(custom: String) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.host.is_empty() {
            return lic.host.clone();
        }
    }
    if !custom.is_empty() {
        return custom;
    }
    if !config::PROD_RENDEZVOUS_SERVER.read().unwrap().is_empty() {
        return config::PROD_RENDEZVOUS_SERVER.read().unwrap().clone();
    }
    "".to_owned()
}

#[inline]
pub fn get_api_server(api: String, custom: String) -> String {
    if Config::no_register_device() {
        return "".to_owned();
    }
    let mut res = get_api_server_(api, custom);
    if res.ends_with('/') {
        res.pop();
    }
    if res.starts_with("https")
        && res.ends_with(":21114")
        && get_builtin_option(keys::OPTION_ALLOW_HTTPS_21114) != "Y"
    {
        return res.replace(":21114", "");
    }
    res
}

fn get_api_server_(api: String, custom: String) -> String {
    #[cfg(windows)]
    if let Ok(lic) = crate::platform::windows::get_license_from_exe_name() {
        if !lic.api.is_empty() {
            return lic.api.clone();
        }
    }
    if !api.is_empty() {
        return api.to_owned();
    }
    let s0 = get_custom_rendezvous_server(custom);
    if !s0.is_empty() {
        let s = crate::increase_port(&s0, -2);
        if s == s0 {
            return format!("http://{}:{}", s, config::RENDEZVOUS_PORT - 2);
        } else {
            return format!("http://{}", s);
        }
    }
    "https://admin.rustdesk.com".to_owned()
}

#[inline]
pub fn is_public(url: &str) -> bool {
    let parsed = url::Url::parse(url)
        .ok()
        .filter(|parsed| parsed.has_host())
        .or_else(|| url::Url::parse(&format!("http://{url}")).ok());
    let Some(host) = parsed.as_ref().and_then(url::Url::host_str) else {
        return false;
    };
    let host = host.strip_suffix('.').unwrap_or(host);
    host == "rustdesk.com" || host.ends_with(".rustdesk.com")
}

pub fn get_tcp_punch_enabled() -> bool {
    config::option2bool(
        keys::OPTION_ENABLE_TCP_PUNCH,
        &get_local_option(keys::OPTION_ENABLE_TCP_PUNCH),
    )
}

pub fn get_udp_punch_enabled() -> bool {
    config::option2bool(
        keys::OPTION_ENABLE_UDP_PUNCH,
        &get_local_option(keys::OPTION_ENABLE_UDP_PUNCH),
    )
}

pub fn get_ipv6_punch_enabled() -> bool {
    config::option2bool(
        keys::OPTION_ENABLE_IPV6_PUNCH,
        &get_local_option(keys::OPTION_ENABLE_IPV6_PUNCH),
    )
}

pub fn get_webrtc_enabled() -> bool {
    config::option2bool(
        keys::OPTION_ENABLE_WEBRTC,
        &get_local_option(keys::OPTION_ENABLE_WEBRTC),
    )
}

pub fn get_local_option(key: &str) -> String {
    let v = LocalConfig::get_option(key);
    // UDP and IPv6 hole punching used to be switched off here whenever the
    // rendezvous server was not one of the public ones. That read a
    // deployment's shape as a statement about its capability, which it is
    // not: a self-hosted hbbs answers the NAT test and punches like any
    // other, so the effect was that self-hosted deployments silently relayed
    // every session. Both now follow the ordinary `enable-` default of on,
    // and the runtime decides: the NAT probe falls back to TCP punching when
    // it gets no answer, and the transports race with a relay behind them.
    //
    // WebRTC is not the same case and stays off by default here. With no ICE
    // servers of its own it falls back to the built-in public STUN list, so
    // enabling it for a self-hosted deployment would quietly send that
    // deployment's addresses to third parties it never chose. Configure
    // `ice-servers`, or turn the switch on explicitly, and it is honoured.
    if key == keys::OPTION_ENABLE_WEBRTC
        && webrtc_off_by_default(
            &v,
            is_public(&Config::get_rendezvous_server()),
            !Config::get_option(keys::OPTION_ICE_SERVERS).trim().is_empty(),
        )
    {
        return "N".to_owned();
    }
    v
}

/// Split out from `get_local_option` so the decision can be tested without
/// writing the process-global, on-disk configuration that the real getters
/// read; the tests of this crate share one process.
fn webrtc_off_by_default(user_value: &str, server_is_public: bool, has_own_ice_servers: bool) -> bool {
    user_value.is_empty() && !server_is_public && !has_own_ice_servers
}

pub fn get_audit_server(api: String, custom: String, typ: String) -> String {
    let url = get_api_server(api, custom);
    if url.is_empty() || is_public(&url) {
        return "".to_owned();
    }
    format!("{}/api/audit/{}", url, typ)
}

#[inline]
pub fn using_public_server() -> bool {
    crate::get_custom_rendezvous_server(get_option("custom-rendezvous-server")).is_empty()
}

#[cfg(test)]
mod tests;
