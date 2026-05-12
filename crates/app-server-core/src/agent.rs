pub mod plan_package;
pub mod tools;

use std::{
    collections::VecDeque,
    fmt::Display,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    time::Duration,
};

use app_server_protocol::{
    AgentMode, ChatMessageRecord, ChatRole, PathHandle, SelectionKind, SelectionRef,
};
use futures_util::{Stream, StreamExt};
use rig::{
    agent::MultiTurnStreamItem,
    completion::{Message, ToolDefinition},
    message::{ReasoningContent, ToolResultContent},
    prelude::*,
    streaming::{StreamedAssistantContent, StreamedUserContent, StreamingChat},
    tool::{ToolDyn, ToolError},
    wasm_compat::WasmBoxedFuture,
};
use serde_json::json;
use tokio::time::{self, MissedTickBehavior, Sleep};

use crate::llm::{AgentProviderKind, RigAgentConfig, load_rig_agent_config_with_discovery};

use self::tools::{
    AgentToolCall, AgentToolObserver, AgentToolRunContext, ToolExecutor,
    agent_tool_definitions_for_mode, execute_registered_tool,
};

const MAX_HISTORY_TURNS: usize = 8;
const MAX_RIG_TOOL_TURNS: usize = 10;

#[derive(Debug, Clone)]
pub struct AgentTurnInput {
    pub mode: AgentMode,
    pub prompt: String,
    pub history: Vec<ChatMessageRecord>,
    pub selections: Vec<SelectionRef>,
    pub active_selection_index: Option<u32>,
    pub plan_ref: Option<PathHandle>,
    pub context_refs: Vec<String>,
    pub native_web_search_enabled: bool,
    pub execution_scope: Option<tools::AgentExecutionScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RigAgentTurnResult {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RigAgentError {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedToolRequest {
    pub tool_type: String,
    pub provider_tool_type: String,
    pub provider_tool_name: Option<String>,
}

pub struct RigAgentCallbacks {
    pub on_token: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    pub on_reasoning: Arc<dyn Fn(&str) -> bool + Send + Sync>,
    pub on_hosted_tool_requested: Arc<dyn Fn(&HostedToolRequest) -> bool + Send + Sync>,
    pub cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
}

pub fn cadquery_agent_system_prompt() -> &'static str {
    include_str!("../../../docs/cadquery-mvp/agent-system-prompt.md")
}

pub async fn run_rig_agent_turn(
    input: AgentTurnInput,
    tool_executor: Arc<dyn ToolExecutor>,
    tool_context: AgentToolRunContext,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError> {
    let config = load_rig_agent_config_with_discovery()
        .await
        .map_err(|error| RigAgentError {
            message: error.message,
        })?
        .ok_or_else(|| RigAgentError {
            message:
                "Rig Agent is not configured. Set BUDN_AGENT_CONFIG or a provider API key env."
                    .into(),
        })?;
    run_rig_agent_turn_with_config(
        input,
        config,
        tool_executor,
        tool_context,
        tool_observer,
        callbacks,
    )
    .await
}

pub async fn run_rig_agent_turn_with_config(
    input: AgentTurnInput,
    config: RigAgentConfig,
    tool_executor: Arc<dyn ToolExecutor>,
    tool_context: AgentToolRunContext,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError> {
    match config.provider_kind {
        AgentProviderKind::OpenAiResponses => {
            run_openai_responses_rig_agent_turn_with_config(
                input,
                config,
                tool_executor,
                tool_context,
                tool_observer,
                callbacks,
            )
            .await
        }
        AgentProviderKind::OpenAiCompletions => {
            run_openai_completions_rig_agent_turn_with_config(
                input,
                config,
                tool_executor,
                tool_context,
                tool_observer,
                callbacks,
            )
            .await
        }
        AgentProviderKind::Anthropic => {
            run_anthropic_rig_agent_turn_with_config(
                input,
                config,
                tool_executor,
                tool_context,
                tool_observer,
                callbacks,
            )
            .await
        }
    }
}

async fn run_openai_responses_rig_agent_turn_with_config(
    input: AgentTurnInput,
    config: RigAgentConfig,
    tool_executor: Arc<dyn ToolExecutor>,
    tool_context: AgentToolRunContext,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError> {
    let mut builder = rig::providers::openai::Client::builder().api_key(&config.api_key);
    if let Some(base_url) = config.base_url.as_deref() {
        builder = builder.base_url(base_url);
    }
    let client = builder.build().map_err(|error| RigAgentError {
        message: format!("Cannot create Rig OpenAI Responses client: {error}"),
    })?;
    run_streaming_rig_agent_turn(
        input,
        config,
        client,
        tool_executor,
        tool_context,
        tool_observer,
        callbacks,
    )
    .await
}

async fn run_openai_completions_rig_agent_turn_with_config(
    input: AgentTurnInput,
    config: RigAgentConfig,
    tool_executor: Arc<dyn ToolExecutor>,
    tool_context: AgentToolRunContext,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError> {
    let mut builder = rig::providers::openai::CompletionsClient::builder().api_key(&config.api_key);
    if let Some(base_url) = config.base_url.as_deref() {
        builder = builder.base_url(base_url);
    }
    let client = builder.build().map_err(|error| RigAgentError {
        message: format!("Cannot create Rig OpenAI Chat Completions client: {error}"),
    })?;
    run_streaming_rig_agent_turn(
        input,
        config,
        client,
        tool_executor,
        tool_context,
        tool_observer,
        callbacks,
    )
    .await
}

async fn run_streaming_rig_agent_turn<C>(
    input: AgentTurnInput,
    config: RigAgentConfig,
    client: C,
    tool_executor: Arc<dyn ToolExecutor>,
    tool_context: AgentToolRunContext,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError>
where
    C: rig::client::completion::CompletionClient + Clone + Send + Sync + 'static,
{
    let pending_calls = Arc::new(Mutex::new(VecDeque::new()));
    let tools = rig_tools_for_context(
        tool_context.clone(),
        Arc::clone(&tool_executor),
        Arc::clone(&pending_calls),
        Arc::clone(&callbacks.cancelled),
    );
    let mut builder = client
        .agent(config.model.clone())
        .preamble(cadquery_agent_system_prompt())
        .max_tokens(config.max_tokens)
        .default_max_turns(MAX_RIG_TOOL_TURNS);
    if let Some(temperature) = rig_agent_temperature_param(&config) {
        builder = builder.temperature(temperature);
    }
    if let Some(additional_params) = rig_agent_additional_params(&config) {
        builder = builder.additional_params(additional_params);
    }
    notify_hosted_tool_requests(&config, &callbacks);
    let (prompt, history) = build_rig_prompt_and_history(&input);
    let agent = builder.tools(tools).build();
    let stream_future = async {
        agent
            .stream_chat(prompt, history)
            .multi_turn(MAX_RIG_TOOL_TURNS)
            .await
    };
    run_rig_stream_future(
        stream_future,
        config.timeout_secs,
        pending_calls,
        tool_observer,
        callbacks,
    )
    .await
}

async fn run_anthropic_rig_agent_turn_with_config(
    input: AgentTurnInput,
    config: RigAgentConfig,
    tool_executor: Arc<dyn ToolExecutor>,
    tool_context: AgentToolRunContext,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError> {
    let client = anthropic_client(&config)?;
    let pending_calls = Arc::new(Mutex::new(VecDeque::new()));
    let tools = rig_tools_for_context(
        tool_context.clone(),
        Arc::clone(&tool_executor),
        Arc::clone(&pending_calls),
        Arc::clone(&callbacks.cancelled),
    );
    let mut builder = client
        .agent(config.model.clone())
        .preamble(cadquery_agent_system_prompt())
        .max_tokens(config.max_tokens)
        .default_max_turns(MAX_RIG_TOOL_TURNS);
    if let Some(temperature) = rig_agent_temperature_param(&config) {
        builder = builder.temperature(temperature);
    }
    if let Some(additional_params) = rig_agent_additional_params(&config) {
        builder = builder.additional_params(additional_params);
    }
    notify_hosted_tool_requests(&config, &callbacks);
    let (prompt, history) = build_rig_prompt_and_history(&input);
    let agent = builder.tools(tools).build();
    let stream_future = async {
        agent
            .stream_chat(prompt, history)
            .multi_turn(MAX_RIG_TOOL_TURNS)
            .await
    };
    run_rig_stream_future(
        stream_future,
        config.timeout_secs,
        pending_calls,
        tool_observer,
        callbacks,
    )
    .await
}

fn anthropic_client(
    config: &RigAgentConfig,
) -> Result<rig::providers::anthropic::Client, RigAgentError> {
    let version = config.anthropic_version.as_deref().unwrap_or("2023-06-01");
    let mut builder = rig::providers::anthropic::Client::builder()
        .api_key(config.api_key.clone())
        .anthropic_version(version);
    if let Some(base_url) = config.base_url.as_deref() {
        builder = builder.base_url(base_url);
    }
    builder.build().map_err(|error| RigAgentError {
        message: format!("Cannot create Rig Anthropic Messages client: {error}"),
    })
}

async fn run_rig_stream_future<R, E, S, F>(
    stream_future: F,
    timeout_secs: u64,
    pending_calls: Arc<Mutex<VecDeque<AgentToolCall>>>,
    tool_observer: &dyn AgentToolObserver,
    callbacks: RigAgentCallbacks,
) -> Result<RigAgentTurnResult, RigAgentError>
where
    F: Future<Output = S>,
    S: Stream<Item = Result<MultiTurnStreamItem<R>, E>> + Unpin,
    E: Display,
{
    tokio::pin!(stream_future);
    let mut timeout = Box::pin(time::sleep(Duration::from_secs(timeout_secs)));
    let mut cancellation_tick = cancellation_interval();
    let stream = loop {
        tokio::select! {
            biased;
            _ = cancellation_tick.tick() => {
                if (callbacks.cancelled)() {
                    return Ok(RigAgentTurnResult { text: String::new() });
                }
            }
            _ = timeout.as_mut() => {
                return Err(rig_timeout_error());
            }
            stream = &mut stream_future => break stream,
        }
    };
    let mut state = RigStreamState::default();

    drain_rig_stream(
        stream,
        &mut state,
        &pending_calls,
        tool_observer,
        &callbacks,
        &mut timeout,
    )
    .await?;

    Ok(RigAgentTurnResult {
        text: state.final_text(),
    })
}

pub fn rig_agent_additional_params(config: &RigAgentConfig) -> Option<serde_json::Value> {
    match config.provider_kind {
        AgentProviderKind::OpenAiResponses => openai_responses_agent_additional_params(config),
        AgentProviderKind::OpenAiCompletions => None,
        AgentProviderKind::Anthropic => anthropic_agent_additional_params(config),
    }
}

pub fn rig_agent_temperature_param(config: &RigAgentConfig) -> Option<f64> {
    match config.provider_kind {
        AgentProviderKind::OpenAiResponses | AgentProviderKind::OpenAiCompletions => {
            Some(config.temperature)
        }
        AgentProviderKind::Anthropic => {
            let thinking_enabled = config
                .reasoning_effort
                .as_deref()
                .and_then(|effort| anthropic_thinking_budget_tokens(effort, config.max_tokens))
                .is_some();
            (!thinking_enabled).then_some(config.temperature)
        }
    }
}

pub fn hosted_tool_requests_for_config(config: &RigAgentConfig) -> Vec<HostedToolRequest> {
    if !config.native_web_search {
        return Vec::new();
    }
    match config.provider_kind {
        AgentProviderKind::OpenAiResponses => vec![HostedToolRequest {
            tool_type: "web_search".into(),
            provider_tool_type: "web_search".into(),
            provider_tool_name: None,
        }],
        AgentProviderKind::Anthropic => vec![HostedToolRequest {
            tool_type: "web_search".into(),
            provider_tool_type: "web_search_20250305".into(),
            provider_tool_name: Some("web_search".into()),
        }],
        AgentProviderKind::OpenAiCompletions => Vec::new(),
    }
}

fn openai_responses_agent_additional_params(config: &RigAgentConfig) -> Option<serde_json::Value> {
    let mut params = serde_json::Map::new();
    if let Some(effort) = config.reasoning_effort.as_deref() {
        params.insert("reasoning".into(), json!({ "effort": effort }));
    }
    if let Some(service_label) = config.service_label.as_deref() {
        params.insert("service_tier".into(), json!(service_label));
    }
    let tools = hosted_tool_requests_json(config);
    if !tools.is_empty() {
        params.insert("tools".into(), serde_json::Value::Array(tools));
    }
    (!params.is_empty()).then(|| serde_json::Value::Object(params))
}

fn anthropic_agent_additional_params(config: &RigAgentConfig) -> Option<serde_json::Value> {
    let mut params = serde_json::Map::new();
    if let Some(effort) = config.reasoning_effort.as_deref()
        && let Some(budget_tokens) = anthropic_thinking_budget_tokens(effort, config.max_tokens)
    {
        params.insert(
            "thinking".into(),
            json!({
                "type": "enabled",
                "budget_tokens": budget_tokens,
            }),
        );
    }
    let tools = hosted_tool_requests_json(config);
    if !tools.is_empty() {
        params.insert("tools".into(), serde_json::Value::Array(tools));
    }
    (!params.is_empty()).then(|| serde_json::Value::Object(params))
}

fn hosted_tool_requests_json(config: &RigAgentConfig) -> Vec<serde_json::Value> {
    hosted_tool_requests_for_config(config)
        .into_iter()
        .map(|request| {
            let mut tool = serde_json::Map::new();
            tool.insert("type".into(), json!(request.provider_tool_type));
            if let Some(name) = request.provider_tool_name {
                tool.insert("name".into(), json!(name));
            }
            serde_json::Value::Object(tool)
        })
        .collect()
}

fn notify_hosted_tool_requests(config: &RigAgentConfig, callbacks: &RigAgentCallbacks) {
    for request in hosted_tool_requests_for_config(config) {
        if (callbacks.cancelled)() || !(callbacks.on_hosted_tool_requested)(&request) {
            return;
        }
    }
}

fn anthropic_thinking_budget_tokens(effort: &str, max_tokens: u64) -> Option<u64> {
    let requested = match effort.trim().to_ascii_lowercase().as_str() {
        "minimal" | "low" => 1024,
        "medium" => 4096,
        "high" => 8192,
        "xhigh" => 16384,
        _ => return None,
    };
    if max_tokens <= 1024 {
        return None;
    }
    Some(requested.min(max_tokens - 1))
}

async fn drain_rig_stream<R, E, S>(
    mut stream: S,
    state: &mut RigStreamState,
    pending_calls: &Arc<Mutex<VecDeque<AgentToolCall>>>,
    tool_observer: &dyn AgentToolObserver,
    callbacks: &RigAgentCallbacks,
    timeout: &mut Pin<Box<Sleep>>,
) -> Result<(), RigAgentError>
where
    S: Stream<Item = Result<MultiTurnStreamItem<R>, E>> + Unpin,
    E: Display,
{
    let mut cancellation_tick = cancellation_interval();
    loop {
        if (callbacks.cancelled)() {
            return Ok(());
        }
        tokio::select! {
            biased;
            _ = cancellation_tick.tick() => {
                if (callbacks.cancelled)() {
                    return Ok(());
                }
            }
            _ = timeout.as_mut() => {
                return Err(rig_timeout_error());
            }
            item = stream.next() => {
                let Some(item) = item else {
                    return Ok(());
                };
                let item = match item {
                    Ok(item) => item,
                    Err(error) => {
                        log::warn!("[agent] stream item error (skipping): {error}");
                        continue;
                    }
                };
                handle_rig_stream_item(item, state, pending_calls, tool_observer, callbacks);
            }
        }
    }
}

fn cancellation_interval() -> time::Interval {
    let mut interval = time::interval(Duration::from_millis(50));
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
}

fn rig_timeout_error() -> RigAgentError {
    RigAgentError {
        message: "Rig Agent request timed out".into(),
    }
}

#[derive(Default)]
struct RigStreamState {
    text: String,
    final_text: Option<String>,
    calls: Vec<AgentToolCall>,
}

impl RigStreamState {
    fn final_text(self) -> String {
        self.final_text.unwrap_or(self.text)
    }
}

fn handle_rig_stream_item<R>(
    item: MultiTurnStreamItem<R>,
    state: &mut RigStreamState,
    pending_calls: &Arc<Mutex<VecDeque<AgentToolCall>>>,
    tool_observer: &dyn AgentToolObserver,
    callbacks: &RigAgentCallbacks,
) {
    match item {
        MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(text)) => {
            state.text.push_str(&text.text);
            (callbacks.on_token)(&text.text);
        }
        MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Reasoning(
            reasoning,
        )) => {
            for content in reasoning.content {
                if let Some(text) = reasoning_text(content) {
                    (callbacks.on_reasoning)(&text);
                }
            }
        }
        MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ReasoningDelta {
            reasoning,
            ..
        }) => {
            (callbacks.on_reasoning)(&reasoning);
        }
        MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::ToolCall {
            tool_call,
            ..
        }) => {
            let call = AgentToolCall {
                id: tool_call.id,
                function_name: tool_call.function.name,
                arguments: tool_call.function.arguments.to_string(),
            };
            tool_observer.tool_start(&call);
            state.calls.push(call.clone());
            if let Ok(mut pending) = pending_calls.lock() {
                pending.push_back(call);
            }
        }
        MultiTurnStreamItem::StreamUserItem(StreamedUserContent::ToolResult {
            tool_result,
            ..
        }) => {
            if let Some(call) = state.calls.iter().find(|call| call.id == tool_result.id) {
                tool_observer.tool_result(call, &tool_result_text(tool_result.content));
            }
        }
        MultiTurnStreamItem::FinalResponse(response) => {
            state.final_text = Some(response.response().to_owned());
        }
        _ => {}
    }
}

fn reasoning_text(content: ReasoningContent) -> Option<String> {
    match content {
        ReasoningContent::Text { text, .. } | ReasoningContent::Summary(text) => Some(text),
        _ => None,
    }
}

fn tool_result_text(content: rig::OneOrMany<ToolResultContent>) -> String {
    content
        .iter()
        .map(|item| match item {
            ToolResultContent::Text(text) => text.text.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn rig_tools_for_context(
    context: AgentToolRunContext,
    executor: Arc<dyn ToolExecutor>,
    pending_calls: Arc<Mutex<VecDeque<AgentToolCall>>>,
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
) -> Vec<Box<dyn ToolDyn>> {
    agent_tool_definitions_for_mode(context.mode)
        .into_iter()
        .map(|definition| {
            Box::new(RigRegistryTool {
                definition,
                context: context.clone(),
                executor: Arc::clone(&executor),
                pending_calls: Arc::clone(&pending_calls),
                cancelled: Arc::clone(&cancelled),
            }) as Box<dyn ToolDyn>
        })
        .collect()
}

struct RigRegistryTool {
    definition: tools::AgentToolDefinition,
    context: AgentToolRunContext,
    executor: Arc<dyn ToolExecutor>,
    pending_calls: Arc<Mutex<VecDeque<AgentToolCall>>>,
    cancelled: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl ToolDyn for RigRegistryTool {
    fn name(&self) -> String {
        self.definition.name.clone()
    }

    fn definition<'a>(&'a self, _prompt: String) -> WasmBoxedFuture<'a, ToolDefinition> {
        Box::pin(async move {
            ToolDefinition {
                name: self.definition.name.clone(),
                description: self.definition.description.clone(),
                parameters: self.definition.parameters.clone(),
            }
        })
    }

    fn call<'a>(&'a self, args: String) -> WasmBoxedFuture<'a, Result<String, ToolError>> {
        Box::pin(async move {
            let call = self.take_pending_call(args);
            if (self.cancelled)() {
                return Ok(cancelled_tool_result(&call));
            }
            Ok(execute_registered_tool(&call, &self.context, self.executor.as_ref()).await)
        })
    }
}

impl RigRegistryTool {
    fn take_pending_call(&self, args: String) -> AgentToolCall {
        if let Ok(mut pending) = self.pending_calls.lock()
            && let Some(index) = pending
                .iter()
                .position(|call| call.function_name == self.definition.name)
        {
            let mut call = pending.remove(index).expect("pending call index is valid");
            call.arguments = args;
            return call;
        }
        AgentToolCall {
            id: format!("rig_{}", self.definition.name),
            function_name: self.definition.name.clone(),
            arguments: args,
        }
    }
}

fn cancelled_tool_result(call: &AgentToolCall) -> String {
    json!({
        "status": "error",
        "tool_call_id": call.id,
        "tool": call.function_name,
        "message": "Agent run cancelled before tool execution.",
        "error_type": "cancelled",
        "retry_allowed": false
    })
    .to_string()
}

pub fn build_rig_prompt_and_history(input: &AgentTurnInput) -> (String, Vec<Message>) {
    let context = build_turn_context(input);
    let prompt = if context.is_empty() {
        input.prompt.clone()
    } else {
        format!("{context}\n\n{}", input.prompt)
    };
    (prompt, build_rig_history(&input.history))
}

fn build_rig_history(history: &[ChatMessageRecord]) -> Vec<Message> {
    let mut messages = Vec::new();
    if let Some(summary) = extract_latest_chat_summary(history) {
        messages.push(Message::user(format!("[Chat summary]\n{summary}")));
        messages.push(Message::assistant("Understood."));
    }
    let effective = collect_effective_history(history);
    let start = effective.len().saturating_sub(MAX_HISTORY_TURNS * 2);
    for (role, content) in &effective[start..] {
        if *role == "assistant" {
            messages.push(Message::assistant(content.clone()));
        } else {
            messages.push(Message::user(content.clone()));
        }
    }
    messages
}

pub fn build_turn_context(input: &AgentTurnInput) -> String {
    let mut parts = Vec::new();
    parts.push(format!("Mode: {}", mode_label(input.mode)));
    if let Some(plan_ref) = &input.plan_ref {
        parts.push(format!("Plan ref: {}", plan_ref.display_path()));
    }
    if !input.context_refs.is_empty() {
        parts.push(format!(
            "User-attached context refs: {}",
            input.context_refs.join(", ")
        ));
    }
    parts.push(provider_native_capabilities_context(
        input.native_web_search_enabled,
    ));
    parts.push(current_turn_app_tools_context(input.mode));
    if let Some(scope) = &input.execution_scope {
        parts.push(format!(
            "Execution scope:\n{}",
            execution_scope_context(scope)
        ));
    }
    if !input.selections.is_empty() {
        parts.push(format!(
            "Current Web preview selection:\n{}",
            selection_context(&input.selections)
        ));
    }
    parts.join("\n")
}

fn provider_native_capabilities_context(native_web_search_enabled: bool) -> String {
    if native_web_search_enabled {
        "Provider-native capabilities:\n\
         - Hosted native web search: enabled. The provider handles search execution natively; \
         use it when you need current information, and do not look for a web search entry in the app tools list."
            .to_owned()
    } else {
        "Provider-native capabilities:\n\
         - Hosted native web search: disabled. Do not attempt web searches this turn."
            .to_owned()
    }
}

fn current_turn_app_tools_context(mode: AgentMode) -> String {
    let mut lines = vec![
        "Current turn app tools (registered by host):".to_owned(),
        "- Before answering what tools are available, inspect this current-turn app tool list and the tool schemas registered by the host. Do not invent tools from memory.".to_owned(),
    ];
    let tools = agent_tool_definitions_for_mode(mode);
    if tools.is_empty() {
        lines.push("- none".to_owned());
    } else {
        lines.extend(
            tools
                .into_iter()
                .map(|tool| format!("- `{}`: {}", tool.name, tool.description)),
        );
    }
    lines.join("\n")
}

fn extract_latest_chat_summary(history: &[ChatMessageRecord]) -> Option<String> {
    history
        .iter()
        .rev()
        .filter(|msg| msg.role == ChatRole::Meta)
        .find_map(|msg| {
            let value: serde_json::Value = serde_json::from_str(&msg.content).ok()?;
            if value.get("type")?.as_str()? != "chat_summary" {
                return None;
            }
            value.get("summary")?.as_str().map(|s| s.to_owned())
        })
}

fn collect_effective_history(history: &[ChatMessageRecord]) -> Vec<(&'static str, String)> {
    let mut result: Vec<(&'static str, String)> = Vec::new();
    for msg in history {
        let role = match msg.role {
            ChatRole::User => "user",
            ChatRole::Assistant => "assistant",
            ChatRole::Tool | ChatRole::Meta => continue,
        };
        if role == "assistant" && is_tool_call_placeholder(msg) {
            continue;
        }
        let content = msg.content.trim().to_owned();
        if content.is_empty() {
            continue;
        }
        if let Some((last_role, last_content)) = result.last_mut()
            && *last_role == role
        {
            last_content.push_str("\n\n");
            last_content.push_str(&content);
            continue;
        }
        result.push((role, content));
    }
    result
}

fn is_tool_call_placeholder(msg: &ChatMessageRecord) -> bool {
    !msg.tool_calls.is_empty() || msg.content.trim() == "agent tool started"
}

fn mode_label(mode: AgentMode) -> &'static str {
    match mode {
        AgentMode::Agent => "Agent",
        AgentMode::Plan => "Plan",
    }
}

fn object_kind_label(kind: app_server_protocol::CadQueryObjectKind) -> &'static str {
    match kind {
        app_server_protocol::CadQueryObjectKind::Part => "part",
        app_server_protocol::CadQueryObjectKind::Component => "component",
        app_server_protocol::CadQueryObjectKind::Assembly => "assembly",
    }
}

fn selection_kind_label(kind: SelectionKind) -> &'static str {
    match kind {
        SelectionKind::Component => "component",
        SelectionKind::Part => "part",
        SelectionKind::Assembly => "assembly",
        SelectionKind::Instance => "instance",
        SelectionKind::Feature => "feature",
        SelectionKind::Face => "face",
        SelectionKind::Edge => "edge",
        SelectionKind::Vertex => "vertex",
    }
}

fn selection_context(selections: &[SelectionRef]) -> String {
    if selections.is_empty() {
        return "- none".into();
    }
    selections
        .iter()
        .enumerate()
        .map(|(index, selection)| {
            let owner = selection.owner_ref_text.as_deref().unwrap_or("none");
            let owner_kind = selection
                .owner_object_kind
                .map(object_kind_label)
                .unwrap_or("none");
            let instance = selection.instance_path.as_deref().unwrap_or("none");
            let feature = selection
                .candidate_feature_ref
                .as_deref()
                .unwrap_or("none");
            let build = selection.build_id.as_deref().unwrap_or("none");
            let result = selection.result_id.as_deref().unwrap_or("none");
            format!(
                "- index={index}; kind={}; ref_text={}; owner_ref_text={owner}; owner_object_kind={owner_kind}; instance_path={instance}; candidate_feature_ref={feature}; build_id={build}; result_id={result}; ambiguous={}",
                selection_kind_label(selection.kind),
                selection.ref_text,
                selection.ambiguous
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn execution_scope_context(scope: &tools::AgentExecutionScope) -> String {
    let target_path = scope.target_path.as_deref().unwrap_or("none");
    let target_type = scope.target_type.map(object_kind_label).unwrap_or("none");
    let plan_ref = scope.plan_ref.as_deref().unwrap_or("none");
    let plan_result_path = scope.plan_result_path.as_deref().unwrap_or("none");
    format!(
        "- plan_ref={plan_ref}\n- target_path={target_path}\n- target_type={target_type}\n- affected_files={}\n- new_files={}\n- export_targets={}\n- plan_result_path={plan_result_path}",
        scope.affected_files.join(", "),
        scope.new_files.join(", "),
        scope.export_targets.join(", ")
    )
}

pub fn extract_cadquery_code(response: &str) -> String {
    if let Some(start) = response.find("```python") {
        let code_start = start + "```python".len();
        if let Some(end) = response[code_start..].find("```") {
            return response[code_start..code_start + end].trim().to_owned();
        }
    }
    if let Some(start) = response.find("```") {
        let code_start = start + 3;
        let after_lang = if response[code_start..].starts_with('\n') {
            code_start + 1
        } else {
            code_start
        };
        if let Some(end) = response[after_lang..].find("```") {
            return response[after_lang..after_lang + end].trim().to_owned();
        }
    }
    response.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig::{
        OneOrMany,
        message::{Text, ToolCall, ToolFunction, ToolResult},
    };
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    #[derive(Default)]
    struct RecordingObserver {
        starts: Mutex<Vec<AgentToolCall>>,
        results: Mutex<Vec<(String, String)>>,
    }

    impl AgentToolObserver for RecordingObserver {
        fn tool_start(&self, call: &AgentToolCall) {
            self.starts.lock().unwrap().push(call.clone());
        }

        fn tool_result(&self, call: &AgentToolCall, result: &str) {
            self.results
                .lock()
                .unwrap()
                .push((call.id.clone(), result.to_owned()));
        }
    }

    #[test]
    fn rig_stream_mapping_forwards_text_reasoning_and_tool_events() {
        let observer = RecordingObserver::default();
        let pending = Arc::new(Mutex::new(VecDeque::new()));
        let tokens = Arc::new(Mutex::new(Vec::new()));
        let reasoning = Arc::new(Mutex::new(Vec::new()));
        let token_sink = {
            let tokens = Arc::clone(&tokens);
            Arc::new(move |text: &str| {
                tokens.lock().unwrap().push(text.to_owned());
                true
            })
        };
        let reasoning_sink = {
            let reasoning = Arc::clone(&reasoning);
            Arc::new(move |text: &str| {
                reasoning.lock().unwrap().push(text.to_owned());
                true
            })
        };
        let callbacks = RigAgentCallbacks {
            on_token: token_sink,
            on_reasoning: reasoning_sink,
            on_hosted_tool_requested: Arc::new(|_| true),
            cancelled: Arc::new(|| false),
        };
        let mut state = RigStreamState::default();

        handle_rig_stream_item(
            MultiTurnStreamItem::<()>::StreamAssistantItem(StreamedAssistantContent::Text(Text {
                text: "hello".into(),
            })),
            &mut state,
            &pending,
            &observer,
            &callbacks,
        );
        handle_rig_stream_item(
            MultiTurnStreamItem::<()>::StreamAssistantItem(
                StreamedAssistantContent::ReasoningDelta {
                    id: Some("rs_1".into()),
                    reasoning: "thinking".into(),
                },
            ),
            &mut state,
            &pending,
            &observer,
            &callbacks,
        );
        handle_rig_stream_item(
            MultiTurnStreamItem::<()>::StreamAssistantItem(StreamedAssistantContent::ToolCall {
                tool_call: ToolCall::new(
                    "call_1".into(),
                    ToolFunction::new("read_file".into(), json!({"path":"a.py"})),
                ),
                internal_call_id: "internal_1".into(),
            }),
            &mut state,
            &pending,
            &observer,
            &callbacks,
        );
        handle_rig_stream_item(
            MultiTurnStreamItem::<()>::StreamUserItem(StreamedUserContent::ToolResult {
                tool_result: ToolResult {
                    id: "call_1".into(),
                    call_id: None,
                    content: OneOrMany::one(ToolResultContent::Text(Text {
                        text: "{\"status\":\"ok\"}".into(),
                    })),
                },
                internal_call_id: "internal_1".into(),
            }),
            &mut state,
            &pending,
            &observer,
            &callbacks,
        );

        assert_eq!(tokens.lock().unwrap().as_slice(), ["hello"]);
        assert_eq!(reasoning.lock().unwrap().as_slice(), ["thinking"]);
        assert_eq!(state.text, "hello");
        assert_eq!(
            observer.starts.lock().unwrap()[0].function_name,
            "read_file"
        );
        assert_eq!(
            observer.results.lock().unwrap().as_slice(),
            [("call_1".into(), "{\"status\":\"ok\"}".into())]
        );
        assert_eq!(pending.lock().unwrap()[0].id, "call_1");
    }

    #[test]
    fn hosted_tool_request_callback_uses_config_helper_output() {
        let received = Arc::new(Mutex::new(Vec::<HostedToolRequest>::new()));
        let callback_received = Arc::clone(&received);
        let callbacks = RigAgentCallbacks {
            on_token: Arc::new(|_| true),
            on_reasoning: Arc::new(|_| true),
            on_hosted_tool_requested: Arc::new(move |request| {
                callback_received.lock().unwrap().push(request.clone());
                true
            }),
            cancelled: Arc::new(|| false),
        };
        let config = test_rig_config(AgentProviderKind::Anthropic, true);

        notify_hosted_tool_requests(&config, &callbacks);

        let received = received.lock().unwrap();
        assert_eq!(
            received.as_slice(),
            hosted_tool_requests_for_config(&config)
        );
    }

    #[test]
    fn hosted_tool_request_callback_omits_openai_completions() {
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let callback_count = Arc::clone(&count);
        let callbacks = RigAgentCallbacks {
            on_token: Arc::new(|_| true),
            on_reasoning: Arc::new(|_| true),
            on_hosted_tool_requested: Arc::new(move |_| {
                callback_count.fetch_add(1, Ordering::SeqCst);
                true
            }),
            cancelled: Arc::new(|| false),
        };
        let config = test_rig_config(AgentProviderKind::OpenAiCompletions, true);

        notify_hosted_tool_requests(&config, &callbacks);

        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn cancelled_tool_result_uses_current_tool_call_identity() {
        let call = AgentToolCall {
            id: "call_cancel".into(),
            function_name: "write_file".into(),
            arguments: "{}".into(),
        };
        let value: serde_json::Value = serde_json::from_str(&cancelled_tool_result(&call)).unwrap();
        assert_eq!(value["status"], "error");
        assert_eq!(value["tool_call_id"], "call_cancel");
        assert_eq!(value["tool"], "write_file");
        assert_eq!(value["error_type"], "cancelled");
    }

    #[tokio::test]
    async fn drain_rig_stream_returns_when_cancelled_while_stream_is_pending() {
        let observer = RecordingObserver::default();
        let pending_calls = Arc::new(Mutex::new(VecDeque::new()));
        let cancelled = Arc::new(AtomicBool::new(true));
        let callbacks = callbacks_for_cancelled(Arc::clone(&cancelled));
        let mut state = RigStreamState::default();
        let stream =
            futures_util::stream::pending::<Result<MultiTurnStreamItem<()>, &'static str>>();
        let mut timeout = Box::pin(time::sleep(Duration::from_secs(60)));

        drain_rig_stream(
            stream,
            &mut state,
            &pending_calls,
            &observer,
            &callbacks,
            &mut timeout,
        )
        .await
        .expect("cancelled stream should stop without waiting for provider output");

        assert_eq!(state.text, "");
    }

    #[tokio::test]
    async fn drain_rig_stream_applies_configured_timeout() {
        let observer = RecordingObserver::default();
        let pending_calls = Arc::new(Mutex::new(VecDeque::new()));
        let callbacks = callbacks_for_cancelled(Arc::new(AtomicBool::new(false)));
        let mut state = RigStreamState::default();
        let stream =
            futures_util::stream::pending::<Result<MultiTurnStreamItem<()>, &'static str>>();
        let mut timeout = Box::pin(time::sleep(Duration::from_millis(1)));

        let error = drain_rig_stream(
            stream,
            &mut state,
            &pending_calls,
            &observer,
            &callbacks,
            &mut timeout,
        )
        .await
        .expect_err("pending provider stream should time out");

        assert!(error.message.contains("timed out"));
    }

    #[tokio::test]
    async fn drain_rig_stream_maps_provider_error() {
        let observer = RecordingObserver::default();
        let pending_calls = Arc::new(Mutex::new(VecDeque::new()));
        let callbacks = callbacks_for_cancelled(Arc::new(AtomicBool::new(false)));
        let mut state = RigStreamState::default();
        let stream =
            futures_util::stream::iter(vec![Err::<MultiTurnStreamItem<()>, _>("provider down")]);
        let mut timeout = Box::pin(time::sleep(Duration::from_secs(60)));

        let error = drain_rig_stream(
            stream,
            &mut state,
            &pending_calls,
            &observer,
            &callbacks,
            &mut timeout,
        )
        .await
        .expect_err("provider error should be reported");

        assert_eq!(error.message, "provider down");
    }

    fn callbacks_for_cancelled(cancelled: Arc<AtomicBool>) -> RigAgentCallbacks {
        RigAgentCallbacks {
            on_token: Arc::new(|_| true),
            on_reasoning: Arc::new(|_| true),
            on_hosted_tool_requested: Arc::new(|_| true),
            cancelled: Arc::new(move || cancelled.load(Ordering::SeqCst)),
        }
    }

    fn test_rig_config(
        provider_kind: AgentProviderKind,
        native_web_search: bool,
    ) -> RigAgentConfig {
        RigAgentConfig {
            provider_id: "test-provider".into(),
            provider_kind,
            api_key: "sk-test".into(),
            model: "test-model".into(),
            timeout_secs: 1,
            max_tokens: 1024,
            temperature: 0.0,
            reasoning_effort: None,
            service_label: None,
            native_web_search,
            anthropic_version: None,
            base_url: None,
        }
    }
}
