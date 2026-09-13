use hbb_common::{anyhow, bail, config::Config, log, tls::TlsType, tokio, ResultType};
use std::time::{Duration, Instant};

pub fn session_token() -> String {
    Config::get_option("openuu-account-token")
}

pub async fn require_login() -> ResultType<()> {
    let token = session_token();
    let url = Config::get_option("api-server");
    if token.is_empty() || url.is_empty() {
        bail!("Sign in to OpenUU before using remote connections");
    }
    let parsed = reqwest::Url::parse(&url)?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        bail!("Configure an HTTP or HTTPS OpenUU account server");
    }
    static CACHE: tokio::sync::Mutex<Option<(String, String, Instant)>> =
        tokio::sync::Mutex::const_new(None);
    let mut cache = CACHE.lock().await;
    if cache
        .as_ref()
        .is_some_and(|(t, u, at)| t == &token && u == &url && at.elapsed() < Duration::from_secs(5))
    {
        return Ok(());
    }
    *cache = None;
    let endpoint = format!("{}/api/currentUser", url.trim_end_matches('/'));
    let response = account_client(&url)
        .post(&endpoint)
        .timeout(Duration::from_secs(5))
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| unreachable_error(&url, e))?;
    if !response.status().is_success() {
        bail!("OpenUU login expired or unavailable; sign in again");
    }
    let user: serde_json::Value = response.json().await?;
    if user.get("status").and_then(|v| v.as_i64()) != Some(1)
        || user
            .get("name")
            .and_then(|v| v.as_str())
            .map_or(true, str::is_empty)
    {
        bail!("OpenUU account is unavailable");
    }
    if session_token() != token || Config::get_option("api-server") != url {
        bail!("OpenUU account changed");
    }
    *cache = Some((token, url, Instant::now()));
    Ok(())
}

pub async fn relay_ticket(relay_id: &str) -> ResultType<String> {
    require_login().await?;
    let url = Config::get_option("api-server");
    let endpoint = format!("{}/api/relay-ticket", url.trim_end_matches('/'));
    let response = account_client(&url)
        .post(&endpoint)
        .timeout(Duration::from_secs(5))
        .bearer_auth(session_token())
        .json(&serde_json::json!({"uuid":relay_id}))
        .send()
        .await
        .map_err(|e| unreachable_error(&url, e))?;
    if !response.status().is_success() {
        bail!("OpenUU relay authorization failed");
    }
    let body: serde_json::Value = response.json().await?;
    match body.get("ticket").and_then(|v| v.as_str()) {
        Some(ticket) if ticket.len() == 64 => Ok(ticket.to_owned()),
        _ => bail!("Invalid relay authorization response"),
    }
}

/// The account server is reached like every other HTTP endpoint of the app:
/// through the proxy configured in the network settings when there is one,
/// otherwise directly. A bare reqwest client would also honour HTTP_PROXY /
/// ALL_PROXY from the environment, which sent these requests through a local
/// proxy that cannot reach the server.
fn account_client(url: &str) -> reqwest::Client {
    let tls = if hbb_common::tls::is_plain(url) {
        TlsType::Plain
    } else {
        TlsType::Rustls
    };
    crate::hbbs_http::create_http_client_async(tls, false)
}

fn unreachable_error(url: &str, err: impl std::fmt::Display) -> anyhow::Error {
    log::error!("event=account_request_error url={url} err={err}");
    let parsed = reqwest::Url::parse(url).ok();
    let host = parsed
        .as_ref()
        .and_then(|u| u.host_str().map(|h| h.to_owned()))
        .unwrap_or_else(|| url.to_owned());
    let port = parsed
        .as_ref()
        .and_then(|u| u.port_or_known_default())
        .unwrap_or(0);
    anyhow::anyhow!("无法连接账号服务器 {host}:{port}，请检查网络或代理设置")
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    #[hbb_common::tokio::test]
    async fn environment_proxy_is_ignored_without_an_app_proxy() {
        // A proxy nobody listens on: a client that honoured it could not reach the server.
        std::env::set_var("HTTP_PROXY", "http://127.0.0.1:9");
        std::env::set_var("ALL_PROXY", "http://127.0.0.1:9");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = s.read(&mut buf).await;
            s.write_all(b"HTTP/1.1 200 OK
Content-Length: 2

ok")
                .await
                .unwrap();
        });
        let url = format!("http://{addr}");
        let res = account_client(&url)
            .post(format!("{url}/api/currentUser"))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        std::env::remove_var("HTTP_PROXY");
        std::env::remove_var("ALL_PROXY");
    }

    #[test]
    fn unreachable_error_names_host_and_port() {
        let err = unreachable_error("http://203.0.113.10:21114", "connection refused");
        assert!(err.to_string().contains("203.0.113.10:21114"), "{err}");
        assert!(!err.to_string().contains("refused"), "the raw error only goes to the log");
        let err = unreachable_error("https://rs.example", "timeout");
        assert!(err.to_string().contains("rs.example:443"), "{err}");
    }
}
