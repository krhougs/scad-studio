use app_server_protocol::{CadQueryObjectKind, SelectionKind, SelectionRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SelectionTargetDecision {
    pub(super) selection_ref: String,
    pub(super) target_path: Option<String>,
    pub(super) affected_paths: Vec<String>,
    pub(super) edit_goal: &'static str,
}

pub(super) fn selection_target_decision(
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
        edit_goal: edit_goal(selection, moving, replacing),
        target_path,
    })
}

pub(super) fn preferred_selection_ref(selection: &SelectionRef) -> String {
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

pub(super) fn affected_paths_text(decision: &SelectionTargetDecision) -> String {
    if decision.affected_paths.is_empty() {
        return decision
            .target_path
            .clone()
            .unwrap_or_else(|| "未确认 CadQuery 目标文件".into());
    }
    decision.affected_paths.join(", ")
}

pub(super) fn export_target_path(target_path: &str) -> String {
    let stem = target_path
        .rsplit('/')
        .next()
        .unwrap_or("agent_model.py")
        .split('.')
        .next()
        .unwrap_or("agent_model");
    format!("outputs/{}.step", super::non_empty(stem, "agent_model"))
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
