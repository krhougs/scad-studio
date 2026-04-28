use std::io::{BufRead, BufReader};
use std::time::Duration;

use super::{LlmConfig, LlmError, LlmMessage, LlmProvider, LlmResponse, LlmToolCall, LlmToolDefinition};

pub struct OpenAiCompatibleProvider {
    config: LlmConfig,
    agent: ureq::Agent,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: LlmConfig) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(config.timeout_secs)))
            .build()
            .new_agent();
        Self { config, agent }
    }
}

impl LlmProvider for OpenAiCompatibleProvider {
    fn stream_chat(
        &self,
        messages: Vec<LlmMessage>,
        tools: &[LlmToolDefinition],
        on_token: &dyn Fn(&str) -> bool,
    ) -> Result<LlmResponse, LlmError> {
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let body = build_request_body(&self.config, &messages, tools, true);

        let response = self
            .agent
            .post(&url)
            .header("Authorization", &format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .send_json(&body)
            .map_err(|err| LlmError {
                message: format!("LLM HTTP request failed: {err}"),
            })?;

        let reader = BufReader::new(response.into_body().into_reader());
        let result = read_sse_stream(reader, on_token)?;

        if result.content.is_empty() && result.tool_calls.is_empty() {
            return Err(LlmError {
                message: "LLM returned empty response".into(),
            });
        }

        Ok(result)
    }
}

pub fn build_request_body(
    config: &LlmConfig,
    messages: &[LlmMessage],
    tools: &[LlmToolDefinition],
    stream: bool,
) -> serde_json::Value {
    let messages_json: Vec<serde_json::Value> = messages.iter().map(serialize_message).collect();

    let mut body = serde_json::json!({
        "model": config.model,
        "messages": messages_json,
        "temperature": config.temperature,
        "max_tokens": config.max_tokens,
        "stream": stream,
    });

    if !tools.is_empty() {
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect();
        body["tools"] = serde_json::Value::Array(tools_json);
    }

    body
}

fn serialize_message(msg: &LlmMessage) -> serde_json::Value {
    if msg.role == "tool" {
        return serde_json::json!({
            "role": "tool",
            "tool_call_id": msg.tool_call_id,
            "content": msg.content,
        });
    }
    if !msg.tool_calls.is_empty() {
        let tc_json: Vec<serde_json::Value> = msg
            .tool_calls
            .iter()
            .map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.function_name,
                        "arguments": tc.arguments,
                    }
                })
            })
            .collect();
        let content = if msg.content.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(msg.content.clone())
        };
        return serde_json::json!({
            "role": msg.role,
            "content": content,
            "tool_calls": tc_json,
        });
    }
    serde_json::json!({
        "role": msg.role,
        "content": msg.content,
    })
}

pub fn read_sse_stream(
    reader: BufReader<impl std::io::Read>,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<LlmResponse, LlmError> {
    let mut content = String::new();
    let mut tool_acc = ToolCallAccumulator::new();

    for line_result in reader.lines() {
        let line = line_result.map_err(|err| LlmError {
            message: format!("Failed to read SSE stream: {err}"),
        })?;

        let data = match line.strip_prefix("data: ") {
            Some(data) => data.trim(),
            None => continue,
        };

        if data == "[DONE]" {
            break;
        }

        let value: serde_json::Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let Some(delta) = first_choice_delta(&value) else {
            continue;
        };

        if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
            if !text.is_empty() {
                content.push_str(text);
                if !on_token(text) {
                    break;
                }
            }
        }

        if let Some(tc_array) = delta.get("tool_calls").and_then(|tc| tc.as_array()) {
            tool_acc.process_delta(tc_array);
        }
    }

    Ok(LlmResponse {
        content,
        tool_calls: tool_acc.finish(),
    })
}

fn first_choice_delta(value: &serde_json::Value) -> Option<&serde_json::Value> {
    value
        .get("choices")?
        .as_array()?
        .first()?
        .get("delta")
}

pub fn extract_delta_content(json_str: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let delta = first_choice_delta(&value)?;
    let content = delta.get("content")?.as_str()?;
    if content.is_empty() {
        None
    } else {
        Some(content.to_owned())
    }
}

struct ToolCallAccumulator {
    entries: Vec<(String, String, String)>,
}

impl ToolCallAccumulator {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn process_delta(&mut self, tc_array: &[serde_json::Value]) {
        for tc in tc_array {
            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
            while self.entries.len() <= index {
                self.entries.push((String::new(), String::new(), String::new()));
            }
            let entry = &mut self.entries[index];
            if let Some(id) = tc.get("id").and_then(|id| id.as_str()) {
                entry.0 = id.to_owned();
            }
            if let Some(function) = tc.get("function") {
                if let Some(name) = function.get("name").and_then(|n| n.as_str()) {
                    entry.1 = name.to_owned();
                }
                if let Some(args) = function.get("arguments").and_then(|a| a.as_str()) {
                    entry.2.push_str(args);
                }
            }
        }
    }

    fn finish(self) -> Vec<LlmToolCall> {
        self.entries
            .into_iter()
            .filter(|(id, name, _)| !id.is_empty() && !name.is_empty())
            .map(|(id, name, args)| LlmToolCall {
                id,
                function_name: name,
                arguments: args,
            })
            .collect()
    }
}
