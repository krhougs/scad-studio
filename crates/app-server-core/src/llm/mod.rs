mod config;
mod openai_compat;

pub use config::{LlmConfig, LlmConfigError, build_model_string, load_llm_config};
pub use openai_compat::{
    OpenAiCompatibleProvider, build_request_body, extract_delta_content, read_sse_stream,
    read_sse_stream_with_reasoning,
};

#[derive(Debug, Clone)]
pub struct LlmToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmToolCall {
    pub id: String,
    pub function_name: String,
    pub arguments: String,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
    pub reasoning_content: Option<String>,
    pub tool_calls: Vec<LlmToolCall>,
}

impl LlmResponse {
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
    pub reasoning_content: Option<String>,
    pub tool_calls: Vec<LlmToolCall>,
    pub tool_call_id: Option<String>,
}

impl LlmMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            reasoning_content: None,
            tool_calls: Vec::new(),
            tool_call_id: None,
        }
    }

    pub fn assistant_with_tool_calls(content: String, tool_calls: Vec<LlmToolCall>) -> Self {
        Self::assistant_response(content, None, tool_calls)
    }

    pub fn assistant_with_reasoning_and_tool_calls(
        content: String,
        reasoning_content: String,
        tool_calls: Vec<LlmToolCall>,
    ) -> Self {
        Self::assistant_response(content, Some(reasoning_content), tool_calls)
    }

    pub fn assistant_response(
        content: String,
        reasoning_content: Option<String>,
        tool_calls: Vec<LlmToolCall>,
    ) -> Self {
        Self {
            role: "assistant".into(),
            content,
            reasoning_content,
            tool_calls,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_call_id: String, content: String) -> Self {
        Self {
            role: "tool".into(),
            content,
            reasoning_content: None,
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id),
        }
    }
}

#[derive(Debug)]
pub struct LlmError {
    pub message: String,
}

impl std::fmt::Display for LlmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

pub trait LlmProvider: Send + Sync {
    fn stream_chat(
        &self,
        messages: Vec<LlmMessage>,
        tools: &[LlmToolDefinition],
        on_token: &dyn Fn(&str) -> bool,
    ) -> Result<LlmResponse, LlmError>;

    fn stream_chat_with_reasoning(
        &self,
        messages: Vec<LlmMessage>,
        tools: &[LlmToolDefinition],
        on_token: &dyn Fn(&str) -> bool,
        on_reasoning: &dyn Fn(&str) -> bool,
    ) -> Result<LlmResponse, LlmError> {
        let _ = on_reasoning;
        self.stream_chat(messages, tools, on_token)
    }
}

pub async fn create_provider() -> Result<Box<dyn LlmProvider>, LlmError> {
    let config = load_llm_config()
        .await
        .map_err(|e| LlmError { message: e.message })?
        .ok_or_else(|| LlmError {
            message: "LLM not configured. Set BUDN_LLM_CONFIG or BUDN_LLM_BASE_URL + BUDN_LLM_API_KEY environment variables.".into(),
        })?;
    log::info!(
        "LLM provider configured: {} (model: {})",
        config.base_url,
        config.model
    );
    Ok(Box::new(OpenAiCompatibleProvider::new(config)))
}
