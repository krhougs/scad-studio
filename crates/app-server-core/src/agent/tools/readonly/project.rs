use std::path::Path;

use serde_json::{Value, json};
use tokio::fs;

use crate::agent::plan_package::collect_plan_packages;
use crate::agent::tools::AgentToolCall;

use super::{canonical_or_original, collect_files, path};

pub(super) async fn get_project_context(workspace_root: &Path, call: &AgentToolCall) -> String {
    let workspace_root = canonical_or_original(workspace_root).await;
    let mut objects = Vec::new();
    for (root, object_type) in [
        ("components", "component"),
        ("parts", "part"),
        ("assemblies", "assembly"),
    ] {
        collect_project_objects(&workspace_root, root, object_type, &mut objects).await;
    }
    objects.sort_by_key(|value| value["source_path"].as_str().unwrap_or("").to_owned());
    let (plans, plan_warnings) = collect_plan_entries(&workspace_root).await;
    let chats = collect_simple_files(&workspace_root, "chats", "jsonl").await;
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "project context collected",
        "objects": objects,
        "plans": plans,
        "chats": chats,
        "warnings": plan_warnings
    })
    .to_string()
}

async fn collect_project_objects(
    workspace_root: &Path,
    root: &str,
    object_type: &str,
    objects: &mut Vec<Value>,
) {
    let mut files = Vec::new();
    collect_files(workspace_root, &workspace_root.join(root), &mut files).await;
    for source_path in files.into_iter().filter(|path| path.ends_with(".py")) {
        let doc_path = source_path.trim_end_matches(".py").to_owned() + ".md";
        let has_doc = path::safe_file_path(workspace_root, &doc_path)
            .await
            .is_some();
        objects.push(json!({
            "object_type": object_type,
            "source_path": source_path,
            "paired_doc_path": has_doc.then_some(doc_path),
            "has_paired_doc": has_doc
        }));
    }
}

async fn collect_simple_files(workspace_root: &Path, root: &str, extension: &str) -> Vec<Value> {
    let mut files = Vec::new();
    collect_files(workspace_root, &workspace_root.join(root), &mut files).await;
    files.sort();
    files
        .into_iter()
        .filter(|path| path.ends_with(&format!(".{extension}")))
        .map(|path| json!({"path": path}))
        .collect()
}

async fn collect_plan_entries(workspace_root: &Path) -> (Vec<Value>, Vec<String>) {
    let (packages, warnings) = collect_plan_packages(workspace_root).await;
    let mut plans = Vec::new();
    for package in packages {
        let updated_ms = modified_ms(workspace_root, &package.result_path).await;
        plans.push(json!({
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
            "updated_ms": updated_ms
        }));
    }
    for path in collect_legacy_plan_files(workspace_root).await {
        let updated_ms = modified_ms(workspace_root, &path).await;
        plans.push(json!({
            "kind": "legacy_plan",
            "path": path,
            "updated_ms": updated_ms
        }));
    }
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

async fn collect_legacy_plan_files(workspace_root: &Path) -> Vec<String> {
    let plans_dir = workspace_root.join("plans");
    let Ok(metadata) = fs::symlink_metadata(&plans_dir).await else {
        return Vec::new();
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Vec::new();
    }
    let Ok(mut entries) = fs::read_dir(&plans_dir).await else {
        return Vec::new();
    };
    let mut plans = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        plans.push(format!("plans/{}", entry.file_name().to_string_lossy()));
    }
    plans.sort();
    plans
}

async fn modified_ms(workspace_root: &Path, relative_path: &str) -> Option<u64> {
    fs::metadata(workspace_root.join(relative_path))
        .await
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
}
