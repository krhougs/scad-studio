use serde_json::json;

use crate::agent::tools::AgentToolCall;
use crate::agent::web_search;
use crate::llm::ResolvedWebSearchProvider;

use super::tool_error_json;

pub(super) async fn web_search(
    client: &reqwest::Client,
    provider: &ResolvedWebSearchProvider,
    call: &AgentToolCall,
) -> String {
    let args = match parse_web_search_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    match web_search::execute_web_search(client, provider, &args.query, args.top_k).await {
        Ok(results) => {
            let count = results.len();
            let results_json: Vec<serde_json::Value> = results
                .into_iter()
                .map(|r| serde_json::to_value(r).unwrap_or_default())
                .collect();
            json!({
                "status": "ok",
                "tool": call.function_name,
                "results": results_json,
                "result_count": count,
                "message": format!("Found {count} results.")
            })
            .to_string()
        }
        Err(e) => tool_error_json(call, &e.message, "web_search_error"),
    }
}

pub(super) async fn fetch_url(client: &reqwest::Client, call: &AgentToolCall) -> String {
    let url = match parse_fetch_url_args(call) {
        Ok(url) => url,
        Err(result) => return result,
    };
    match web_search::fetch_url(client, &url).await {
        Ok(content) => {
            let len = content.len();
            json!({
                "status": "ok",
                "tool": call.function_name,
                "url": url,
                "content": content,
                "content_length": len,
                "message": format!("Fetched {len} bytes from {url}.")
            })
            .to_string()
        }
        Err(e) => tool_error_json(call, &e.message, "web_search_error"),
    }
}

struct WebSearchArgs {
    query: String,
    top_k: Option<u32>,
}

fn parse_web_search_args(call: &AgentToolCall) -> Result<WebSearchArgs, String> {
    let args: serde_json::Value =
        serde_json::from_str(&call.arguments).map_err(|_| {
            tool_error_json(call, "invalid JSON arguments", "invalid_arguments")
        })?;
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            tool_error_json(call, "missing or empty 'query' parameter", "invalid_arguments")
        })?
        .to_owned();
    let top_k = args.get("top_k").and_then(|v| v.as_u64()).map(|v| v as u32);
    Ok(WebSearchArgs { query, top_k })
}

fn parse_fetch_url_args(call: &AgentToolCall) -> Result<String, String> {
    let args: serde_json::Value =
        serde_json::from_str(&call.arguments).map_err(|_| {
            tool_error_json(call, "invalid JSON arguments", "invalid_arguments")
        })?;
    args.get("url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            tool_error_json(call, "missing or empty 'url' parameter", "invalid_arguments")
        })
}
