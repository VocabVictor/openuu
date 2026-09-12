use super::*;

/// Check if we should use raw TCP proxy for API calls.
/// Returns true if USE_RAW_TCP_FOR_API builtin option is "Y", WebSocket is off,
/// and the target URL belongs to the configured non-public API host.
#[inline]
pub(super) fn should_use_raw_tcp_for_api(url: &str) -> bool {
    get_builtin_option(keys::OPTION_USE_RAW_TCP_FOR_API) == "Y"
        && !use_ws()
        && is_tcp_proxy_api_target(url)
}

/// Check if we can attempt raw TCP proxy fallback for this target URL.
#[inline]
pub(super) fn can_fallback_to_raw_tcp(url: &str) -> bool {
    !use_ws() && is_tcp_proxy_api_target(url)
}

#[inline]
fn should_use_tcp_proxy_for_api_url(url: &str, api_url: &str) -> bool {
    if api_url.is_empty() || is_public(api_url) {
        return false;
    }

    let target_host = url::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()));
    let api_host = url::Url::parse(api_url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(|host| host.to_ascii_lowercase()));

    matches!((target_host, api_host), (Some(target), Some(api)) if target == api)
}

#[inline]
fn is_tcp_proxy_api_target(url: &str) -> bool {
    should_use_tcp_proxy_for_api_url(url, &ui_get_api_server())
}

pub(super) fn tcp_proxy_log_target(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .map(|parsed| {
            let mut redacted = format!("{}://", parsed.scheme());
            let Some(host) = parsed.host() else {
                return "<invalid-url>".to_owned();
            };
            redacted.push_str(&host.to_string());
            if let Some(port) = parsed.port() {
                redacted.push(':');
                redacted.push_str(&port.to_string());
            }
            redacted.push_str(parsed.path());
            redacted
        })
        .unwrap_or_else(|| "<invalid-url>".to_owned())
}

#[inline]
fn get_tcp_proxy_addr() -> String {
    check_port(Config::get_rendezvous_server(), RENDEZVOUS_PORT)
}

/// Send an HTTP request via the rendezvous server's TCP proxy using protobuf.
/// Connects with `connect_tcp` + `secure_tcp`, sends `HttpProxyRequest`,
/// receives `HttpProxyResponse`.
///
/// The entire operation (connect + handshake + send + receive) is wrapped in
/// an overall timeout of `CONNECT_TIMEOUT + READ_TIMEOUT` so that a stall at
/// any stage cannot block the caller indefinitely.
pub(super) async fn tcp_proxy_request(
    method: &str,
    url: &str,
    body: &[u8],
    headers: Vec<HeaderEntry>,
) -> ResultType<HttpProxyResponse> {
    let tcp_addr = get_tcp_proxy_addr();
    if tcp_addr.is_empty() {
        bail!("No rendezvous server configured for TCP proxy");
    }

    let parsed = url::Url::parse(url)?;
    let path = if let Some(query) = parsed.query() {
        format!("{}?{}", parsed.path(), query)
    } else {
        parsed.path().to_string()
    };

    log::debug!(
        "Sending {} {} via TCP proxy to {}",
        method,
        parsed.path(),
        tcp_addr
    );

    let overall_timeout = CONNECT_TIMEOUT + READ_TIMEOUT;
    timeout(overall_timeout, async {
        let mut conn = socket_client::connect_tcp(&*tcp_addr, CONNECT_TIMEOUT).await?;
        let key = crate::get_key(true).await;
        secure_tcp_silent(&mut conn, &key).await?;

        let mut req = HttpProxyRequest::new();
        req.method = method.to_uppercase();
        req.path = path;
        req.headers = headers.into();
        req.body = Bytes::from(body.to_vec());

        let mut msg_out = RendezvousMessage::new();
        msg_out.set_http_proxy_request(req);
        conn.send(&msg_out).await?;

        match conn.next().await {
            Some(Ok(bytes)) => {
                let msg_in = RendezvousMessage::parse_from_bytes(&bytes)?;
                match msg_in.union {
                    Some(rendezvous_message::Union::HttpProxyResponse(resp)) => Ok(resp),
                    _ => bail!("Unexpected response from TCP proxy"),
                }
            }
            Some(Err(e)) => bail!("TCP proxy read error: {}", e),
            None => bail!("TCP proxy connection closed without response"),
        }
    })
    .await?
}

/// Build HeaderEntry list from "Key: Value" style header string (used by post_request).
/// If the caller supplies a Content-Type header it overrides the default `application/json`.
pub(super) fn parse_simple_header(header: &str) -> Vec<HeaderEntry> {
    let mut entries = Vec::new();
    let mut has_content_type = false;
    if !header.is_empty() {
        let tmp: Vec<&str> = header.splitn(2, ": ").collect();
        if tmp.len() == 2 {
            if tmp[0].eq_ignore_ascii_case("Content-Type") {
                has_content_type = true;
            }
            entries.push(HeaderEntry {
                name: tmp[0].into(),
                value: tmp[1].into(),
                ..Default::default()
            });
        }
    }
    if !has_content_type {
        entries.insert(
            0,
            HeaderEntry {
                name: "Content-Type".into(),
                value: "application/json".into(),
                ..Default::default()
            },
        );
    }
    entries
}

/// POST request via TCP proxy.
pub(super) async fn post_request_via_tcp_proxy(url: &str, body: &str, header: &str) -> ResultType<String> {
    let headers = parse_simple_header(header);
    let resp = tcp_proxy_request("POST", url, body.as_bytes(), headers).await?;
    if !resp.error.is_empty() {
        bail!("TCP proxy error: {}", resp.error);
    }
    Ok(String::from_utf8_lossy(&resp.body).to_string())
}

pub(super) fn http_proxy_response_to_json(resp: HttpProxyResponse) -> ResultType<String> {
    if !resp.error.is_empty() {
        bail!("TCP proxy error: {}", resp.error);
    }

    let mut response_headers = Map::new();
    for entry in resp.headers.iter() {
        response_headers.insert(entry.name.to_lowercase(), json!(entry.value));
    }

    let mut result = Map::new();
    result.insert("status_code".to_string(), json!(resp.status));
    result.insert("headers".to_string(), Value::Object(response_headers));
    result.insert(
        "body".to_string(),
        json!(String::from_utf8_lossy(&resp.body)),
    );

    serde_json::to_string(&result).map_err(|e| anyhow!("Failed to serialize response: {}", e))
}

pub(super) fn parse_json_header_entries(header: &str) -> ResultType<Vec<HeaderEntry>> {
    let v: Value = serde_json::from_str(header)?;
    if let Value::Object(obj) = v {
        Ok(obj
            .iter()
            .map(|(key, value)| HeaderEntry {
                name: key.clone(),
                value: value.as_str().unwrap_or_default().into(),
                ..Default::default()
            })
            .collect())
    } else {
        Err(anyhow!("HTTP header information parsing failed!"))
    }
}

/// Returns (status_code, body_text). Separating status so the wrapper can decide on fallback.
pub(super) async fn post_request_http(url: &str, body: &str, header: &str) -> ResultType<(u16, String)> {
    let proxy_conf = Config::get_socks();
    let tls_url = get_url_for_tls(url, &proxy_conf);
    let tls_type = get_cached_tls_type(tls_url);
    let danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(tls_url);
    let response = post_request_(
        url,
        tls_url,
        body.to_owned(),
        header,
        tls_type,
        danger_accept_invalid_cert,
        danger_accept_invalid_cert,
    )
    .await?;
    let status = response.status().as_u16();
    let text = response.text().await?;
    Ok((status, text))
}

/// Try `http_fn` first; on connection failure or 5xx, fall back to `tcp_fn`
/// if the URL is eligible. 4xx responses are returned as-is.
pub(super) async fn with_tcp_proxy_fallback<HttpFut, TcpFut>(
    url: &str,
    method: &str,
    http_fn: HttpFut,
    tcp_fn: TcpFut,
) -> ResultType<String>
where
    HttpFut: Future<Output = ResultType<(u16, String)>>,
    TcpFut: Future<Output = ResultType<String>>,
{
    if should_use_raw_tcp_for_api(url) {
        return tcp_fn.await;
    }

    let http_result = http_fn.await;
    let should_fallback = match &http_result {
        Err(_) => true,
        Ok((status, _)) => *status >= 500,
    };

    if should_fallback && can_fallback_to_raw_tcp(url) {
        log::warn!(
            "HTTP {} to {} failed or 5xx (result: {:?}), trying TCP proxy fallback",
            method,
            tcp_proxy_log_target(url),
            http_result
                .as_ref()
                .map(|(s, _)| *s)
                .map_err(|e| e.to_string()),
        );
        match tcp_fn.await {
            Ok(resp) => return Ok(resp),
            Err(tcp_err) => {
                log::warn!("TCP proxy fallback also failed: {:?}", tcp_err);
            }
        }
    }

    http_result.map(|(_status, text)| text)
}

#[cfg(test)]
mod tests;
