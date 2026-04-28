use std::{
    fs,
    path::{Path, PathBuf},
};

use app_server_protocol::WorkspaceId;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::llm::LlmToolCall;

use super::{AgentToolRunContext, semantic_export, tool_error_json};

const DENIED_RELATION_ROOTS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "chats",
    "outputs",
    ".budn_staging",
];
pub(super) const PLAN_SCOPE_ROOTS: &[&str] =
    &["components", "parts", "assemblies", "plans", "refs", "docs"];

pub(super) fn save_cad_plan(
    workspace_root: &Path,
    call: &LlmToolCall,
    context: &AgentToolRunContext,
) -> String {
    let args = match save_plan_args(call) {
        Ok(args) => args,
        Err(result) => return result,
    };
    let plan_path = match unique_plan_path(workspace_root, &args.title, call) {
        Ok(path) => path,
        Err(result) => return result,
    };
    let markdown = render_plan_markdown(&args);
    if let Err(error) = fs::write(&plan_path.absolute, markdown.as_bytes()) {
        return tool_error_json(
            call,
            &format!("写入 CAD Plan 失败: {error}"),
            "file_conflict",
        );
    }
    save_plan_success(call, context, args, plan_path.relative, markdown).to_string()
}

struct SavePlanArgs {
    title: String,
    target_ref: String,
    resolved_target: String,
    affected_files: Vec<String>,
    new_files: Vec<String>,
    export_targets: Vec<String>,
    strategy: String,
    risks: Vec<String>,
    acceptance: Vec<String>,
    execution_boundary: String,
}

struct PlanPath {
    relative: String,
    absolute: PathBuf,
}

fn save_plan_args(call: &LlmToolCall) -> Result<SavePlanArgs, String> {
    let value = parse_object(call)?;
    let resolved_target = cadquery_target_arg(&value, "resolved_target", call)?;
    let affected_files = plan_scope_paths(&value, "affected_files", call)?;
    let args = SavePlanArgs {
        title: non_empty_string_arg(&value, "title", call)?,
        target_ref: non_empty_string_arg(&value, "target_ref", call)?,
        resolved_target,
        affected_files,
        new_files: optional_plan_scope_paths(&value, "new_files", call)?,
        export_targets: export_targets(&value, call)?,
        strategy: non_empty_string_arg(&value, "strategy", call)?,
        risks: optional_string_array(&value, "risks", call)?,
        acceptance: optional_string_array(&value, "acceptance", call)?,
        execution_boundary: non_empty_string_arg(&value, "execution_boundary", call)?,
    };
    validate_plan_confirmation_scope(&args, call)?;
    semantic_export::validate_plan_export_targets(
        &args.resolved_target,
        &args.export_targets,
        call,
    )?;
    Ok(args)
}

pub(super) fn parse_object(call: &LlmToolCall) -> Result<Value, String> {
    serde_json::from_str(&call.arguments).map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid tool arguments: {error}"),
            "invalid_arguments",
        )
    })
}

pub(super) fn non_empty_string_arg(
    value: &Value,
    key: &str,
    call: &LlmToolCall,
) -> Result<String, String> {
    let text = value.get(key).and_then(Value::as_str).unwrap_or("").trim();
    if text.is_empty() {
        Err(tool_error_json(
            call,
            &format!("missing required string argument '{key}'"),
            "invalid_arguments",
        ))
    } else {
        Ok(text.to_owned())
    }
}

fn plan_scope_paths(value: &Value, key: &str, call: &LlmToolCall) -> Result<Vec<String>, String> {
    let paths = optional_plan_scope_paths(value, key, call)?;
    if paths.is_empty() {
        Err(tool_error_json(
            call,
            &format!("'{key}' must contain at least one path"),
            "invalid_arguments",
        ))
    } else {
        Ok(paths)
    }
}

fn optional_plan_scope_paths(
    value: &Value,
    key: &str,
    call: &LlmToolCall,
) -> Result<Vec<String>, String> {
    optional_string_array(value, key, call)?
        .into_iter()
        .map(|path| normalize_allowed_path(&path, PLAN_SCOPE_ROOTS, call))
        .collect()
}

fn export_targets(value: &Value, call: &LlmToolCall) -> Result<Vec<String>, String> {
    let targets = optional_string_array(value, "export_targets", call)?
        .into_iter()
        .map(|path| normalize_export_target(&path, call))
        .collect::<Result<Vec<_>, _>>()?;
    if targets.is_empty() {
        return Err(tool_error_json(
            call,
            "'export_targets' must contain at least one path",
            "invalid_arguments",
        ));
    }
    Ok(targets)
}

fn validate_plan_confirmation_scope(args: &SavePlanArgs, call: &LlmToolCall) -> Result<(), String> {
    if args
        .affected_files
        .iter()
        .any(|path| path == &args.resolved_target)
        || args
            .new_files
            .iter()
            .any(|path| path == &args.resolved_target)
    {
        return Ok(());
    }
    Err(tool_error_json(
        call,
        "resolved_target must be included in affected_files or new_files",
        "invalid_arguments",
    ))
}

fn cadquery_target_arg(value: &Value, key: &str, call: &LlmToolCall) -> Result<String, String> {
    let path = non_empty_string_arg(value, key, call)?;
    let normalized = normalize_allowed_path(&path, &["components", "parts", "assemblies"], call)?;
    if !normalized.ends_with(".py") {
        return Err(tool_error_json(
            call,
            "resolved_target must be a CadQuery .py model source",
            "invalid_arguments",
        ));
    }
    Ok(normalized)
}

pub(super) fn optional_string_array(
    value: &Value,
    key: &str,
    call: &LlmToolCall,
) -> Result<Vec<String>, String> {
    let Some(raw) = value.get(key) else {
        return Ok(Vec::new());
    };
    let Some(array) = raw.as_array() else {
        return Err(tool_error_json(
            call,
            &format!("'{key}' must be an array of strings"),
            "invalid_arguments",
        ));
    };
    array
        .iter()
        .map(|entry| {
            entry.as_str().map(str::to_owned).ok_or_else(|| {
                tool_error_json(
                    call,
                    &format!("'{key}' must be an array of strings"),
                    "invalid_arguments",
                )
            })
        })
        .collect()
}

pub(super) fn path_handle(
    path: &str,
    call: &LlmToolCall,
) -> Result<app_server_protocol::PathHandle, String> {
    app_server_protocol::PathHandle::new(
        WorkspaceId::new("workspace"),
        path.split('/').map(str::to_owned),
    )
    .map_err(|error| {
        tool_error_json(
            call,
            &format!("invalid workspace path: {error}"),
            "invalid_arguments",
        )
    })
}

fn normalize_export_target(path: &str, call: &LlmToolCall) -> Result<String, String> {
    let normalized = normalize_workspace_path(path, call)?;
    if first_segment(&normalized) != "outputs" {
        return Err(tool_error_json(
            call,
            "export_targets must be under outputs/",
            "permission_denied",
        ));
    }
    if supported_export_extension(&normalized) {
        Ok(normalized)
    } else {
        Err(tool_error_json(
            call,
            "export_targets must use .step, .stl, or .3mf",
            "invalid_arguments",
        ))
    }
}

fn supported_export_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".step") || lower.ends_with(".stl") || lower.ends_with(".3mf")
}

pub(super) fn normalize_allowed_path(
    path: &str,
    allowed_roots: &[&str],
    call: &LlmToolCall,
) -> Result<String, String> {
    let normalized = normalize_workspace_path(path, call)?;
    let root = first_segment(&normalized);
    if DENIED_RELATION_ROOTS.contains(&root) {
        return Err(tool_error_json(
            call,
            &format!("path root '{root}' is denied for this tool"),
            "permission_denied",
        ));
    }
    if allowed_roots.iter().any(|allowed| *allowed == root) {
        Ok(normalized)
    } else {
        Err(tool_error_json(
            call,
            &format!("path root '{root}' is not allowed for this tool"),
            "permission_denied",
        ))
    }
}

fn normalize_workspace_path(path: &str, call: &LlmToolCall) -> Result<String, String> {
    let cleaned = path.trim().replace('\\', "/");
    if cleaned.is_empty() || cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(tool_error_json(
            call,
            "path must be workspace-relative",
            "permission_denied",
        ));
    }
    if cleaned.split('/').any(|segment| segment == "..") {
        return Err(tool_error_json(
            call,
            "path must not contain '..'",
            "permission_denied",
        ));
    }
    Ok(cleaned
        .trim_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

fn unique_plan_path(
    workspace_root: &Path,
    title: &str,
    call: &LlmToolCall,
) -> Result<PlanPath, String> {
    let plans_dir = safe_plans_dir(workspace_root, call)?;
    let slug = slugify(title);
    for index in 1..=999 {
        let file_name = if index == 1 {
            format!("{slug}.md")
        } else {
            format!("{slug}-{index}.md")
        };
        let absolute = plans_dir.join(&file_name);
        if !path_occupied(&absolute, call)? {
            return Ok(PlanPath {
                relative: format!("plans/{file_name}"),
                absolute,
            });
        }
    }
    Err(tool_error_json(
        call,
        "unable to allocate unique CAD Plan path",
        "file_conflict",
    ))
}

fn path_occupied(path: &Path, call: &LlmToolCall) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(tool_error_json(
            call,
            &format!("读取 CAD Plan 路径失败: {error}"),
            "file_conflict",
        )),
    }
}

fn safe_plans_dir(workspace_root: &Path, call: &LlmToolCall) -> Result<PathBuf, String> {
    let plans_dir = workspace_root.join("plans");
    match fs::symlink_metadata(&plans_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(tool_error_json(
            call,
            "plans directory must not be a symlink",
            "permission_denied",
        )),
        Ok(metadata) if metadata.is_dir() => Ok(plans_dir),
        Ok(_) => Err(tool_error_json(
            call,
            "plans path must be a directory",
            "invalid_arguments",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&plans_dir).map_err(|error| {
                tool_error_json(
                    call,
                    &format!("创建 plans 目录失败: {error}"),
                    "file_conflict",
                )
            })?;
            Ok(plans_dir)
        }
        Err(error) => Err(tool_error_json(
            call,
            &format!("读取 plans 目录失败: {error}"),
            "file_conflict",
        )),
    }
}

fn render_plan_markdown(args: &SavePlanArgs) -> String {
    [
        format!("# {}", args.title),
        "## Target".into(),
        format!("- Target Ref: {}", args.target_ref),
        format!("- Resolved Target: {}", args.resolved_target),
        list_section("Affected Files", &args.affected_files),
        list_section("New Files", &args.new_files),
        list_section("Export Targets", &args.export_targets),
        "## CadQuery Strategy".into(),
        args.strategy.clone(),
        list_section("Risks", &args.risks),
        list_section("Acceptance", &args.acceptance),
        "## Execution Boundary".into(),
        args.execution_boundary.clone(),
    ]
    .join("\n\n")
}

fn list_section(title: &str, items: &[String]) -> String {
    let body = if items.is_empty() {
        "- none".into()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!("## {title}\n{body}")
}

fn save_plan_success(
    call: &LlmToolCall,
    context: &AgentToolRunContext,
    args: SavePlanArgs,
    plan_ref: String,
    markdown: String,
) -> Value {
    json!({
        "status": "ok",
        "tool": call.function_name,
        "message": "CAD Plan saved",
        "plan_ref": plan_ref.clone(),
        "display_path": plan_ref,
        "hash": sha256_text(&markdown),
        "summary": args.strategy,
        "target_ref": args.target_ref,
        "target_path": args.resolved_target,
        "affected_files": args.affected_files,
        "new_files": args.new_files,
        "export_targets": args.export_targets,
        "execution_boundary": args.execution_boundary,
        "run_id": context.run_id
    })
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    for character in title.chars().flat_map(|value| value.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "cad-plan".into()
    } else {
        slug
    }
}

fn sha256_text(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

fn first_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}
