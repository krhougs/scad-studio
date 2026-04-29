use std::{fs, path::Path};

use serde_json::{Value, json};

use crate::agent::plan_package::collect_plan_packages;
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
    let (plans, plan_warnings) = collect_plan_entries(&workspace_root);
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "project context collected",
        "objects": objects,
        "plans": plans,
        "chats": collect_simple_files(&workspace_root, "chats", "jsonl"),
        "warnings": plan_warnings
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

fn collect_plan_entries(workspace_root: &Path) -> (Vec<Value>, Vec<String>) {
    let (packages, warnings) = collect_plan_packages(workspace_root);
    let mut plans = packages
        .into_iter()
        .map(|package| {
            json!({
                "kind": "plan_package",
                "plan_id": package.plan_id,
                "plan_ref": package.plan_ref,
                "request_path": package.request_path,
                "plan_path": package.plan_path,
                "result_path": package.result_path,
                "title": package.title,
                "status": package.status,
                "target_path": package.target_path,
                "target_type": target_type_label(package.target_type),
                "updated_ms": modified_ms(workspace_root, &package.result_path)
            })
        })
        .collect::<Vec<_>>();
    plans.extend(
        collect_legacy_plan_files(workspace_root)
            .into_iter()
            .map(|path| {
                json!({
                    "kind": "legacy_plan",
                    "path": path,
                    "updated_ms": modified_ms(workspace_root, &path)
                })
            }),
    );
    plans.sort_by_key(|value| {
        value["plan_ref"]
            .as_str()
            .or_else(|| value["path"].as_str())
            .unwrap_or("")
            .to_owned()
    });
    (plans, warnings)
}

fn target_type_label(target_type: app_server_protocol::CadQueryObjectKind) -> &'static str {
    match target_type {
        app_server_protocol::CadQueryObjectKind::Assembly => "assembly",
        app_server_protocol::CadQueryObjectKind::Component => "component",
        app_server_protocol::CadQueryObjectKind::Part => "part",
    }
}

fn collect_legacy_plan_files(workspace_root: &Path) -> Vec<String> {
    let plans_dir = workspace_root.join("plans");
    let Ok(metadata) = fs::symlink_metadata(&plans_dir) else {
        return Vec::new();
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(&plans_dir) else {
        return Vec::new();
    };
    let mut plans = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).ok()?;
            if !metadata.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                return None;
            }
            Some(format!("plans/{}", entry.file_name().to_string_lossy()))
        })
        .collect::<Vec<_>>();
    plans.sort();
    plans
}

fn modified_ms(workspace_root: &Path, relative_path: &str) -> Option<u64> {
    fs::metadata(workspace_root.join(relative_path))
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}
