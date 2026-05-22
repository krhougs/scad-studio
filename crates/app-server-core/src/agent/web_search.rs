use crate::llm::{
    ResolvedWebSearchProvider, WebSearchAuth, WebSearchHttpMethod, WebSearchParams,
    WebSearchResultMap,
};

#[derive(Debug)]
pub struct WebSearchError {
    pub message: String,
}

impl std::fmt::Display for WebSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Debug, serde::Serialize)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

pub async fn execute_web_search(
    client: &reqwest::Client,
    provider: &ResolvedWebSearchProvider,
    query: &str,
    top_k: Option<u32>,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    let response_text = send_search_request(client, provider, query, top_k).await?;
    let truncated = truncate_response(&response_text, provider.max_result_chars);
    let json: serde_json::Value =
        serde_json::from_str(truncated).map_err(|e| WebSearchError {
            message: format!("search response is not valid JSON: {e}"),
        })?;
    extract_results(&json, &provider.result_map)
}

async fn send_search_request(
    client: &reqwest::Client,
    provider: &ResolvedWebSearchProvider,
    query: &str,
    top_k: Option<u32>,
) -> Result<String, WebSearchError> {
    let request = match provider.method {
        WebSearchHttpMethod::Get => build_get_request(client, provider, query, top_k),
        WebSearchHttpMethod::Post => build_post_request(client, provider, query, top_k),
    }?;
    let response = request.send().await.map_err(|e| WebSearchError {
        message: format!("search request failed: {e}"),
    })?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let preview = truncate_response(&body, Some(512));
        return Err(WebSearchError {
            message: format!("search API returned {status}: {preview}"),
        });
    }
    response.text().await.map_err(|e| WebSearchError {
        message: format!("failed to read search response: {e}"),
    })
}

fn build_get_request(
    client: &reqwest::Client,
    provider: &ResolvedWebSearchProvider,
    query: &str,
    top_k: Option<u32>,
) -> Result<reqwest::RequestBuilder, WebSearchError> {
    let mut pairs: Vec<(&str, String)> = vec![(&provider.query_key, query.to_owned())];
    if let WebSearchParams::QueryPairs(static_pairs) = &provider.params {
        for (k, v) in static_pairs {
            pairs.push((k, v.clone()));
        }
    }
    if let (Some(param), Some(k)) = (&provider.top_k_param, top_k) {
        pairs.push((param, k.to_string()));
    }
    let mut req = client.get(&provider.endpoint).query(&pairs);
    req = apply_auth(req, &provider.auth)?;
    Ok(req)
}

fn build_post_request(
    client: &reqwest::Client,
    provider: &ResolvedWebSearchProvider,
    query: &str,
    top_k: Option<u32>,
) -> Result<reqwest::RequestBuilder, WebSearchError> {
    let mut body = match &provider.params {
        WebSearchParams::JsonObject(map) => {
            serde_json::Value::Object(map.clone())
        }
        WebSearchParams::QueryPairs(_) => serde_json::Value::Object(Default::default()),
    };
    let obj = body.as_object_mut().unwrap();
    obj.insert(
        provider.query_key.clone(),
        serde_json::Value::String(query.to_owned()),
    );
    if let (Some(param), Some(k)) = (&provider.top_k_param, top_k) {
        obj.insert(param.clone(), serde_json::Value::Number(k.into()));
    }
    let mut req = client.post(&provider.endpoint).json(&body);
    req = apply_auth(req, &provider.auth)?;
    Ok(req)
}

fn apply_auth(
    req: reqwest::RequestBuilder,
    auth: &WebSearchAuth,
) -> Result<reqwest::RequestBuilder, WebSearchError> {
    match auth {
        WebSearchAuth::None => Ok(req),
        WebSearchAuth::Header {
            header,
            prefix,
            api_key,
        } => {
            let value = match prefix {
                Some(p) => format!("{p}{api_key}"),
                None => api_key.clone(),
            };
            Ok(req.header(header, value))
        }
        WebSearchAuth::QueryParam { param, api_key } => {
            Ok(req.query(&[(param.as_str(), api_key.as_str())]))
        }
    }
}

pub fn truncate_response<'a>(text: &'a str, max_bytes: Option<usize>) -> &'a str {
    match max_bytes {
        Some(limit) if text.len() > limit => {
            let mut end = limit;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            &text[..end]
        }
        _ => text,
    }
}

pub fn extract_results(
    json: &serde_json::Value,
    map: &WebSearchResultMap,
) -> Result<Vec<WebSearchResult>, WebSearchError> {
    let array = resolve_dot_path(json, &map.results_path).and_then(|v| v.as_array());
    let Some(items) = array else {
        return Err(WebSearchError {
            message: format!(
                "result_map.results path `{}` did not resolve to an array",
                map.results_path
            ),
        });
    };
    Ok(items.iter().filter_map(|item| extract_one(item, map)).collect())
}

fn extract_one(item: &serde_json::Value, map: &WebSearchResultMap) -> Option<WebSearchResult> {
    let title = item.get(&map.title)?.as_str()?.to_owned();
    let url = item.get(&map.url)?.as_str()?.to_owned();
    let snippet = item.get(&map.snippet).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let date = map
        .date
        .as_deref()
        .and_then(|key| item.get(key))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    let source = map
        .source
        .as_deref()
        .and_then(|key| item.get(key))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    Some(WebSearchResult {
        title,
        url,
        snippet,
        date,
        source,
    })
}

pub fn resolve_dot_path<'a>(
    value: &'a serde_json::Value,
    path: &str,
) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in path.split('.') {
        current = current.get(segment)?;
    }
    Some(current)
}

const FETCH_MAX_BYTES: usize = 2 * 1024 * 1024;
const FETCH_TIMEOUT_SECS: u64 = 30;

pub async fn fetch_url(client: &reqwest::Client, url: &str) -> Result<String, WebSearchError> {
    let response = client
        .get(url)
        .timeout(std::time::Duration::from_secs(FETCH_TIMEOUT_SECS))
        .send()
        .await
        .map_err(|e| WebSearchError {
            message: format!("fetch failed for {url}: {e}"),
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(WebSearchError {
            message: format!("fetch {url} returned {status}"),
        });
    }
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = read_response_body(response).await?;
    if is_html_content_type(&content_type) {
        Ok(html_to_markdown(&body))
    } else {
        Ok(body)
    }
}

async fn read_response_body(mut response: reqwest::Response) -> Result<String, WebSearchError> {
    let url = response.url().to_string();
    if response
        .content_length()
        .is_some_and(|len| len as usize > FETCH_MAX_BYTES)
    {
        return Err(WebSearchError {
            message: format!("response body too large for {url}"),
        });
    }
    let mut buf = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| WebSearchError {
        message: format!("failed to read response body for {url}: {e}"),
    })? {
        buf.extend_from_slice(&chunk);
        if buf.len() > FETCH_MAX_BYTES {
            return Err(WebSearchError {
                message: format!("response body too large ({} bytes) for {url}", buf.len()),
            });
        }
    }
    String::from_utf8(buf).map_err(|_| WebSearchError {
        message: format!("response body is not valid UTF-8 for {url}"),
    })
}

pub fn is_html_content_type(content_type: &str) -> bool {
    let lower = content_type.to_ascii_lowercase();
    lower.contains("text/html") || lower.contains("application/xhtml")
}

pub fn html_to_markdown(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| html.to_owned())
}
