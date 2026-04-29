use std::path::Path;

use crate::llm::LlmToolCall;

use super::tool_error_json;

pub(super) fn validate_plan_export_targets(
    target_path: &str,
    export_targets: &[String],
    call: &LlmToolCall,
) -> Result<(), String> {
    for target in export_targets {
        if !matches_runner_export_target(target_path, target) {
            return Err(tool_error_json(
                call,
                "export_targets must match runner output names for target_path",
                "invalid_arguments",
            ));
        }
    }
    Ok(())
}

fn matches_runner_export_target(target_path: &str, export_target: &str) -> bool {
    let Some(extension) = runner_export_extension(export_target) else {
        return false;
    };
    export_target
        == format!(
            "outputs/{}.{}",
            cadquery_target_stem(target_path),
            extension
        )
}

fn runner_export_extension(export_target: &str) -> Option<&'static str> {
    let lower = export_target.to_ascii_lowercase();
    if lower.ends_with(".step") {
        Some("step")
    } else if lower.ends_with(".stl") {
        Some("stl")
    } else if lower.ends_with(".3mf") {
        Some("3mf")
    } else {
        None
    }
}

fn cadquery_target_stem(target_path: &str) -> String {
    Path::new(target_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("cadquery")
        .to_owned()
}
