use std::path::Path;

use app_server_protocol::{CadQueryMeshPayload, CadQueryObjectKind, CadQueryPartMesh};
use serde_json::{Value, json};

use crate::llm::LlmToolCall;

use super::super::{CadQueryToolCachedResult, CadQueryToolRunResult};

const REQUIRED_MODEL_DETAILS: [&str; 6] = [
    "purpose",
    "key_dimensions",
    "intended_use",
    "assumptions",
    "interaction_notes",
    "manufacturing_or_placement_constraints",
];

#[derive(Debug, Clone)]
pub(super) struct SourceContract {
    pub(super) target_type_matches: bool,
    pub(super) has_build_function: bool,
    pub(super) has_refs: bool,
    pub(super) has_model_description: bool,
    pub(super) unsafe_calls: Vec<&'static str>,
    pub(super) invalid_imports: Vec<String>,
}

pub(super) fn analyze_success(
    workspace_root: &Path,
    call: &LlmToolCall,
    target_path: &str,
    include_paired_doc: bool,
    include_dependencies: bool,
    source: &str,
    has_model_description: bool,
) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery source analyzed",
        "target_path": target_path,
        "target_type": target_type_label(target_type_from_path(target_path)),
        "has_build_function": has_build_function(source),
        "has_refs": has_refs(source),
        "has_model_description": has_model_description,
        "paired_doc_path": include_paired_doc.then(|| paired_doc_path(workspace_root, target_path)).flatten(),
        "local_dependencies": if include_dependencies { local_dependencies(source) } else { Vec::new() },
        "ref_keys": feature_keys(source),
        "warnings": analyze_warnings(source, has_model_description)
    })
}

pub(super) fn source_contract(
    path: &str,
    expected_type: CadQueryObjectKind,
    source: &str,
) -> SourceContract {
    SourceContract {
        target_type_matches: declared_type(source).unwrap_or_else(|| target_type_from_path(path))
            == expected_type,
        has_build_function: has_build_function(source),
        has_refs: has_refs(source),
        has_model_description: has_model_description(source),
        unsafe_calls: unsafe_calls(source),
        invalid_imports: invalid_project_imports(source),
    }
}

pub(super) fn contract_json(contract: &SourceContract) -> Value {
    json!({
        "target_type_matches": contract.target_type_matches,
        "has_build_function": contract.has_build_function,
        "has_refs": contract.has_refs,
        "has_model_description": contract.has_model_description,
        "unsafe_calls": contract.unsafe_calls,
        "invalid_imports": contract.invalid_imports
    })
}

pub(super) fn contract_warnings(contract: &SourceContract) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if !contract.has_build_function {
        warnings.push("missing build function");
    }
    if !contract.has_refs {
        warnings.push(
            "missing REFS.features; add REFS = {\"type\":\"part\",\"features\":{...}} with stable feature keys chosen from this model's semantics",
        );
    }
    if !contract.has_model_description {
        warnings.push(
            "missing MODEL_DESCRIPTION / MODEL_DETAILS; add purpose, key_dimensions, intended_use, assumptions, interaction_notes, and manufacturing_or_placement_constraints",
        );
    }
    if !contract.target_type_matches {
        warnings.push("target type does not match REFS type");
    }
    if !contract.invalid_imports.is_empty() {
        warnings.push("invalid project-local import");
    }
    if !contract.unsafe_calls.is_empty() {
        warnings.push("unsafe call detected");
    }
    warnings
}

pub(super) fn run_success(
    call: &LlmToolCall,
    result: CadQueryToolRunResult,
    committed: bool,
) -> Value {
    if committed {
        let message = if result.warnings.is_empty() {
            "CadQuery execution completed"
        } else {
            "CadQuery execution completed with warnings"
        };
        json!({
            "status": "ok",
            "tool": call.function_name,
            "message": message,
            "result_id": result.mesh.result_id,
            "build_id": result.mesh.build_id,
            "committed_files": result.committed_files,
            "exports": result.exports,
            "summary": mesh_summary(&result.mesh),
            "warnings": result.warnings
        })
    } else {
        json!({
            "status": "ok",
            "tool": call.function_name,
            "message": "CadQuery dry run completed",
            "result_id": result.mesh.result_id,
            "build_id": result.mesh.build_id,
            "root_object_kind": target_type_label(result.mesh.root_object_kind),
            "summary": mesh_summary(&result.mesh),
            "warnings": result.warnings
        })
    }
}

pub(super) fn result_success(call: &LlmToolCall, result: &CadQueryToolCachedResult) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery result summarized",
        "result_id": result.mesh.result_id,
        "build_id": result.mesh.build_id,
        "root_ref_text": result.mesh.root_ref_text,
        "root_object_kind": target_type_label(result.mesh.root_object_kind),
        "parts": result.mesh.parts.iter().map(part_summary).collect::<Vec<_>>(),
        "exports": result.exports
    })
}

pub(super) fn resolve_selection_success(
    call: &LlmToolCall,
    mesh: &CadQueryMeshPayload,
    ref_text: &str,
) -> Value {
    let resolved = resolve_face_ref(mesh, ref_text).or_else(|| resolve_feature_ref(mesh, ref_text));
    let (owner, feature, stable, ambiguous) = match resolved {
        Some((owner, feature)) => (Some(owner), Some(feature.clone()), Some(feature), false),
        None => (None, None, None, true),
    };
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CadQuery selection resolved",
        "raw_ref_text": ref_text,
        "owner_ref_text": owner,
        "owner_path": Value::Null,
        "owner_doc_path": Value::Null,
        "candidate_feature_ref": feature,
        "stable_ref": stable,
        "ambiguous": ambiguous,
        "risks": if ambiguous { vec!["selection could not be mapped to a stable feature"] } else { Vec::new() }
    })
}

pub(super) fn is_model_path(path: &str) -> bool {
    matches!(
        path.split('/').next().unwrap_or(""),
        "components" | "parts" | "assemblies"
    ) && path.ends_with(".py")
}

pub(super) fn target_type_label(kind: CadQueryObjectKind) -> &'static str {
    match kind {
        CadQueryObjectKind::Part => "part",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Assembly => "assembly",
    }
}

pub(super) fn target_type_from_path(path: &str) -> CadQueryObjectKind {
    match path.split('/').next().unwrap_or("") {
        "components" => CadQueryObjectKind::Component,
        "assemblies" => CadQueryObjectKind::Assembly,
        _ => CadQueryObjectKind::Part,
    }
}

fn paired_doc_path(root: &Path, source: &str) -> Option<String> {
    let doc = source.strip_suffix(".py")?.to_owned() + ".md";
    root.join(&doc).is_file().then_some(doc)
}

fn has_build_function(source: &str) -> bool {
    source.contains("def build(") || source.contains("def build (")
}

fn has_refs(source: &str) -> bool {
    source.contains("REFS") && source.contains("features")
}

fn has_model_description(source: &str) -> bool {
    has_string_assignment(source, "MODEL_DESCRIPTION")
        && assignment_dict(source, "MODEL_DETAILS").is_some_and(|dict| {
            REQUIRED_MODEL_DETAILS
                .iter()
                .all(|field| dict_has_key(dict, field))
        })
}

fn has_string_assignment(source: &str, name: &str) -> bool {
    module_assignment_value(source, name)
        .map(str::trim_start)
        .is_some_and(|value| {
            let Some(quote @ ('\'' | '"')) = value.chars().next() else {
                return false;
            };
            python_string_at(value, 0, quote).is_some_and(|(_, text)| !text.trim().is_empty())
        })
}

fn assignment_dict<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let value = module_assignment_value(source, name)?;
    let start = skip_ws(value, 0);
    dict_body_at(value, start)
}

fn module_assignment_value<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    let mut index = 0;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if ch == '\'' || ch == '"' {
            index = quoted_string_end(source, index, ch)?;
            continue;
        }
        if ch == '#' {
            index = line_comment_end(source, index);
            continue;
        }
        if !is_line_start(source, index) || !source[index..].starts_with(name) {
            index += ch.len_utf8();
            continue;
        }
        if !is_identifier_boundary(source, index, name.len()) {
            index += name.len();
            continue;
        }
        if let Some(value_start) = assignment_value_start(source, index + name.len()) {
            return Some(&source[value_start..]);
        }
        index += name.len();
    }
    None
}

fn assignment_value_start(source: &str, name_end: usize) -> Option<usize> {
    let index = skip_inline_ws(source, name_end);
    if source[index..].starts_with('=') && !source[index + 1..].starts_with('=') {
        return Some(index + 1);
    }
    if !source[index..].starts_with(':') {
        return None;
    }
    let line_end = source[index..]
        .find('\n')
        .map_or(source.len(), |offset| index + offset);
    let assign = source[index + 1..line_end].find('=')? + index + 1;
    (!source[assign + 1..].starts_with('=')).then_some(assign + 1)
}

fn dict_has_key(dict: &str, key: &str) -> bool {
    value_start_for_key(dict, key)
        .is_some_and(|value_start| dict_value_is_non_empty(dict, value_start))
}

fn value_start_for_key(dict: &str, key: &str) -> Option<usize> {
    let mut index = 0;
    let mut depth = 0usize;
    while index < dict.len() {
        let ch = dict[index..].chars().next()?;
        if ch == '\'' || ch == '"' {
            let (end, text) = python_string_at(dict, index, ch)?;
            if depth == 0 && text == key {
                let colon = skip_ws(dict, end);
                if dict[colon..].starts_with(':') {
                    return Some(colon + 1);
                }
            }
            index = end;
            continue;
        }
        if ch == '#' {
            index = line_comment_end(dict, index);
            continue;
        }
        update_depth(ch, &mut depth);
        index += ch.len_utf8();
    }
    None
}

fn dict_value_is_non_empty(dict: &str, value_start: usize) -> bool {
    let value_start = skip_ws(dict, value_start);
    let Some(ch) = dict[value_start..].chars().next() else {
        return false;
    };
    match ch {
        '\'' | '"' => {
            python_string_at(dict, value_start, ch).is_some_and(|(_, text)| !text.trim().is_empty())
        }
        '{' => dict_body_at(dict, value_start).is_some_and(collection_body_has_content),
        '[' => list_body_at(dict, value_start).is_some_and(collection_body_has_content),
        _ => false,
    }
}

fn collection_body_has_content(body: &str) -> bool {
    let mut index = 0;
    while index < body.len() {
        let Some(ch) = body[index..].chars().next() else {
            break;
        };
        match ch {
            '\'' | '"' => {
                let Some((end, text)) = python_string_at(body, index, ch) else {
                    return false;
                };
                if !text.trim().is_empty() {
                    return true;
                }
                index = end;
            }
            '#' => index = line_comment_end(body, index),
            ':' | ',' | '{' | '}' | '[' | ']' | '(' | ')' => index += ch.len_utf8(),
            _ if ch.is_whitespace() => index += ch.len_utf8(),
            _ => return true,
        }
    }
    false
}

fn dict_body_at(source: &str, open_index: usize) -> Option<&str> {
    collection_body_at(source, open_index, '{', '}')
}

fn list_body_at(source: &str, open_index: usize) -> Option<&str> {
    collection_body_at(source, open_index, '[', ']')
}

fn collection_body_at(source: &str, open_index: usize, open: char, close: char) -> Option<&str> {
    if !source[open_index..].starts_with(open) {
        return None;
    }
    let mut index = open_index;
    let mut depth = 0usize;
    let mut content_start = open_index;
    while index < source.len() {
        let ch = source[index..].chars().next()?;
        if ch == '\'' || ch == '"' {
            index = quoted_string_end(source, index, ch)?;
            continue;
        }
        if ch == '#' {
            index = line_comment_end(source, index);
            continue;
        }
        if ch == open {
            if depth == 0 {
                content_start = index + ch.len_utf8();
            }
        }
        update_depth(ch, &mut depth);
        if ch == close {
            if depth == 0 {
                return Some(&source[content_start..index]);
            }
        }
        index += ch.len_utf8();
    }
    None
}

fn python_string_at(source: &str, quote_index: usize, quote: char) -> Option<(usize, &str)> {
    let marker = if quote == '\'' { "'''" } else { "\"\"\"" };
    if source[quote_index..].starts_with(marker) {
        let content_start = quote_index + marker.len();
        let offset = source[content_start..].find(marker)?;
        let content_end = content_start + offset;
        return Some((
            content_end + marker.len(),
            &source[content_start..content_end],
        ));
    }
    quoted_string_at(source, quote_index, quote)
}

fn quoted_string_at(source: &str, quote_index: usize, quote: char) -> Option<(usize, &str)> {
    let content_start = quote_index + quote.len_utf8();
    let mut escaped = false;
    for (offset, ch) in source[content_start..].char_indices() {
        let index = content_start + offset;
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some((index + ch.len_utf8(), &source[content_start..index]));
        }
    }
    None
}

fn quoted_string_end(source: &str, quote_index: usize, quote: char) -> Option<usize> {
    python_string_at(source, quote_index, quote).map(|(end, _)| end)
}

fn update_depth(ch: char, depth: &mut usize) {
    match ch {
        '{' | '[' | '(' => *depth += 1,
        '}' | ']' | ')' => *depth = depth.saturating_sub(1),
        _ => {}
    }
}

fn skip_ws(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn skip_inline_ws(source: &str, mut index: usize) -> usize {
    while index < source.len() {
        let Some(ch) = source[index..].chars().next() else {
            break;
        };
        if !matches!(ch, ' ' | '\t') {
            break;
        }
        index += ch.len_utf8();
    }
    index
}

fn line_comment_end(source: &str, index: usize) -> usize {
    source[index..]
        .find('\n')
        .map_or(source.len(), |offset| index + offset + 1)
}

fn is_line_start(source: &str, index: usize) -> bool {
    index == 0 || source[..index].chars().next_back() == Some('\n')
}

fn is_identifier_boundary(source: &str, start: usize, len: usize) -> bool {
    let before = source[..start].chars().next_back();
    let after = source[start + len..].chars().next();
    !before.is_some_and(is_identifier_char) && !after.is_some_and(is_identifier_char)
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn declared_type(source: &str) -> Option<CadQueryObjectKind> {
    for (needle, kind) in [
        ("\"type\":\"part\"", CadQueryObjectKind::Part),
        ("\"type\": \"part\"", CadQueryObjectKind::Part),
        ("\"type\":\"component\"", CadQueryObjectKind::Component),
        ("\"type\": \"component\"", CadQueryObjectKind::Component),
        ("\"type\":\"assembly\"", CadQueryObjectKind::Assembly),
        ("\"type\": \"assembly\"", CadQueryObjectKind::Assembly),
    ] {
        if source.contains(needle) {
            return Some(kind);
        }
    }
    None
}

fn unsafe_calls(source: &str) -> Vec<&'static str> {
    [
        ("open(", "open"),
        ("open (", "open"),
        ("io.open", "io.open"),
        ("Path(", "Path"),
        ("Path (", "Path"),
        ("write_text(", "write_text"),
        ("write_bytes(", "write_bytes"),
        ("unlink(", "unlink"),
        ("subprocess.", "subprocess"),
        ("os.system", "os.system"),
        ("os.remove", "os.remove"),
        ("os.rename", "os.rename"),
        ("os.replace", "os.replace"),
        ("shutil.rmtree", "shutil.rmtree"),
        ("shutil.move", "shutil.move"),
    ]
    .into_iter()
    .filter_map(|(needle, label)| source.contains(needle).then_some(label))
    .collect()
}

fn invalid_project_imports(source: &str) -> Vec<String> {
    let mut imports = source
        .lines()
        .filter_map(invalid_project_import_line)
        .collect::<Vec<_>>();
    imports.sort();
    imports.dedup();
    imports
}

fn invalid_project_import_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.starts_with("from .") {
        return Some(trimmed.to_owned());
    }
    if let Some(module) = trimmed
        .strip_prefix("from ")
        .and_then(|rest| rest.split_once(" import ").map(|(module, _)| module))
    {
        return invalid_import_root(module).then(|| trimmed.to_owned());
    }
    let modules = trimmed.strip_prefix("import ")?;
    modules
        .split(',')
        .any(|module| invalid_import_root(module))
        .then(|| trimmed.to_owned())
}

fn invalid_import_root(module: &str) -> bool {
    let root = module
        .trim()
        .split_whitespace()
        .next()
        .unwrap_or("")
        .split('.')
        .next()
        .unwrap_or("");
    matches!(
        root,
        "chats" | "plans" | "outputs" | ".budn_staging" | "docs" | "target" | "node_modules"
    )
}

fn analyze_warnings(source: &str, has_model_description: bool) -> Vec<&'static str> {
    let mut warnings = Vec::new();
    if !has_build_function(source) {
        warnings.push("missing build function");
    }
    if !has_refs(source) {
        warnings.push("missing REFS features");
    }
    if !has_model_description {
        warnings.push("missing MODEL_DESCRIPTION / MODEL_DETAILS");
    }
    warnings
}

fn local_dependencies(source: &str) -> Vec<String> {
    let mut deps = source
        .lines()
        .filter_map(local_dependency_line)
        .collect::<Vec<_>>();
    deps.sort();
    deps.dedup();
    deps
}

fn local_dependency_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let module = trimmed
        .strip_prefix("from ")
        .and_then(|rest| rest.split_once(" import ").map(|(module, _)| module))
        .or_else(|| trimmed.strip_prefix("import "))?;
    let root = module.split('.').next()?;
    matches!(root, "components" | "parts" | "assemblies")
        .then(|| format!("{}.py", module.replace('.', "/")))
}

fn feature_keys(source: &str) -> Vec<String> {
    let Some(features_index) = source
        .find("\"features\"")
        .or_else(|| source.find("'features'"))
    else {
        return Vec::new();
    };
    let Some(open_offset) = source[features_index..].find('{') else {
        return Vec::new();
    };
    source[features_index + open_offset + 1..]
        .split(['"', '\''])
        .skip(1)
        .step_by(2)
        .take_while(|key| *key != "selector")
        .filter(|key| !matches!(*key, "type" | "features"))
        .map(str::to_owned)
        .take(20)
        .collect()
}

fn part_summary(part: &CadQueryPartMesh) -> Value {
    json!({
        "ref_text": part.ref_text,
        "object_kind": target_type_label(part.object_kind),
        "instance_path": part.instance_path,
        "features": part.feature_map.iter().map(|item| item.feature.clone()).collect::<Vec<_>>(),
        "face_count": part.faces.len(),
        "edge_count": part.edges.len(),
        "vertex_count": part.vertices.len()
    })
}

fn mesh_summary(mesh: &CadQueryMeshPayload) -> Value {
    json!({
        "part_count": mesh.parts.len(),
        "face_count": mesh.parts.iter().map(|part| part.faces.len()).sum::<usize>(),
        "edge_count": mesh.parts.iter().map(|part| part.edges.len()).sum::<usize>(),
        "vertex_count": mesh.parts.iter().map(|part| part.vertices.len()).sum::<usize>(),
        "features": mesh.parts.iter().flat_map(|part| part.feature_map.iter().map(|feature| feature.feature.clone())).collect::<Vec<_>>()
    })
}

fn resolve_face_ref(mesh: &CadQueryMeshPayload, ref_text: &str) -> Option<(String, String)> {
    let (owner, face_idx) = parse_face_ref(ref_text)?;
    let part = mesh
        .parts
        .iter()
        .find(|part| part_name_matches(part, &owner))?;
    let feature = part
        .feature_map
        .iter()
        .find(|feature| feature.face_indices.contains(&face_idx))?;
    Some((
        part.ref_text.clone(),
        format!("@feature[{owner}.{}]", feature.feature),
    ))
}

fn resolve_feature_ref(mesh: &CadQueryMeshPayload, ref_text: &str) -> Option<(String, String)> {
    let value = ref_text.strip_prefix("@feature[")?.strip_suffix(']')?;
    let (owner, feature_name) = value.split_once('.')?;
    let part = mesh
        .parts
        .iter()
        .find(|part| part_name_matches(part, owner))?;
    part.feature_map
        .iter()
        .any(|feature| feature.feature == feature_name)
        .then(|| (part.ref_text.clone(), ref_text.to_owned()))
}

fn parse_face_ref(ref_text: &str) -> Option<(String, u32)> {
    let value = ref_text.strip_prefix("@face[")?.strip_suffix(']')?;
    let (owner, face) = value.split_once(":f_")?;
    Some((owner.to_owned(), face.parse().ok()?))
}

fn part_name_matches(part: &CadQueryPartMesh, owner: &str) -> bool {
    part.name == owner || part.ref_text == format!("@part[{owner}]")
}
