use app_server_core::llm::{
    ResolvedWebSearchProvider, WebSearchAuth, WebSearchHttpMethod, WebSearchParams,
    WebSearchResultMap,
};
use app_server_core::web_search;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

async fn start_mock_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }
                let request = String::from_utf8_lossy(&buf[..n]);
                let (status, content_type, body) = route_request(&request);
                let response = format!(
                    "HTTP/1.1 {status}\r\n\
                     Content-Type: {content_type}\r\n\
                     Content-Length: {}\r\n\
                     Connection: close\r\n\r\n\
                     {body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });
    (port, handle)
}

fn route_request(request: &str) -> (&'static str, &'static str, String) {
    if request.contains("/search") {
        let query = extract_query_param(request, "q").unwrap_or_default();
        let body = serde_json::json!({
            "results": [
                {
                    "title": format!("Result for: {query}"),
                    "url": "https://example.com/result1",
                    "content": format!("Snippet about {query} from the web."),
                    "publishedDate": "2026-05-13",
                    "engine": "mock"
                },
                {
                    "title": "Second result",
                    "url": "https://example.com/result2",
                    "content": "Another relevant snippet.",
                    "engine": "mock"
                }
            ]
        });
        ("200 OK", "application/json", body.to_string())
    } else if request.contains("/page") {
        let html = "\
            <html><head><title>Test Page</title></head>\
            <body>\
            <h1>Integration Test</h1>\
            <p>This is a <strong>test page</strong> for fetch_url verification.</p>\
            <a href=\"https://example.com\">Example link</a>\
            </body></html>";
        ("200 OK", "text/html; charset=utf-8", html.to_owned())
    } else if request.contains("/plain") {
        ("200 OK", "text/plain", "Plain text content.".to_owned())
    } else if request.contains("/large") {
        let body = "x".repeat(3 * 1024 * 1024);
        ("200 OK", "text/plain", body)
    } else {
        ("404 Not Found", "text/plain", "not found".to_owned())
    }
}

fn extract_query_param<'a>(request: &'a str, key: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query_string = path.split('?').nth(1)?;
    for pair in query_string.split('&') {
        let mut kv = pair.splitn(2, '=');
        if kv.next() == Some(key) {
            return kv
                .next()
                .map(|v| urldecode(v));
        }
    }
    None
}

fn urldecode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'+' {
            result.push(' ');
        } else if b == b'%' {
            let hi = chars.next().and_then(|c| (c as char).to_digit(16));
            let lo = chars.next().and_then(|c| (c as char).to_digit(16));
            if let (Some(h), Some(l)) = (hi, lo) {
                result.push((h * 16 + l) as u8 as char);
            }
        } else {
            result.push(b as char);
        }
    }
    result
}

fn test_provider(port: u16) -> ResolvedWebSearchProvider {
    ResolvedWebSearchProvider {
        id: "mock".into(),
        endpoint: format!("http://127.0.0.1:{port}/search"),
        method: WebSearchHttpMethod::Get,
        query_key: "q".into(),
        top_k_param: None,
        auth: WebSearchAuth::None,
        max_result_chars: None,
        user_agent: Some("budn-test/1.0".into()),
        params: WebSearchParams::QueryPairs(vec![("format".into(), "json".into())]),
        result_map: WebSearchResultMap {
            results_path: "results".into(),
            title: "title".into(),
            url: "url".into(),
            snippet: "content".into(),
            date: Some("publishedDate".into()),
            source: Some("engine".into()),
        },
    }
}

#[tokio::test]
async fn execute_web_search_returns_results_from_mock_server() {
    let (port, _handle) = start_mock_server().await;
    let client = reqwest::Client::builder()
        .user_agent("budn-test/1.0")
        .build()
        .unwrap();
    let provider = test_provider(port);

    let results = web_search::execute_web_search(&client, &provider, "rust language", None)
        .await
        .expect("search should succeed");

    assert_eq!(results.len(), 2);
    assert!(results[0].title.contains("rust language"));
    assert_eq!(results[0].url, "https://example.com/result1");
    assert!(results[0].snippet.contains("rust language"));
    assert_eq!(results[0].date.as_deref(), Some("2026-05-13"));
    assert_eq!(results[0].source.as_deref(), Some("mock"));
    assert_eq!(results[1].title, "Second result");
}

#[tokio::test]
async fn fetch_url_converts_html_to_markdown() {
    let (port, _handle) = start_mock_server().await;
    let client = reqwest::Client::new();

    let content = web_search::fetch_url(&client, &format!("http://127.0.0.1:{port}/page"))
        .await
        .expect("fetch should succeed");

    assert!(content.contains("Integration Test"), "should contain heading: {content}");
    assert!(content.contains("**test page**"), "should convert bold: {content}");
    assert!(content.contains("example.com"), "should contain link URL: {content}");
}

#[tokio::test]
async fn fetch_url_returns_plain_text_as_is() {
    let (port, _handle) = start_mock_server().await;
    let client = reqwest::Client::new();

    let content = web_search::fetch_url(&client, &format!("http://127.0.0.1:{port}/plain"))
        .await
        .expect("fetch should succeed");

    assert_eq!(content, "Plain text content.");
}

#[tokio::test]
async fn fetch_url_rejects_oversized_response() {
    let (port, _handle) = start_mock_server().await;
    let client = reqwest::Client::new();

    let result = web_search::fetch_url(&client, &format!("http://127.0.0.1:{port}/large")).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("too large"));
}

#[tokio::test]
async fn fetch_url_reports_404_error() {
    let (port, _handle) = start_mock_server().await;
    let client = reqwest::Client::new();

    let result =
        web_search::fetch_url(&client, &format!("http://127.0.0.1:{port}/nonexistent")).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("404"));
}
