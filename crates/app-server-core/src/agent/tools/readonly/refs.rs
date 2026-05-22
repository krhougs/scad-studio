use std::path::Path;

use app_server_protocol::SelectionRef;
use serde_json::{Value, json};
use tokio::fs;

use crate::agent::tools::AgentToolCall;

use super::{AgentToolRunContext, parse_object, path, string_arg};
use crate::agent::tools::tool_error_json;

mod parse;

pub(super) async fn resolve_ref(
    workspace_root: &Path,
    call: &AgentToolCall,
    context: &AgentToolRunContext,
) -> String {
    let args = match parse_object(&call.arguments, call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let ref_text = match string_arg(&args, "ref_text", call) {
        Ok(ref_text) => ref_text,
        Err(result) => return result,
    };
    if let Some((kind, name)) = parse_object_ref(&ref_text) {
        if let Err(result) = validate_ref_name(call, &name) {
            return result;
        }
        return resolved_object_ref(call, workspace_root, kind, &name)
            .await
            .to_string();
    }
    if let Some((owner, feature)) = parse_feature_ref(&ref_text) {
        if let Err(result) = validate_feature_ref_parts(call, &owner, &feature) {
            return result;
        }
        return resolved_feature_ref(call, workspace_root, &ref_text, &owner, &feature)
            .await
            .to_string();
    }
    if let Some(selection) = context
        .selections
        .iter()
        .find(|selection| selection.ref_text == ref_text)
    {
        return resolved_selection(call, workspace_root, selection)
            .await
            .to_string();
    }
    resolved_unstable_raw(call, &ref_text).to_string()
}

async fn resolved_selection(
    call: &AgentToolCall,
    workspace_root: &Path,
    selection: &SelectionRef,
) -> Value {
    let owner_path_candidate = selection
        .owner_ref_text
        .as_deref()
        .and_then(owner_path_from_ref);
    let owner_path = match owner_path_candidate {
        Some(path) if path::safe_file_path(workspace_root, &path).await.is_some() => Some(path),
        _ => None,
    };
    let owner_doc_path = match owner_path.as_deref() {
        Some(path) => paired_doc_path(workspace_root, path).await,
        None => None,
    };
    let stable_ref =
        selection_stable_feature(workspace_root, selection, owner_path.as_deref()).await;
    let risks = selection_risks(selection, owner_path.as_deref(), stable_ref.as_ref());
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "ref resolved from current selection",
        "raw_ref_text": selection.ref_text,
        "owner_ref_text": selection.owner_ref_text,
        "owner_path": owner_path,
        "owner_doc_path": owner_doc_path,
        "candidate_feature_ref": selection.candidate_feature_ref,
        "stable_ref": stable_ref,
        "ambiguous": stable_ref.is_none(),
        "risks": risks
    })
}

async fn selection_stable_feature(
    workspace_root: &Path,
    selection: &SelectionRef,
    owner_path: Option<&str>,
) -> Option<String> {
    if selection.ambiguous {
        return None;
    }
    let candidate = selection.candidate_feature_ref.as_deref()?;
    let (feature_owner, feature) = parse_feature_ref(candidate)?;
    if !validate_ref_name_text(&feature_owner) || !validate_ref_name_text(&feature) {
        return None;
    }
    let owner_name = selection
        .owner_ref_text
        .as_deref()
        .and_then(parse_object_ref)
        .map(|(_, name)| name)?;
    if feature_owner != owner_name {
        return None;
    }
    let source = fs::read_to_string(path::safe_file_path(workspace_root, owner_path?).await?)
        .await
        .ok()?;
    parse::source_has_refs_feature(&source, &feature).then(|| candidate.to_owned())
}

fn selection_risks(
    selection: &SelectionRef,
    owner_path: Option<&str>,
    stable_ref: Option<&String>,
) -> Vec<&'static str> {
    if stable_ref.is_some() {
        return Vec::new();
    }
    if selection.candidate_feature_ref.is_some() && owner_path.is_none() {
        return vec!["owner source file was not found or is outside the workspace"];
    }
    if selection.candidate_feature_ref.is_some() {
        return vec!["candidate feature was not found in owner source"];
    }
    vec!["raw geometry has no stable feature mapping"]
}

async fn resolved_object_ref(
    call: &AgentToolCall,
    workspace_root: &Path,
    kind: RefObjectKind,
    name: &str,
) -> Value {
    let source_path = object_source_path(kind, name);
    let exists = path::safe_file_path(workspace_root, &source_path)
        .await
        .is_some();
    let owner_doc_path = if exists {
        paired_doc_path(workspace_root, &source_path).await
    } else {
        None
    };
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "object ref resolved",
        "raw_ref_text": Value::Null,
        "owner_ref_text": format!("@{}[{name}]", kind.ref_label()),
        "owner_path": exists.then_some(source_path),
        "owner_doc_path": owner_doc_path,
        "candidate_feature_ref": Value::Null,
        "stable_ref": exists.then_some(format!("@{}[{name}]", kind.ref_label())),
        "ambiguous": !exists,
        "risks": if exists { vec![] } else { vec!["object source file was not found"] }
    })
}

async fn resolved_feature_ref(
    call: &AgentToolCall,
    workspace_root: &Path,
    ref_text: &str,
    owner: &str,
    feature: &str,
) -> Value {
    let (owner_ref, owner_path) = find_owner_source(workspace_root, owner).await;
    let owner_doc_path = match owner_path.as_deref() {
        Some(path) => paired_doc_path(workspace_root, path).await,
        None => None,
    };
    let has_feature = match owner_path.as_ref() {
        Some(path) => match path::safe_file_path(workspace_root, path).await {
            Some(path) => fs::read_to_string(path)
                .await
                .is_ok_and(|source| parse::source_has_refs_feature(&source, feature)),
            None => false,
        },
        None => false,
    };
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "feature ref resolved",
        "raw_ref_text": Value::Null,
        "owner_ref_text": owner_ref,
        "owner_path": owner_path,
        "owner_doc_path": owner_doc_path,
        "candidate_feature_ref": has_feature.then_some(ref_text.to_owned()),
        "stable_ref": has_feature.then_some(ref_text.to_owned()),
        "ambiguous": !has_feature,
        "risks": if has_feature { vec![] } else { vec!["feature ref was not found in owner source"] }
    })
}

fn resolved_unstable_raw(call: &AgentToolCall, ref_text: &str) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "raw geometry ref has no stable mapping",
        "raw_ref_text": ref_text,
        "owner_ref_text": Value::Null,
        "owner_path": Value::Null,
        "owner_doc_path": Value::Null,
        "candidate_feature_ref": Value::Null,
        "stable_ref": Value::Null,
        "ambiguous": true,
        "risks": [format!("{ref_text} is raw geometry and has no current feature mapping")]
    })
}

#[derive(Debug, Clone, Copy)]
enum RefObjectKind {
    Component,
    Part,
    Assembly,
}

impl RefObjectKind {
    fn root(self) -> &'static str {
        match self {
            Self::Component => "components",
            Self::Part => "parts",
            Self::Assembly => "assemblies",
        }
    }

    fn ref_label(self) -> &'static str {
        match self {
            Self::Component => "component",
            Self::Part => "part",
            Self::Assembly => "assembly",
        }
    }
}

fn parse_object_ref(ref_text: &str) -> Option<(RefObjectKind, String)> {
    for (prefix, kind) in [
        ("@component[", RefObjectKind::Component),
        ("@part[", RefObjectKind::Part),
        ("@assembly[", RefObjectKind::Assembly),
    ] {
        if let Some(name) = ref_text
            .strip_prefix(prefix)
            .and_then(|value| value.strip_suffix(']'))
        {
            return Some((kind, name.to_owned()));
        }
    }
    None
}

fn parse_feature_ref(ref_text: &str) -> Option<(String, String)> {
    let value = ref_text
        .strip_prefix("@feature[")
        .and_then(|value| value.strip_suffix(']'))?;
    let (owner, feature) = value.split_once('.')?;
    Some((owner.to_owned(), feature.to_owned()))
}

async fn find_owner_source(workspace_root: &Path, owner: &str) -> (Option<String>, Option<String>) {
    for kind in [
        RefObjectKind::Part,
        RefObjectKind::Component,
        RefObjectKind::Assembly,
    ] {
        let source_path = object_source_path(kind, owner);
        if path::safe_file_path(workspace_root, &source_path)
            .await
            .is_some()
        {
            return (
                Some(format!("@{}[{owner}]", kind.ref_label())),
                Some(source_path),
            );
        }
    }
    (None, None)
}

fn object_source_path(kind: RefObjectKind, name: &str) -> String {
    format!("{}/{name}.py", kind.root())
}

fn owner_path_from_ref(ref_text: &str) -> Option<String> {
    parse_object_ref(ref_text).and_then(|(kind, name)| {
        validate_ref_name_text(&name).then(|| object_source_path(kind, &name))
    })
}

fn validate_feature_ref_parts(
    call: &AgentToolCall,
    owner: &str,
    feature: &str,
) -> Result<(), String> {
    validate_ref_name(call, owner)?;
    validate_ref_name(call, feature)
}

fn validate_ref_name(call: &AgentToolCall, value: &str) -> Result<(), String> {
    if validate_ref_name_text(value) {
        Ok(())
    } else {
        Err(tool_error_json(
            call,
            "ref name must be a safe identifier",
            "permission_denied",
        ))
    }
}

fn validate_ref_name_text(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

async fn paired_doc_path(workspace_root: &Path, source_path: &str) -> Option<String> {
    let doc_path = source_path.strip_suffix(".py")?.to_owned() + ".md";
    path::safe_file_path(workspace_root, &doc_path)
        .await
        .map(|_| doc_path)
}
