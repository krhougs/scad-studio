use app_server_core::llm::WebSearchResultMap;
use app_server_core::web_search::{
    WebSearchResult, extract_results, html_to_markdown, is_html_content_type, resolve_dot_path,
    truncate_response,
};

fn searxng_result_map() -> WebSearchResultMap {
    WebSearchResultMap {
        results_path: "results".into(),
        title: "title".into(),
        url: "url".into(),
        snippet: "content".into(),
        date: Some("publishedDate".into()),
        source: Some("engine".into()),
    }
}

fn brave_result_map() -> WebSearchResultMap {
    WebSearchResultMap {
        results_path: "web.results".into(),
        title: "title".into(),
        url: "url".into(),
        snippet: "description".into(),
        date: None,
        source: None,
    }
}

#[test]
fn extract_results_from_searxng_response() {
    let json: serde_json::Value = serde_json::json!({
        "results": [
            {
                "title": "Rust lang",
                "url": "https://rust-lang.org",
                "content": "A systems programming language",
                "publishedDate": "2024-01-15",
                "engine": "google"
            },
            {
                "title": "Rust book",
                "url": "https://doc.rust-lang.org/book/",
                "content": "The official Rust book",
                "engine": "bing"
            }
        ]
    });
    let results = extract_results(&json, &searxng_result_map()).unwrap();
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].title, "Rust lang");
    assert_eq!(results[0].url, "https://rust-lang.org");
    assert_eq!(results[0].snippet, "A systems programming language");
    assert_eq!(results[0].date.as_deref(), Some("2024-01-15"));
    assert_eq!(results[0].source.as_deref(), Some("google"));
    assert!(results[1].date.is_none());
    assert_eq!(results[1].source.as_deref(), Some("bing"));
}

#[test]
fn extract_results_from_brave_nested_response() {
    let json: serde_json::Value = serde_json::json!({
        "web": {
            "results": [
                {
                    "title": "Example",
                    "url": "https://example.com",
                    "description": "An example site"
                }
            ]
        }
    });
    let results = extract_results(&json, &brave_result_map()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Example");
    assert_eq!(results[0].snippet, "An example site");
    assert!(results[0].date.is_none());
    assert!(results[0].source.is_none());
}

#[test]
fn extract_results_missing_path_returns_error() {
    let json: serde_json::Value = serde_json::json!({"data": []});
    let err = extract_results(&json, &searxng_result_map()).unwrap_err();
    assert!(err.message.contains("results"));
}

#[test]
fn extract_results_skips_items_without_required_fields() {
    let json: serde_json::Value = serde_json::json!({
        "results": [
            {"title": "Good", "url": "https://ok.com", "content": "ok"},
            {"title": "No URL"},
            {"content": "No title"}
        ]
    });
    let results = extract_results(&json, &searxng_result_map()).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Good");
}

#[test]
fn truncate_response_within_limit() {
    assert_eq!(truncate_response("hello", Some(10)), "hello");
    assert_eq!(truncate_response("hello", None), "hello");
}

#[test]
fn truncate_response_over_limit() {
    assert_eq!(truncate_response("hello world", Some(5)), "hello");
}

#[test]
fn truncate_response_respects_utf8_boundary() {
    let text = "你好世界";
    let truncated = truncate_response(text, Some(5));
    assert_eq!(truncated, "你");
}

#[test]
fn resolve_dot_path_nested() {
    let json: serde_json::Value = serde_json::json!({"a": {"b": {"c": 42}}});
    assert_eq!(
        resolve_dot_path(&json, "a.b.c"),
        Some(&serde_json::json!(42))
    );
}

#[test]
fn resolve_dot_path_missing() {
    let json: serde_json::Value = serde_json::json!({"a": 1});
    assert!(resolve_dot_path(&json, "a.b").is_none());
}

#[test]
fn serialized_result_omits_none_fields() {
    let result = WebSearchResult {
        title: "T".into(),
        url: "U".into(),
        snippet: "S".into(),
        date: None,
        source: None,
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(!json.contains("date"));
    assert!(!json.contains("source"));
}

#[test]
fn html_to_markdown_converts_basic_html() {
    let html = "<h1>Title</h1><p>Hello <strong>world</strong>.</p><ul><li>item</li></ul>";
    let md = html_to_markdown(html);
    assert!(md.contains("Title"), "should contain heading text");
    assert!(md.contains("**world**"), "should preserve bold");
    assert!(md.contains("item"), "should contain list item");
}

#[test]
fn html_to_markdown_extracts_links() {
    let html = r#"<a href="https://example.com">click</a>"#;
    let md = html_to_markdown(html);
    assert!(md.contains("https://example.com"), "should contain URL");
    assert!(md.contains("click"), "should contain link text");
}

#[test]
fn html_to_markdown_handles_full_page() {
    let html = r#"
    <!DOCTYPE html>
    <html><head><title>Test</title></head>
    <body>
        <nav>navigation</nav>
        <main><article><p>Main content here.</p></article></main>
        <footer>footer</footer>
    </body></html>"#;
    let md = html_to_markdown(html);
    assert!(md.contains("Main content"), "should contain main content");
}

#[test]
fn html_to_markdown_returns_original_on_empty() {
    assert_eq!(html_to_markdown(""), "");
}

#[test]
fn html_to_markdown_passes_through_plain_text() {
    let text = "Just plain text, no HTML.";
    let md = html_to_markdown(text);
    assert!(md.contains("Just plain text"), "should preserve plain text");
}

#[test]
fn is_html_content_type_detects_html() {
    assert!(is_html_content_type("text/html"));
    assert!(is_html_content_type("text/html; charset=utf-8"));
    assert!(is_html_content_type("TEXT/HTML"));
    assert!(is_html_content_type("application/xhtml+xml"));
}

#[test]
fn is_html_content_type_rejects_non_html() {
    assert!(!is_html_content_type("application/json"));
    assert!(!is_html_content_type("text/plain"));
    assert!(!is_html_content_type("image/png"));
    assert!(!is_html_content_type(""));
}
