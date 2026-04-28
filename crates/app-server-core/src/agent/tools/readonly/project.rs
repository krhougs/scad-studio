use std::path::Path;

use serde_json::{Value, json};

use crate::llm::LlmToolCall;

use super::{canonical_or_original, collect_files, path};

pub(super) fn get_project_context(workspace_root: &Path, call: &LlmToolCall) -> String {
    let workspace_root = canonical_or_original(workspace_root);
    let mut objects = Vec::new();
    for (root, object_type) in [
        ("components", "component"),
        ("parts", "part"),
        ("assemblies", "assembly"),
    ] {
        collect_project_objects(&workspace_root, root, object_type, &mut objects);
    }
    objects.sort_by_key(|value| value["source_path"].as_str().unwrap_or("").to_owned());
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "project context collected",
        "objects": objects,
        "plans": collect_simple_files(&workspace_root, "plans", "md"),
        "chats": collect_simple_files(&workspace_root, "chats", "jsonl"),
        "warnings": []
    })
    .to_string()
}

fn collect_project_objects(
    workspace_root: &Path,
    root: &str,
    object_type: &str,
    objects: &mut Vec<Value>,
) {
    let mut files = Vec::new();
    collect_files(workspace_root, &workspace_root.join(root), &mut files);
    for source_path in files.into_iter().filter(|path| path.ends_with(".py")) {
        let doc_path = source_path.trim_end_matches(".py").to_owned() + ".md";
        let has_doc = path::safe_file_path(workspace_root, &doc_path).is_some();
        objects.push(json!({
            "object_type": object_type,
            "source_path": source_path,
            "paired_doc_path": has_doc.then_some(doc_path),
            "has_paired_doc": has_doc
        }));
    }
}

fn collect_simple_files(workspace_root: &Path, root: &str, extension: &str) -> Vec<Value> {
    let mut files = Vec::new();
    collect_files(workspace_root, &workspace_root.join(root), &mut files);
    files.sort();
    files
        .into_iter()
        .filter(|path| path.ends_with(&format!(".{extension}")))
        .map(|path| json!({"path": path}))
        .collect()
}
