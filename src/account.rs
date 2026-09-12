use hbb_common::{bail, config::Config, tokio, ResultType};
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
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .post(format!("{}/api/currentUser", url.trim_end_matches('/')))
        .bearer_auth(&token)
        .send()
        .await?;
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
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()?
        .post(format!(
            "{}/api/relay-ticket",
            Config::get_option("api-server").trim_end_matches('/')
        ))
        .bearer_auth(session_token())
        .json(&serde_json::json!({"uuid":relay_id}))
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("OpenUU relay authorization failed");
    }
    let body: serde_json::Value = response.json().await?;
    match body.get("ticket").and_then(|v| v.as_str()) {
        Some(ticket) if ticket.len() == 64 => Ok(ticket.to_owned()),
        _ => bail!("Invalid relay authorization response"),
    }
}
