use super::{
    AgentToolConfirmationScope, AgentToolPathPolicy, CadQueryModelFilePolicy, OutputPathPolicy,
};

pub(super) struct ToolPolicyError {
    pub(super) message: String,
    pub(super) error_type: &'static str,
}

impl ToolPolicyError {
    fn invalid_arguments(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "invalid_arguments",
        }
    }

    fn permission_denied(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error_type: "permission_denied",
        }
    }
}

pub(super) fn validate_tool_path_policy(
    tool_name: &str,
    args: &str,
    policy: &AgentToolPathPolicy,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    let parsed: serde_json::Value = serde_json::from_str(args).map_err(|error| {
        ToolPolicyError::invalid_arguments(format!("invalid tool arguments: {error}"))
    })?;
    let path_args = collect_normalized_path_args(tool_name, &parsed, policy)?;
    validate_copy_model_boundary(tool_name, &path_args)?;
    for (field, normalized) in path_args {
        validate_cadquery_model_file_policy(&field, &normalized, policy)?;
        validate_confirmation_file_scope(
            tool_name,
            &field,
            &normalized,
            policy,
            confirmation_scope,
        )?;
    }
    let export_targets = parsed
        .get("export_targets")
        .map(parse_export_targets)
        .transpose()?;
    if export_formats_requested(&parsed)
        && policy.output_paths == OutputPathPolicy::ConfirmationOutputsOnly
        && export_targets.as_ref().is_none_or(Vec::is_empty)
    {
        return Err(ToolPolicyError::permission_denied(
            "export_formats require confirmed export_targets",
        ));
    }
    if let Some(exports) = export_targets {
        for export in exports {
            validate_export_target_scope(&export, policy, confirmation_scope)?;
        }
    }
    Ok(())
}

pub(super) fn validate_registry_tool_intent(
    tool_name: &str,
    args: &str,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    let parsed: serde_json::Value = serde_json::from_str(args).map_err(|error| {
        ToolPolicyError::invalid_arguments(format!("invalid tool arguments: {error}"))
    })?;
    let path_args = collect_normalized_workspace_path_args(&parsed)?;
    validate_write_file_intent(tool_name, &path_args, &parsed, confirmation_scope)
}

pub(super) fn normalize_scope_paths(paths: Vec<String>) -> Vec<String> {
    paths
        .into_iter()
        .filter_map(|path| normalize_workspace_path(&path).ok())
        .collect()
}

fn collect_normalized_workspace_path_args(
    parsed: &serde_json::Value,
) -> Result<Vec<(&'static str, String)>, ToolPolicyError> {
    collect_workspace_path_args(parsed)
        .into_iter()
        .map(|(field, path)| Ok((field, normalize_workspace_path(&path)?)))
        .collect()
}

fn collect_normalized_path_args(
    tool_name: &str,
    parsed: &serde_json::Value,
    policy: &AgentToolPathPolicy,
) -> Result<Vec<(&'static str, String)>, ToolPolicyError> {
    collect_workspace_path_args_for_tool(tool_name, parsed)
        .into_iter()
        .map(|(field, path)| {
            let normalized = validate_one_tool_path(&path, policy)?;
            Ok((field, normalized))
        })
        .collect()
}

fn collect_workspace_path_args_for_tool(
    tool_name: &str,
    parsed: &serde_json::Value,
) -> Vec<(&'static str, String)> {
    collect_workspace_path_args(parsed)
        .into_iter()
        .filter(|(field, _)| !(tool_name == "save_cad_plan" && *field == "target_path"))
        .collect()
}

fn collect_workspace_path_args(parsed: &serde_json::Value) -> Vec<(&'static str, String)> {
    let Some(object) = parsed.as_object() else {
        return Vec::new();
    };
    ["path", "source_path", "target_path"]
        .iter()
        .filter_map(|field| {
            object
                .get(*field)
                .and_then(|value| value.as_str())
                .map(|value| (*field, value.to_owned()))
        })
        .collect()
}

fn validate_one_tool_path(
    path: &str,
    policy: &AgentToolPathPolicy,
) -> Result<String, ToolPolicyError> {
    let cleaned = normalize_workspace_path(path)?;
    let root = first_path_segment(&cleaned);
    if policy.denied_roots.iter().any(|denied| *denied == root) {
        return Err(ToolPolicyError::permission_denied(format!(
            "path root '{root}' is denied for this tool"
        )));
    }
    if !policy.allowed_roots.is_empty()
        && !policy
            .allowed_roots
            .iter()
            .any(|allowed| *allowed == "" || *allowed == root)
    {
        return Err(ToolPolicyError::permission_denied(format!(
            "path root '{root}' is not allowed for this tool"
        )));
    }
    Ok(cleaned)
}

fn validate_copy_model_boundary(
    tool_name: &str,
    paths: &[(&'static str, String)],
) -> Result<(), ToolPolicyError> {
    if tool_name != "copy_file" {
        return Ok(());
    }
    let source = path_arg(paths, "source_path");
    let target = path_arg(paths, "target_path");
    if target.is_some_and(is_cadquery_model_path) && !source.is_some_and(is_cadquery_model_path) {
        return Err(ToolPolicyError::permission_denied(
            "CadQuery model .py copy targets require a CadQuery model .py source",
        ));
    }
    Ok(())
}

fn validate_write_file_intent(
    tool_name: &str,
    paths: &[(&'static str, String)],
    parsed: &serde_json::Value,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    if tool_name != "write_file" {
        return Ok(());
    }
    let (Some(scope), Some(path)) = (confirmation_scope, path_arg(paths, "path")) else {
        return Ok(());
    };
    let has_expected_hash = parsed.get("expected_hash").is_some();
    if scope.contains_new_file(path) && has_expected_hash {
        return Err(ToolPolicyError::permission_denied(
            "write_file paths in confirmed new_files must not provide expected_hash",
        ));
    }
    if scope.contains_affected_file(path) && !has_non_empty_expected_hash(parsed) {
        return Err(ToolPolicyError::permission_denied(
            "write_file paths in confirmed affected_files require expected_hash",
        ));
    }
    Ok(())
}

fn has_non_empty_expected_hash(parsed: &serde_json::Value) -> bool {
    parsed
        .get("expected_hash")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|hash| !hash.is_empty())
}

fn path_arg<'a>(paths: &'a [(&'static str, String)], field: &str) -> Option<&'a str> {
    paths
        .iter()
        .find(|(candidate, _)| *candidate == field)
        .map(|(_, path)| path.as_str())
}

fn validate_cadquery_model_file_policy(
    field: &str,
    path: &str,
    policy: &AgentToolPathPolicy,
) -> Result<(), ToolPolicyError> {
    if !is_cadquery_model_path(path) {
        return Ok(());
    }
    match policy.cadquery_model_file {
        CadQueryModelFilePolicy::ReadOnly | CadQueryModelFilePolicy::CadQueryToolOnly => Ok(()),
        CadQueryModelFilePolicy::Denied => Err(ToolPolicyError::permission_denied(
            "CadQuery model .py files must be modified through CadQuery tools",
        )),
        CadQueryModelFilePolicy::CopyOnly if field == "source_path" || field == "target_path" => {
            Ok(())
        }
        CadQueryModelFilePolicy::CopyOnly => Err(ToolPolicyError::permission_denied(
            "CadQuery model .py files can only be copied by copy_file",
        )),
    }
}

fn validate_confirmation_file_scope(
    tool_name: &str,
    field: &str,
    path: &str,
    policy: &AgentToolPathPolicy,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    if !policy.requires_confirmation_scope || field == "source_path" {
        return Ok(());
    }
    let Some(scope) = confirmation_scope else {
        return Err(ToolPolicyError::permission_denied(
            "tool requires confirmed execution scope",
        ));
    };
    match (tool_name, field) {
        ("patch_file", "path") if !scope.contains_affected_file(path) => {
            return Err(ToolPolicyError::permission_denied(format!(
                "patch_file path '{path}' must be in confirmed affected_files"
            )));
        }
        ("copy_file", "target_path") if !scope.contains_new_file(path) => {
            return Err(ToolPolicyError::permission_denied(format!(
                "copy_file target_path '{path}' must be in confirmed new_files"
            )));
        }
        _ => {}
    }
    if policy.cadquery_model_file == CadQueryModelFilePolicy::CopyOnly
        && field == "target_path"
        && is_cadquery_model_path(path)
        && !scope.contains_new_file(path)
    {
        return Err(ToolPolicyError::permission_denied(
            "copy_file target CadQuery model .py must be in confirmed new_files",
        ));
    }
    if scope.contains_affected_file(path) || scope.contains_new_file(path) {
        Ok(())
    } else {
        Err(ToolPolicyError::permission_denied(format!(
            "path '{path}' is outside confirmed affected_files / new_files"
        )))
    }
}

fn export_formats_requested(parsed: &serde_json::Value) -> bool {
    parsed
        .get("export_formats")
        .and_then(|value| value.as_array())
        .is_some_and(|formats| !formats.is_empty())
}

fn parse_export_targets(value: &serde_json::Value) -> Result<Vec<String>, ToolPolicyError> {
    let Some(targets) = value.as_array() else {
        return Err(ToolPolicyError::invalid_arguments(
            "export_targets must be an array of workspace-relative strings",
        ));
    };
    targets
        .iter()
        .map(|target| {
            let Some(path) = target.as_str() else {
                return Err(ToolPolicyError::invalid_arguments(
                    "export_targets must be an array of workspace-relative strings",
                ));
            };
            normalize_workspace_path(path)
        })
        .collect()
}

fn validate_export_target_scope(
    path: &str,
    policy: &AgentToolPathPolicy,
    confirmation_scope: Option<&AgentToolConfirmationScope>,
) -> Result<(), ToolPolicyError> {
    if policy.output_paths == OutputPathPolicy::Denied {
        return Err(ToolPolicyError::permission_denied(
            "export_targets are not allowed for this tool",
        ));
    }
    if policy.output_paths == OutputPathPolicy::DeclaredOutputsOnly {
        if first_path_segment(path) == "outputs" {
            return Ok(());
        }
        return Err(ToolPolicyError::permission_denied(
            "export target must be under outputs/",
        ));
    }
    if policy.output_paths != OutputPathPolicy::ConfirmationOutputsOnly {
        return Ok(());
    }
    if first_path_segment(path) != "outputs" {
        return Err(ToolPolicyError::permission_denied(
            "export target must be under outputs/",
        ));
    }
    let Some(scope) = confirmation_scope else {
        return Err(ToolPolicyError::permission_denied(
            "export target requires confirmed execution scope",
        ));
    };
    if scope.contains_export_target(path) {
        Ok(())
    } else {
        Err(ToolPolicyError::permission_denied(format!(
            "export target '{path}' is outside confirmed export_targets"
        )))
    }
}

fn is_cadquery_model_path(path: &str) -> bool {
    matches!(
        first_path_segment(path),
        "components" | "parts" | "assemblies"
    ) && path.ends_with(".py")
}

fn normalize_workspace_path(path: &str) -> Result<String, ToolPolicyError> {
    let cleaned = path.replace('\\', "/");
    if cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(ToolPolicyError::permission_denied(
            "path must be workspace-relative",
        ));
    }
    let cleaned = cleaned.trim_matches('/');
    if cleaned.split('/').any(|segment| segment == "..") {
        return Err(ToolPolicyError::permission_denied(
            "path must not contain '..'",
        ));
    }
    Ok(cleaned
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect::<Vec<_>>()
        .join("/"))
}

fn first_path_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}
