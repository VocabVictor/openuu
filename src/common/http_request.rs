use super::*;
use super::tcp_proxy::{
    http_proxy_response_to_json, parse_json_header_entries, tcp_proxy_request, with_tcp_proxy_fallback,
};

#[async_recursion]
async fn get_http_response_async(
    url: &str,
    tls_url: &str,
    method: &str,
    body: Option<String>,
    header: &str,
    tls_type: Option<TlsType>,
    danger_accept_invalid_cert: Option<bool>,
    original_danger_accept_invalid_cert: Option<bool>,
) -> ResultType<reqwest::Response> {
    let http_client = create_http_client_async(
        tls_type.unwrap_or(TlsType::Rustls),
        danger_accept_invalid_cert.unwrap_or(false),
    );
    let normalized_method = method.to_ascii_lowercase();
    let mut http_client = match normalized_method.as_str() {
        "get" => http_client.get(url),
        "post" => http_client.post(url),
        "put" => http_client.put(url),
        "delete" => http_client.delete(url),
        _ => return Err(anyhow!("The HTTP request method is not supported!")),
    };
    for entry in parse_json_header_entries(header)? {
        http_client = http_client.header(entry.name, entry.value);
    }

    if tls_type.is_some() && danger_accept_invalid_cert.is_some() {
        if let Some(b) = body {
            http_client = http_client.body(b);
        }
        match http_client
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await
        {
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
        if let Some(b) = body.clone() {
            http_client = http_client.body(b);
        }

        match http_client
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await
        {
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
                        get_http_response_async(
                            url,
                            tls_url,
                            method,
                            body,
                            header,
                            tls_type,
                            Some(true),
                            original_danger_accept_invalid_cert,
                        )
                        .await
                    } else {
                        log::warn!("HTTP request failed: {:?}, try again with native-tls", e);
                        get_http_response_async(
                            url,
                            tls_url,
                            method,
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

/// Returns (status_code, json_string) so the caller can inspect the status
/// without re-parsing the serialized JSON.
async fn http_request_http(
    url: &str,
    method: &str,
    body: Option<String>,
    header: &str,
) -> ResultType<(u16, String)> {
    let proxy_conf = Config::get_socks();
    let tls_url = get_url_for_tls(url, &proxy_conf);
    let tls_type = get_cached_tls_type(tls_url);
    let danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(tls_url);
    let response = get_http_response_async(
        url,
        tls_url,
        method,
        body,
        header,
        tls_type,
        danger_accept_invalid_cert,
        danger_accept_invalid_cert,
    )
    .await?;
    // Serialize response headers
    let mut response_headers = Map::new();
    for (key, value) in response.headers() {
        response_headers.insert(key.to_string(), json!(value.to_str().unwrap_or("")));
    }

    let status_code = response.status().as_u16();
    let response_body = response.text().await?;

    // Construct the JSON object
    let mut result = Map::new();
    result.insert("status_code".to_string(), json!(status_code));
    result.insert("headers".to_string(), Value::Object(response_headers));
    result.insert("body".to_string(), json!(response_body));

    // Convert map to JSON string
    let json_str = serde_json::to_string(&result)
        .map_err(|e| anyhow!("Failed to serialize response: {}", e))?;
    Ok((status_code, json_str))
}

/// HTTP request with raw TCP proxy support.
#[tokio::main(flavor = "current_thread")]
pub async fn http_request_sync(
    url: String,
    method: String,
    body: Option<String>,
    header: String,
) -> ResultType<String> {
    with_tcp_proxy_fallback(
        &url,
        &method,
        http_request_http(&url, &method, body.clone(), &header),
        http_request_via_tcp_proxy(&url, &method, body.as_deref(), &header),
    )
    .await
}

/// General HTTP request via TCP proxy. Header is a JSON string (used by http_request_sync).
/// Returns a JSON string with status_code, headers, body (same format as http_request_sync).
pub(super) async fn http_request_via_tcp_proxy(
    url: &str,
    method: &str,
    body: Option<&str>,
    header: &str,
) -> ResultType<String> {
    let headers = parse_json_header_entries(header)?;
    let body_bytes = body.unwrap_or("").as_bytes();

    let resp = tcp_proxy_request(method, url, body_bytes, headers).await?;
    http_proxy_response_to_json(resp)
}
