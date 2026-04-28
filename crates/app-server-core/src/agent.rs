mod codegen;
mod selection;

use app_server_protocol::{
    AgentOperationLevel, CadQueryObjectKind, ChatMessageRecord, SelectionRef,
};

use self::{
    codegen::{CadQueryDimensions, cadquery_code, target_identifier, target_kind_label},
    selection::{
        affected_paths_text, export_target_path, preferred_selection_ref, selection_target_decision,
    },
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

impl AgentBackend for LocalAgentBackend {
    fn draft_turn(&self, input: AgentTurnInput) -> Result<AgentTurnDraft, AgentBackendError> {
        Ok(draft_local_turn(input))
    }

    fn generate_cadquery_code(
        &self,
        input: AgentCadQueryCodeInput,
    ) -> Result<GeneratedCadQueryCode, AgentBackendError> {
        let target_name = target_identifier(&input.target_display_path);
        let dimensions = CadQueryDimensions::default();
        let selection_count = input.selections.len();
        let selection_target = selection_target_decision(
            &input.selections,
            input.active_selection_index,
            &input.prompt,
        );
        let history_count = input
            .history
            .iter()
            .filter(|message| !message.content.trim().is_empty())
            .count();
        let selection_ref = selection_target
            .as_ref()
            .map(|target| target.selection_ref.as_str())
            .unwrap_or("无当前选择");
        let edit_goal = selection_target
            .as_ref()
            .map(|target| target.edit_goal)
            .unwrap_or("part geometry");
        Ok(GeneratedCadQueryCode {
            code: cadquery_code(
                &target_name,
                dimensions,
                input.target_type,
                selection_target.as_ref(),
            ),
            response_text: format!(
                "Execute turn\nTarget: {}\nSelection target: {}\nEdit: {}\nGenerated CadQuery {} `{}` from prompt, {} prior messages and {} selections.",
                input.target_display_path,
                selection_ref,
                edit_goal,
                target_kind_label(input.target_type),
                target_name,
                history_count,
                selection_count
            ),
        })
    }
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
    let decision =
        selection_target_decision(&input.selections, input.active_selection_index, prompt);
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

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}
