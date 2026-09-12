use super::*;
use super::tcp_proxy::{
    can_fallback_to_raw_tcp, parse_simple_header, post_request_http, post_request_via_tcp_proxy,
    should_use_raw_tcp_for_api, tcp_proxy_log_target, tcp_proxy_request, with_tcp_proxy_fallback,
};

/// POST request with raw TCP proxy support.
/// - If `USE_RAW_TCP_FOR_API` is "Y" and WS is off, goes directly through TCP proxy.
/// - Otherwise tries HTTP first; on connection failure or 5xx status,
///   falls back to TCP proxy if WS is off.
/// - 4xx responses are returned as-is (server is reachable, business logic error).
/// - If fallback also fails, returns the original HTTP result (text or error).
pub async fn post_request(url: String, body: String, header: &str) -> ResultType<String> {
    with_tcp_proxy_fallback(
        &url,
        "POST",
        post_request_http(&url, &body, header),
        post_request_via_tcp_proxy(&url, &body, header),
    )
    .await
}

/// POST request via TCP proxy, preserving the HTTP status code.
async fn post_request_via_tcp_proxy_status(
    url: &str,
    body: &str,
    header: &str,
) -> ResultType<(u16, String)> {
    let headers = parse_simple_header(header);
    let resp = tcp_proxy_request("POST", url, body.as_bytes(), headers).await?;
    if !resp.error.is_empty() {
        bail!("TCP proxy error: {}", resp.error);
    }
    Ok((
        resp.status as u16,
        String::from_utf8_lossy(&resp.body).to_string(),
    ))
}

/// Like `post_request`, but returns the HTTP status code so callers can tell
/// a server-side failure from success. Same fallback rules: on connection
/// failure or 5xx, retry once through the raw TCP proxy when eligible.
pub async fn post_request_with_status(
    url: String,
    body: String,
    header: &str,
) -> ResultType<(u16, String)> {
    if should_use_raw_tcp_for_api(&url) {
        return post_request_via_tcp_proxy_status(&url, &body, header).await;
    }
    let http_result = post_request_http(&url, &body, header).await;
    let should_fallback = match &http_result {
        Err(_) => true,
        Ok((status, _)) => *status >= 500,
    };
    if should_fallback && can_fallback_to_raw_tcp(&url) {
        log::warn!(
            "HTTP POST to {} failed or 5xx (result: {:?}), trying TCP proxy fallback",
            tcp_proxy_log_target(&url),
            http_result
                .as_ref()
                .map(|(s, _)| *s)
                .map_err(|e| e.to_string()),
        );
        match post_request_via_tcp_proxy_status(&url, &body, header).await {
            Ok(resp) => return Ok(resp),
            Err(tcp_err) => {
                log::warn!("TCP proxy fallback also failed: {:?}", tcp_err);
            }
        }
    }
    http_result
}

#[async_recursion]
pub(super) async fn post_request_(
    url: &str,
    tls_url: &str,
    body: String,
    header: &str,
    tls_type: Option<TlsType>,
    danger_accept_invalid_cert: Option<bool>,
    original_danger_accept_invalid_cert: Option<bool>,
) -> ResultType<reqwest::Response> {
    let mut req = create_http_client_async(
        tls_type.unwrap_or(TlsType::Rustls),
        danger_accept_invalid_cert.unwrap_or(false),
    )
    .post(url);
    if !header.is_empty() {
        let tmp: Vec<&str> = header.split(": ").collect();
        if tmp.len() == 2 {
            req = req.header(tmp[0], tmp[1]);
        }
    }
    req = req.header("Content-Type", "application/json");
    let to = std::time::Duration::from_secs(12);
    if tls_type.is_some() && danger_accept_invalid_cert.is_some() {
        // This branch is used to reduce a `clone()` when both `tls_type` and
        // `danger_accept_invalid_cert` are cached.
        match req.body(body.clone()).timeout(to).send().await {
            Ok(resp) => {
                upsert_tls_cache(
                    tls_url,
                    tls_type.unwrap_or(TlsType::Rustls),
                    danger_accept_invalid_cert.unwrap_or(false),
                );
                Ok(resp)
            }
            Err(e) => Err(anyhow!("{:?}", e)),
        }
    } else {
        match req.body(body.clone()).timeout(to).send().await {
            Ok(resp) => {
                upsert_tls_cache(
                    tls_url,
                    tls_type.unwrap_or(TlsType::Rustls),
                    danger_accept_invalid_cert.unwrap_or(false),
                );
                Ok(resp)
            }
            Err(e) => {
                if (tls_type.is_none() || danger_accept_invalid_cert.is_none()) && e.is_request() {
                    if danger_accept_invalid_cert.is_none() {
                        log::warn!(
                            "HTTP request failed: {:?}, try again, danger accept invalid cert",
                            e
                        );
                        post_request_(
                            url,
                            tls_url,
                            body,
                            header,
                            tls_type,
                            Some(true),
                            original_danger_accept_invalid_cert,
                        )
                        .await
                    } else {
                        log::warn!("HTTP request failed: {:?}, try again with native-tls", e);
                        post_request_(
                            url,
                            tls_url,
                            body,
                            header,
                            Some(TlsType::NativeTls),
                            original_danger_accept_invalid_cert,
                            original_danger_accept_invalid_cert,
                        )
                        .await
                    }
                } else {
                    Err(anyhow!("{:?}", e))
                }
            }
        }
    }
}

#[tokio::main(flavor = "current_thread")]
pub async fn post_request_sync(url: String, body: String, header: &str) -> ResultType<String> {
    post_request(url, body, header).await
}
