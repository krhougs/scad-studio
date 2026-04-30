pub mod plan_package;
mod selection;
pub mod tools;

use app_server_protocol::{
    AgentMode, CadQueryObjectKind, ChatMessageRecord, ChatRole, PathHandle, SelectionKind,
    SelectionRef,
};

use crate::llm::{LlmMessage, LlmProvider};

use self::selection::{
    affected_paths_text, export_target_path, preferred_selection_ref, selection_target_decision,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentBackendDecision {
    pub crate_name: &'static str,
    pub evaluated_version: &'static str,
    pub selected: bool,
    pub rationale: &'static str,
}

#[derive(Debug, Clone)]
pub struct AgentTurnInput {
    pub mode: AgentMode,
    pub prompt: String,
    pub history: Vec<ChatMessageRecord>,
    pub selections: Vec<SelectionRef>,
    pub active_selection_index: Option<u32>,
    pub plan_ref: Option<PathHandle>,
    pub context_refs: Vec<String>,
    pub execution_scope: Option<tools::AgentExecutionScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTurnDraft {
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct AgentCadQueryCodeInput {
    pub prompt: String,
    pub history: Vec<ChatMessageRecord>,
    pub selections: Vec<SelectionRef>,
    pub active_selection_index: Option<u32>,
    pub target_display_path: String,
    pub target_type: CadQueryObjectKind,
    pub execution_scope: Option<tools::AgentExecutionScope>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedCadQueryCode {
    pub code: String,
    pub response_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentLlmRequest {
    pub system_prompt: &'static str,
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentBackendError {
    pub message: String,
}

pub trait AgentBackend {
    fn draft_turn(&self, input: AgentTurnInput) -> Result<AgentTurnDraft, AgentBackendError>;

    fn generate_cadquery_code(
        &self,
        input: AgentCadQueryCodeInput,
    ) -> Result<GeneratedCadQueryCode, AgentBackendError>;
}

#[derive(Debug, Clone, Default)]
pub struct LocalAgentBackend;

pub fn rig_backend_decision() -> AgentBackendDecision {
    AgentBackendDecision {
        crate_name: "rig-core",
        evaluated_version: "0.35.0",
        selected: true,
        rationale: "docs.rs 0.35.0 exposes provider abstraction, tool calling, stream APIs and custom agent control hooks.",
    }
}

pub fn draft_agent_turn(input: AgentTurnInput) -> AgentTurnDraft {
    LocalAgentBackend
        .draft_turn(input)
        .unwrap_or_else(|error| AgentTurnDraft {
            text: format!("Agent backend error: {}", error.message),
        })
}

pub fn generate_cadquery_code(
    input: AgentCadQueryCodeInput,
) -> Result<GeneratedCadQueryCode, AgentBackendError> {
    LocalAgentBackend.generate_cadquery_code(input)
}

pub fn cadquery_agent_system_prompt() -> &'static str {
    include_str!("../../../docs/cadquery-mvp/agent-system-prompt.md")
}

pub fn llm_request_for_cadquery_execute(input: AgentCadQueryCodeInput) -> AgentLlmRequest {
    AgentLlmRequest {
        system_prompt: cadquery_agent_system_prompt(),
        context: cadquery_execute_context_for_llm(&input),
    }
}

pub fn mode_for_tool_loop(requested: AgentMode) -> AgentMode {
    requested
}

impl AgentBackend for LocalAgentBackend {
    fn draft_turn(&self, input: AgentTurnInput) -> Result<AgentTurnDraft, AgentBackendError> {
        Ok(draft_local_turn(input))
    }

    fn generate_cadquery_code(
        &self,
        input: AgentCadQueryCodeInput,
    ) -> Result<GeneratedCadQueryCode, AgentBackendError> {
        let _request = llm_request_for_cadquery_execute(input);
        Err(AgentBackendError {
            message: "Agent mode CadQuery 生成需要 LLM 后端输出结构化 CadQuery 代码；本地 fallback 不生成几何代码"
                .into(),
        })
    }
}

fn draft_local_turn(input: AgentTurnInput) -> AgentTurnDraft {
    if input.mode == AgentMode::Plan {
        return draft_cad_plan(input);
    }
    let mode = mode_label(input.mode);
    let prompt = non_empty(input.prompt.trim(), "未提供具体问题");
    let history = latest_history(&input.history);
    let selections = selection_summary(&input.selections);
    let plan_ref = input
        .plan_ref
        .as_ref()
        .map(PathHandle::display_path)
        .unwrap_or_else(|| "none".into());
    let execution_scope = input
        .execution_scope
        .as_ref()
        .map(execution_scope_context)
        .unwrap_or_else(|| "none".into());
    AgentTurnDraft {
        text: format!(
            "{mode} turn\nPrompt: {prompt}\nContext: {history}\nSelection: {selections}\nPlan ref: {plan_ref}\nExecution scope: {execution_scope}"
        ),
    }
}

fn draft_cad_plan(input: AgentTurnInput) -> AgentTurnDraft {
    let prompt = non_empty(input.prompt.trim(), "未提供具体问题");
    let history = latest_history(&input.history);
    let decision = selection_target_decision(&input.selections, input.active_selection_index);
    let selection = decision
        .as_ref()
        .map(|target| target.selection_ref.as_str())
        .unwrap_or("无当前选择");
    let target_path = decision
        .as_ref()
        .and_then(|target| target.target_path.as_deref())
        .unwrap_or("未指定 CadQuery 目标文件");
    let affected_files = decision
        .as_ref()
        .map(affected_paths_text)
        .unwrap_or_else(|| target_path.to_owned());
    let edit_goal = decision
        .as_ref()
        .map(|target| target.edit_goal)
        .unwrap_or("part geometry");
    AgentTurnDraft {
        text: format!(
            "## CAD Plan\n- Request: {prompt}\n- Context: {history}\n- Selection: {selection}\n- Target: {target_path}\n- Edit: {edit_goal}\n- Execution scope: affected_files=[{affected_files}], export_targets=[{}]",
            export_target_path(target_path)
        ),
    }
}

fn mode_label(mode: AgentMode) -> &'static str {
    match mode {
        AgentMode::Agent => "Agent",
        AgentMode::Plan => "Plan",
    }
}

fn object_kind_label(kind: CadQueryObjectKind) -> &'static str {
    match kind {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
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

fn chat_role_label(role: ChatRole) -> &'static str {
    match role {
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
        ChatRole::Tool => "tool",
        ChatRole::Meta => "meta",
    }
}

fn history_context(history: &[ChatMessageRecord]) -> String {
    let items = history
        .iter()
        .filter_map(|message| {
            let content = message.content.trim();
            if content.is_empty() {
                return None;
            }
            let id = if message.message_id.is_empty() {
                "unknown"
            } else {
                message.message_id.as_str()
            };
            Some(format!(
                "- id={id}; role={}: {content}",
                chat_role_label(message.role)
            ))
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        "- none".into()
    } else {
        items.join("\n")
    }
}

fn latest_history(history: &[ChatMessageRecord]) -> &str {
    history
        .iter()
        .rev()
        .find(|message| !message.content.trim().is_empty())
        .map(|message| message.content.as_str())
        .unwrap_or("无历史消息")
}

fn selection_summary(selections: &[SelectionRef]) -> String {
    if selections.is_empty() {
        return "无当前选择".into();
    }
    selections
        .iter()
        .map(selection_summary_item)
        .collect::<Vec<_>>()
        .join(", ")
}

fn selection_summary_item(selection: &SelectionRef) -> String {
    let preferred = preferred_selection_ref(selection);
    if preferred == selection.ref_text {
        preferred
    } else {
        format!("{preferred} ({})", selection.ref_text)
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

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}

pub fn stream_agent_turn(
    input: AgentTurnInput,
    provider: &dyn LlmProvider,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<AgentTurnDraft, AgentBackendError> {
    let messages = build_turn_messages(&input);
    let response = provider
        .stream_chat(messages, &[], on_token)
        .map_err(|err| AgentBackendError {
            message: err.message,
        })?;
    Ok(AgentTurnDraft {
        text: response.content,
    })
}

pub async fn stream_agent_turn_with_tools(
    input: AgentTurnInput,
    provider: &dyn LlmProvider,
    tool_executor: &dyn tools::ToolExecutor,
    tool_context: tools::AgentToolRunContext,
    tool_observer: &dyn tools::ToolLoopObserver,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<AgentTurnDraft, AgentBackendError> {
    stream_agent_turn_with_tools_and_reasoning(
        input,
        provider,
        tool_executor,
        tool_context,
        tool_observer,
        on_token,
        &|_| true,
    )
    .await
}

pub async fn stream_agent_turn_with_tools_and_reasoning(
    input: AgentTurnInput,
    provider: &dyn LlmProvider,
    tool_executor: &dyn tools::ToolExecutor,
    tool_context: tools::AgentToolRunContext,
    tool_observer: &dyn tools::ToolLoopObserver,
    on_token: &dyn Fn(&str) -> bool,
    on_reasoning: &dyn Fn(&str) -> bool,
) -> Result<AgentTurnDraft, AgentBackendError> {
    let messages = build_turn_messages(&input);
    let response = tools::run_tool_loop_with_registry_and_reasoning(
        messages,
        tool_context,
        provider,
        tool_executor,
        tool_observer,
        on_token,
        on_reasoning,
    )
    .await
    .map_err(|err| AgentBackendError {
        message: err.message,
    })?;
    Ok(AgentTurnDraft {
        text: response.content,
    })
}

pub fn llm_generate_cadquery_code(
    input: AgentCadQueryCodeInput,
    provider: &dyn LlmProvider,
    on_token: &dyn Fn(&str) -> bool,
) -> Result<GeneratedCadQueryCode, AgentBackendError> {
    let messages = build_execute_messages(&input);
    let response = provider
        .stream_chat(messages, &[], on_token)
        .map_err(|err| AgentBackendError {
            message: err.message,
        })?;
    let code = extract_cadquery_code(&response.content);
    Ok(GeneratedCadQueryCode {
        code,
        response_text: response.content,
    })
}

pub fn build_turn_messages(input: &AgentTurnInput) -> Vec<LlmMessage> {
    let system_prompt = cadquery_agent_system_prompt();
    let mut messages = vec![LlmMessage::new("system", system_prompt)];
    append_history_messages(&mut messages, &input.history);
    let context = build_turn_context(input);
    let user_content = if context.is_empty() {
        input.prompt.clone()
    } else {
        format!("{context}\n\n{}", input.prompt)
    };
    messages.push(LlmMessage::new("user", user_content));
    messages
}

pub fn build_execute_messages(input: &AgentCadQueryCodeInput) -> Vec<LlmMessage> {
    let system_prompt = cadquery_agent_system_prompt();
    let mut messages = vec![LlmMessage::new("system", system_prompt)];
    append_history_messages(&mut messages, &input.history);
    let context = cadquery_execute_context_for_llm(input);
    messages.push(LlmMessage::new("user", context));
    messages
}

const MAX_HISTORY_TURNS: usize = 8;

fn append_history_messages(messages: &mut Vec<LlmMessage>, history: &[ChatMessageRecord]) {
    if let Some(summary) = extract_latest_chat_summary(history) {
        messages.push(LlmMessage::new(
            "user",
            &format!("[Chat summary]\n{summary}"),
        ));
        messages.push(LlmMessage::new("assistant", "Understood."));
    }
    let effective = collect_effective_history(history);
    let start = effective.len().saturating_sub(MAX_HISTORY_TURNS * 2);
    for (role, content) in &effective[start..] {
        messages.push(LlmMessage::new(*role, content.as_str()));
    }
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
        if let Some((last_role, last_content)) = result.last_mut() {
            if *last_role == role {
                last_content.push_str("\n\n");
                last_content.push_str(&content);
                continue;
            }
        }
        result.push((role, content));
    }
    result
}

fn is_tool_call_placeholder(msg: &ChatMessageRecord) -> bool {
    !msg.tool_calls.is_empty() || msg.content.trim() == "agent tool started"
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
    if let Some(scope) = &input.execution_scope {
        parts.push(format!(
            "Execution scope:\n{}",
            execution_scope_context(scope)
        ));
    }
    if !input.selections.is_empty() {
        parts.push(format!(
            "Current Viewer selection:\n{}",
            selection_context(&input.selections)
        ));
    }
    parts.join("\n")
}

fn cadquery_execute_context_for_llm(input: &AgentCadQueryCodeInput) -> String {
    let history = history_context(&input.history);
    let selections = selection_context(&input.selections);
    let execution_scope = input
        .execution_scope
        .as_ref()
        .map(execution_scope_context)
        .unwrap_or_else(|| "none".into());
    format!(
        "Mode: Agent\n\
         User request: {}\n\
         History:\n{history}\n\
         Target path: {}\n\
         Target type: {}\n\
         Execution scope:\n{}\n\
         Active selection index: {}\n\
         Selections:\n{selections}\n\n\
         You must respond with a complete CadQuery Python script that implements the user's request. \
         CadQuery model .py changes must be committed through the CadQuery tool and staging, not ordinary file tools. \
         Include the REFS dict and build() function. \
         Wrap the code in a ```python code block.",
        non_empty(input.prompt.trim(), "未提供具体问题"),
        input.target_display_path,
        object_kind_label(input.target_type),
        execution_scope,
        input
            .active_selection_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "none".into())
    )
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
