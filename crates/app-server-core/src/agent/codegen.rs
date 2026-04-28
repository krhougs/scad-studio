use app_server_protocol::CadQueryObjectKind;

use super::selection::SelectionTargetDecision;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CadQueryDimensions {
    width: f64,
    depth: f64,
    height: f64,
}

impl Default for CadQueryDimensions {
    fn default() -> Self {
        Self {
            width: 1.0,
            depth: 1.0,
            height: 1.0,
        }
    }
}

pub(super) fn cadquery_code(
    name: &str,
    dimensions: CadQueryDimensions,
    target_type: CadQueryObjectKind,
    selection: Option<&SelectionTargetDecision>,
) -> String {
    match target_type {
        CadQueryObjectKind::Assembly => cadquery_assembly_code(name, dimensions, selection),
        CadQueryObjectKind::Part | CadQueryObjectKind::Component => {
            cadquery_shape_code(name, dimensions, target_type, selection)
        }
    }
}

pub(super) fn target_kind_label(target_type: CadQueryObjectKind) -> &'static str {
    match target_type {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}

pub(super) fn target_identifier(path: &str) -> String {
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

fn cadquery_shape_code(
    name: &str,
    dimensions: CadQueryDimensions,
    target_type: CadQueryObjectKind,
    selection: Option<&SelectionTargetDecision>,
) -> String {
    let selected_ref = selection
        .map(|target| python_string(&target.selection_ref))
        .unwrap_or_default();
    format!(
        "import cadquery as cq\n\nSELECTION_REF = \"{selected_ref}\"\n\nREFS = {{\n    \"{kind_key}\": \"{name}\",\n    \"features\": {{\n        \"selection_target\": {{\"description\": SELECTION_REF}}\n    }}\n}}\n\ndef build(params=None):\n    params = params or {{}}\n    width = float(params.get(\"width\", {width:.3}))\n    depth = float(params.get(\"depth\", {depth:.3}))\n    height = float(params.get(\"height\", {height:.3}))\n    result = cq.Workplane(\"XY\").box(width, depth, height).tag(\"{name}\")\n    return result\n",
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
        "import cadquery as cq\n\nSELECTION_REF = \"{selected_ref}\"\n\nREFS = {{\n    \"assembly\": \"{name}\",\n    \"features\": {{\n        \"selection_target\": {{\"description\": SELECTION_REF}}\n    }}\n}}\n\ndef build(params=None):\n    params = params or {{}}\n    offset = float(params.get(\"offset\", 0.000))\n    width = float(params.get(\"width\", {width:.3}))\n    depth = float(params.get(\"depth\", {depth:.3}))\n    height = float(params.get(\"height\", {height:.3}))\n    selected = cq.Workplane(\"XY\").box(width, depth, height).tag(\"selected_instance\")\n    assembly = cq.Assembly(name=\"{name}\")\n    assembly.add(selected, name=\"selected_instance\", loc=cq.Location(cq.Vector(offset, 0, 0)))\n    return assembly\n",
        width = dimensions.width,
        depth = dimensions.depth,
        height = dimensions.height,
    )
}

fn target_kind_key(target_type: CadQueryObjectKind) -> &'static str {
    match target_type {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}

fn python_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
