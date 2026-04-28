mod selection;

use app_server_protocol::{
    AgentOperationLevel, CadQueryObjectKind, ChatMessageRecord, ChatRole, SelectionKind,
    SelectionRef,
};

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
    pub operation: AgentOperationLevel,
    pub prompt: String,
    pub history: Vec<ChatMessageRecord>,
    pub selections: Vec<SelectionRef>,
    pub active_selection_index: Option<u32>,
    pub confirmed_target_path: Option<String>,
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
        context: cadquery_execute_context(input),
    }
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
            message: "CadQuery Execute 需要 LLM 后端输出结构化 CadQuery 代码；本地 fallback 不生成几何代码"
                .into(),
        })
    }
}

fn cadquery_execute_context(input: AgentCadQueryCodeInput) -> String {
    let history = history_context(&input.history);
    let selections = selection_context(&input.selections);
    format!(
        "Operation: Execute\nUser request: {}\nHistory: {history}\nTarget path: {}\nTarget type: {}\nActive selection index: {}\nSelections:\n{selections}",
        non_empty(input.prompt.trim(), "未提供具体问题"),
        input.target_display_path,
        object_kind_label(input.target_type),
        input
            .active_selection_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "none".into())
    )
}

fn draft_local_turn(input: AgentTurnInput) -> AgentTurnDraft {
    if input.operation == AgentOperationLevel::Plan {
        return draft_cad_plan(input);
    }
    let operation = operation_label(input.operation);
    let prompt = non_empty(input.prompt.trim(), "未提供具体问题");
    let history = latest_history(&input.history);
    let selections = selection_summary(&input.selections);
    let target = input
        .confirmed_target_path
        .as_deref()
        .unwrap_or("未确认 CadQuery 目标文件");
    AgentTurnDraft {
        text: format!(
            "{operation} turn\nPrompt: {prompt}\nContext: {history}\nSelection: {selections}\nTarget: {target}"
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
        .unwrap_or("未确认 CadQuery 目标文件");
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
            "## CAD Plan\n- Request: {prompt}\n- Context: {history}\n- Selection: {selection}\n- Target: {target_path}\n- Edit: {edit_goal}\n- Confirmation: affected_files=[{affected_files}], export_targets=[{}]",
            export_target_path(target_path)
        ),
    }
}

fn operation_label(operation: AgentOperationLevel) -> &'static str {
    match operation {
        AgentOperationLevel::Inform => "Inform",
        AgentOperationLevel::Plan => "Plan",
        AgentOperationLevel::Execute => "Execute",
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
