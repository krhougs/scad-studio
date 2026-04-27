use app_server_protocol::{AgentOperationLevel, ChatMessageRecord, SelectionRef};

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
    pub target_display_path: String,
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
        let dimensions = CadQueryDimensions::from_prompt(&input.prompt);
        let selection_count = input.selections.len();
        let history_count = input
            .history
            .iter()
            .filter(|message| !message.content.trim().is_empty())
            .count();
        Ok(GeneratedCadQueryCode {
            code: cadquery_code(&target_name, dimensions),
            response_text: format!(
                "Execute turn\nTarget: {}\nGenerated CadQuery part `{}` from prompt, {} prior messages and {} selections.",
                input.target_display_path, target_name, history_count, selection_count
            ),
        })
    }
}

fn draft_local_turn(input: AgentTurnInput) -> AgentTurnDraft {
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

#[derive(Debug, Clone, Copy, PartialEq)]
struct CadQueryDimensions {
    width: f64,
    depth: f64,
    height: f64,
}

impl CadQueryDimensions {
    fn from_prompt(prompt: &str) -> Self {
        let first = first_positive_number(prompt).unwrap_or(1.0);
        let lower = prompt.to_ascii_lowercase();
        if lower.contains("height") || lower.contains("tall") {
            return Self {
                width: 1.0,
                depth: 1.0,
                height: first,
            };
        }
        Self {
            width: first,
            depth: first,
            height: first,
        }
    }
}

fn first_positive_number(prompt: &str) -> Option<f64> {
    prompt
        .split(|ch: char| !ch.is_ascii_digit() && ch != '.')
        .filter(|value| !value.is_empty())
        .filter_map(|value| value.parse::<f64>().ok())
        .find(|value| value.is_finite() && *value > 0.0)
}

fn cadquery_code(name: &str, dimensions: CadQueryDimensions) -> String {
    format!(
        "import cadquery as cq\n\nREFS = {{\n    \"features\": {{\n        \"body\": {{\"selector\": \"faces(\\\">Z\\\")\"}}\n    }}\n}}\n\ndef build(params=None):\n    params = params or {{}}\n    width = float(params.get(\"width\", {width:.3}))\n    depth = float(params.get(\"depth\", {depth:.3}))\n    height = float(params.get(\"height\", {height:.3}))\n    return cq.Workplane(\"XY\").box(width, depth, height).tag(\"{name}\")\n",
        width = dimensions.width,
        depth = dimensions.depth,
        height = dimensions.height,
    )
}

fn target_identifier(path: &str) -> String {
    let stem = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or("part");
    let mut identifier = stem
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while identifier.contains("__") {
        identifier = identifier.replace("__", "_");
    }
    let trimmed = identifier.trim_matches('_').to_owned();
    if trimmed.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("part_{trimmed}")
    } else if trimmed.is_empty() {
        "part".into()
    } else {
        trimmed
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
        .map(|selection| selection.ref_text.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn non_empty<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.is_empty() { fallback } else { value }
}
