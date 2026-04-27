use app_server_protocol::{
    AgentOperationLevel, CadQueryObjectKind, ChatMessageRecord, SelectionKind, SelectionRef,
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
        let dimensions = CadQueryDimensions::from_prompt(&input.prompt);
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
                &input.prompt,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectionTargetDecision {
    selection_ref: String,
    target_path: Option<String>,
    affected_paths: Vec<String>,
    selector_query: Option<String>,
    edit_goal: &'static str,
}

fn selection_target_decision(
    selections: &[SelectionRef],
    active_selection_index: Option<u32>,
    prompt: &str,
) -> Option<SelectionTargetDecision> {
    let selection = active_selection(selections, active_selection_index)?;
    let selection_ref = preferred_selection_ref(selection);
    let moving = movement_intent(prompt);
    let replacing = replacement_intent(prompt);
    let target_path = if moving && selection.instance_path.is_some() {
        assembly_path_from_instance(selection.instance_path.as_deref())
    } else if replacing && selection.instance_path.is_some() {
        assembly_path_from_instance(selection.instance_path.as_deref())
            .or_else(|| owner_or_selection_path(selection))
    } else if moving && selection.kind == SelectionKind::Component {
        None
    } else {
        owner_or_selection_path(selection)
    };
    Some(SelectionTargetDecision {
        selection_ref,
        affected_paths: affected_paths(selection, target_path.as_deref(), replacing),
        selector_query: selection_query(selection),
        edit_goal: edit_goal(selection, moving, replacing),
        target_path,
    })
}

fn active_selection(
    selections: &[SelectionRef],
    active_selection_index: Option<u32>,
) -> Option<&SelectionRef> {
    if let Some(index) = active_selection_index {
        if let Some(selection) = selections.get(index as usize) {
            return Some(selection);
        }
    }
    selections.last()
}

fn preferred_selection_ref(selection: &SelectionRef) -> String {
    if selection.ambiguous {
        return selection.ref_text.clone();
    }
    selection
        .candidate_feature_ref
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(selection.ref_text.as_str())
        .to_owned()
}

fn owner_or_selection_path(selection: &SelectionRef) -> Option<String> {
    if let Some(path) = ref_text_path(
        selection.owner_ref_text.as_deref(),
        selection.owner_object_kind,
    ) {
        return Some(path);
    }
    ref_text_path(
        Some(selection.ref_text.as_str()),
        object_kind_from_selection(selection.kind),
    )
}

fn ref_text_path(ref_text: Option<&str>, kind: Option<CadQueryObjectKind>) -> Option<String> {
    let name = object_ref_name(ref_text?)?;
    match kind? {
        CadQueryObjectKind::Part => Some(format!("parts/{name}.py")),
        CadQueryObjectKind::Component => Some(format!("components/{name}.py")),
        CadQueryObjectKind::Assembly => Some(format!("assemblies/{name}.py")),
    }
}

fn object_ref_name(ref_text: &str) -> Option<String> {
    let (_, rest) = ref_text.split_once('[')?;
    let (name, _) = rest.split_once(']')?;
    let trimmed = name.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn object_kind_from_selection(kind: SelectionKind) -> Option<CadQueryObjectKind> {
    match kind {
        SelectionKind::Part => Some(CadQueryObjectKind::Part),
        SelectionKind::Component => Some(CadQueryObjectKind::Component),
        SelectionKind::Assembly | SelectionKind::Instance => Some(CadQueryObjectKind::Assembly),
        _ => None,
    }
}

fn assembly_path_from_instance(instance_path: Option<&str>) -> Option<String> {
    let assembly = instance_path?.split('/').next()?.trim();
    (!assembly.is_empty()).then(|| format!("assemblies/{assembly}.py"))
}

fn affected_paths(
    selection: &SelectionRef,
    target_path: Option<&str>,
    replacing: bool,
) -> Vec<String> {
    let mut paths = target_path
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if replacing && selection.instance_path.is_some() {
        if let Some(path) = owner_or_selection_path(selection) {
            paths.push(path);
        }
        if let Some(path) = assembly_path_from_instance(selection.instance_path.as_deref()) {
            paths.push(path);
        }
    }
    unique_strings(paths)
}

fn affected_paths_text(decision: &SelectionTargetDecision) -> String {
    if decision.affected_paths.is_empty() {
        return decision
            .target_path
            .clone()
            .unwrap_or_else(|| "未确认 CadQuery 目标文件".into());
    }
    decision.affected_paths.join(", ")
}

fn unique_strings(values: Vec<String>) -> Vec<String> {
    values.into_iter().fold(Vec::new(), |mut acc, value| {
        if !acc.contains(&value) {
            acc.push(value);
        }
        acc
    })
}

fn edit_goal(selection: &SelectionRef, moving: bool, replacing: bool) -> &'static str {
    if selection.ambiguous {
        return "ambiguous selection confirmation required";
    }
    if moving && selection.kind == SelectionKind::Component {
        return "assembly instance required";
    }
    if replacing && selection.instance_path.is_some() {
        return "assembly instance replacement";
    }
    if replacing
        && (selection.owner_object_kind == Some(CadQueryObjectKind::Component)
            || selection.kind == SelectionKind::Component)
    {
        return "component replacement";
    }
    if moving && selection.instance_path.is_some() || selection.kind == SelectionKind::Assembly {
        return "assembly coordination";
    }
    if selection.owner_object_kind == Some(CadQueryObjectKind::Component)
        || selection.kind == SelectionKind::Component
    {
        return "component geometry";
    }
    "part geometry"
}

fn movement_intent(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    contains_ascii_word(
        &lower,
        &["move", "shift", "place", "position", "align", "rotate"],
    ) || ["移动", "对齐", "旋转", "摆放"]
        .iter()
        .any(|word| prompt.contains(word))
}

fn contains_ascii_word(input: &str, words: &[&str]) -> bool {
    input
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|token| !token.is_empty())
        .any(|token| words.contains(&token))
}

fn replacement_intent(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    ["replace", "swap", "change model", "different model"]
        .iter()
        .any(|word| lower.contains(word))
        || ["替换", "换型号", "更换"]
            .iter()
            .any(|word| prompt.contains(word))
}

fn selection_query(selection: &SelectionRef) -> Option<String> {
    if selection.ambiguous {
        return None;
    }
    if let Some(feature) = selection.candidate_feature_ref.as_deref() {
        return feature_selector(feature);
    }
    None
}

fn feature_selector(feature_ref: &str) -> Option<String> {
    let feature = object_ref_name(feature_ref)?
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match feature.as_str() {
        "top" | "top_face" | "top_surface" => Some("faces(\">Z\")".into()),
        "bottom" | "bottom_face" | "bottom_surface" => Some("faces(\"<Z\")".into()),
        "right" | "right_face" | "right_side" => Some("faces(\">X\")".into()),
        "left" | "left_face" | "left_side" => Some("faces(\"<X\")".into()),
        "front" | "front_face" | "front_side" => Some("faces(\">Y\")".into()),
        "back" | "back_face" | "back_side" => Some("faces(\"<Y\")".into()),
        _ => None,
    }
}

fn export_target_path(target_path: &str) -> String {
    let stem = target_path
        .rsplit('/')
        .next()
        .unwrap_or("agent_model.py")
        .split('.')
        .next()
        .unwrap_or("agent_model");
    format!("outputs/{}.step", non_empty(stem, "agent_model"))
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

fn cadquery_code(
    name: &str,
    dimensions: CadQueryDimensions,
    target_type: CadQueryObjectKind,
    selection: Option<&SelectionTargetDecision>,
    prompt: &str,
) -> String {
    match target_type {
        CadQueryObjectKind::Assembly => cadquery_assembly_code(name, dimensions, selection),
        CadQueryObjectKind::Part | CadQueryObjectKind::Component => {
            cadquery_shape_code(name, dimensions, target_type, selection, prompt)
        }
    }
}

fn cadquery_shape_code(
    name: &str,
    dimensions: CadQueryDimensions,
    target_type: CadQueryObjectKind,
    selection: Option<&SelectionTargetDecision>,
    prompt: &str,
) -> String {
    let selected_ref = selection
        .map(|target| python_string(&target.selection_ref))
        .unwrap_or_default();
    let return_expr = shape_return_expr(selection, prompt);
    format!(
        "import cadquery as cq\n\nSELECTION_REF = \"{selected_ref}\"\n\nREFS = {{\n    \"{kind_key}\": \"{name}\",\n    \"features\": {{\n        \"body\": {{\"selector\": \"faces(\\\">Z\\\")\"}},\n        \"selection_target\": {{\"description\": SELECTION_REF}}\n    }}\n}}\n\ndef build(params=None):\n    params = params or {{}}\n    width = float(params.get(\"width\", {width:.3}))\n    depth = float(params.get(\"depth\", {depth:.3}))\n    height = float(params.get(\"height\", {height:.3}))\n    result = cq.Workplane(\"XY\").box(width, depth, height).tag(\"{name}\")\n    {return_expr}\n",
        kind_key = target_kind_key(target_type),
        width = dimensions.width,
        depth = dimensions.depth,
        height = dimensions.height,
    )
}

fn cadquery_assembly_code(
    name: &str,
    dimensions: CadQueryDimensions,
    selection: Option<&SelectionTargetDecision>,
) -> String {
    let selected_ref = selection
        .map(|target| python_string(&target.selection_ref))
        .unwrap_or_default();
    format!(
        "import cadquery as cq\n\nSELECTION_REF = \"{selected_ref}\"\n\nREFS = {{\n    \"assembly\": \"{name}\",\n    \"features\": {{\n        \"selection_target\": {{\"description\": SELECTION_REF}}\n    }}\n}}\n\ndef build(params=None):\n    params = params or {{}}\n    offset = float(params.get(\"offset\", 5.000))\n    width = float(params.get(\"width\", {width:.3}))\n    depth = float(params.get(\"depth\", {depth:.3}))\n    height = float(params.get(\"height\", {height:.3}))\n    selected = cq.Workplane(\"XY\").box(width, depth, height).tag(\"selected_instance\")\n    assembly = cq.Assembly(name=\"{name}\")\n    assembly.add(selected, name=\"selected_instance\", loc=cq.Location(cq.Vector(offset, 0, 0)))\n    return assembly\n",
        width = dimensions.width,
        depth = dimensions.depth,
        height = dimensions.height,
    )
}

fn shape_return_expr(selection: Option<&SelectionTargetDecision>, prompt: &str) -> String {
    if cut_intent(prompt) {
        if let Some(query) = face_query(selection) {
            return format!(
                "return result.{query}.workplane().rect(width * 0.35, depth * 0.20).cutThruAll()"
            );
        }
    }
    if edge_round_intent(prompt) {
        if let Some(query) = edge_query(selection) {
            return format!("return result.{query}.fillet(1.0)");
        }
    }
    "return result".into()
}

fn face_query(selection: Option<&SelectionTargetDecision>) -> Option<&str> {
    let query = selection?.selector_query.as_deref()?;
    query.starts_with("faces(").then_some(query)
}

fn edge_query(selection: Option<&SelectionTargetDecision>) -> Option<&str> {
    let query = selection?.selector_query.as_deref()?;
    query.starts_with("edges(").then_some(query)
}

fn cut_intent(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    ["slot", "cut", "hole", "vent", "open"]
        .iter()
        .any(|word| lower.contains(word))
        || ["开孔", "槽", "切除"]
            .iter()
            .any(|word| prompt.contains(word))
}

fn edge_round_intent(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    ["fillet", "round", "bevel"]
        .iter()
        .any(|word| lower.contains(word))
        || ["倒角", "圆角"].iter().any(|word| prompt.contains(word))
}

fn target_kind_key(target_type: CadQueryObjectKind) -> &'static str {
    match target_type {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}

fn target_kind_label(target_type: CadQueryObjectKind) -> &'static str {
    match target_type {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}

fn python_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
