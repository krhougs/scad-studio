use std::collections::HashMap;

use app_server_protocol::CadQueryObjectKind;

use super::PlanPackageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlanMetadata {
    pub status: String,
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub affected_files: Vec<String>,
    pub new_files: Vec<String>,
    pub export_targets: Vec<String>,
}

pub(super) fn parse_plan_metadata(
    markdown: &str,
    plan_id: &str,
) -> Result<PlanMetadata, PlanPackageError> {
    let metadata = parse_front_matter(markdown)?;
    normalized_metadata(metadata, plan_id)
}

pub(super) fn target_type_label(target_type: CadQueryObjectKind) -> &'static str {
    match target_type {
        CadQueryObjectKind::Assembly => "assembly",
        CadQueryObjectKind::Component => "component",
        CadQueryObjectKind::Part => "part",
    }
}

#[derive(Debug, Clone, Default)]
struct FrontMatter {
    scalars: HashMap<String, String>,
    arrays: HashMap<String, Vec<String>>,
}

fn parse_front_matter(markdown: &str) -> Result<FrontMatter, PlanPackageError> {
    let rest = markdown.strip_prefix("---\n").ok_or_else(|| {
        PlanPackageError::new("plan.md missing YAML front matter", "invalid_arguments")
    })?;
    let end = rest.find("\n---").ok_or_else(|| {
        PlanPackageError::new("plan.md front matter is not closed", "invalid_arguments")
    })?;
    parse_front_matter_block(&rest[..end])
}

fn parse_front_matter_block(block: &str) -> Result<FrontMatter, PlanPackageError> {
    let mut parsed = FrontMatter::default();
    let lines = block.lines().collect::<Vec<_>>();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let Some((key, value)) = line.split_once(':') else {
            return Err(PlanPackageError::new(
                "invalid front matter line",
                "invalid_arguments",
            ));
        };
        index += parse_front_matter_entry(&lines[index..], key.trim(), value.trim(), &mut parsed)?;
    }
    Ok(parsed)
}

fn parse_front_matter_entry(
    lines: &[&str],
    key: &str,
    value: &str,
    parsed: &mut FrontMatter,
) -> Result<usize, PlanPackageError> {
    if value == "[]" {
        parsed.arrays.insert(key.into(), Vec::new());
        return Ok(1);
    }
    if !value.is_empty() {
        parsed.scalars.insert(key.into(), value.into());
        return Ok(1);
    }
    let values = collect_yaml_array(&lines[1..]);
    parsed.arrays.insert(key.into(), values.0);
    Ok(values.1 + 1)
}

fn collect_yaml_array(lines: &[&str]) -> (Vec<String>, usize) {
    let mut items = Vec::new();
    let mut consumed = 0;
    for line in lines {
        let trimmed = line.trim_start();
        let Some(value) = trimmed.strip_prefix("- ") else {
            break;
        };
        items.push(value.trim().to_owned());
        consumed += 1;
    }
    (items, consumed)
}

fn normalized_metadata(
    metadata: FrontMatter,
    plan_id: &str,
) -> Result<PlanMetadata, PlanPackageError> {
    require_equal(&metadata, "plan_id", plan_id)?;
    require_equal(&metadata, "mode", "plan")?;
    require_equal(&metadata, "status", "planned")?;
    let status = required_scalar(&metadata, "status")?;
    let target_path =
        normalize_model_path(&required_scalar(&metadata, "target_path")?, "target_path")?;
    let target_type = parse_target_type(&required_scalar(&metadata, "target_type")?)?;
    validate_target_type_matches_path(&target_path, target_type)?;
    let affected = normalize_paths(
        required_array(&metadata, "affected_files")?,
        "affected_files",
    )?;
    let new_files = normalize_paths(
        metadata
            .arrays
            .get("new_files")
            .cloned()
            .unwrap_or_default(),
        "new_files",
    )?;
    validate_target_in_scope(&target_path, &affected, &new_files)?;
    let export_targets =
        normalize_export_targets(&target_path, &required_array(&metadata, "export_targets")?)?;
    Ok(PlanMetadata {
        status,
        target_path,
        target_type,
        affected_files: affected,
        new_files,
        export_targets,
    })
}

fn require_equal(
    metadata: &FrontMatter,
    key: &str,
    expected: &str,
) -> Result<(), PlanPackageError> {
    let actual = required_scalar(metadata, key)?;
    if actual == expected {
        Ok(())
    } else {
        Err(PlanPackageError::new(
            format!("{key} does not match plan package"),
            "invalid_arguments",
        ))
    }
}

fn required_scalar(metadata: &FrontMatter, key: &str) -> Result<String, PlanPackageError> {
    metadata
        .scalars
        .get(key)
        .cloned()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            PlanPackageError::new(
                format!("missing required front matter field {key}"),
                "invalid_arguments",
            )
        })
}

fn required_array(metadata: &FrontMatter, key: &str) -> Result<Vec<String>, PlanPackageError> {
    let values = metadata.arrays.get(key).cloned().ok_or_else(|| {
        PlanPackageError::new(
            format!("missing required front matter array {key}"),
            "invalid_arguments",
        )
    })?;
    if values.is_empty() {
        Err(PlanPackageError::new(
            format!("{key} must not be empty"),
            "invalid_arguments",
        ))
    } else {
        Ok(values)
    }
}

fn normalize_model_path(path: &str, field: &str) -> Result<String, PlanPackageError> {
    let normalized = normalize_workspace_path(path, field)?;
    if !matches!(
        first_segment(&normalized),
        "components" | "parts" | "assemblies"
    ) {
        return Err(PlanPackageError::new(
            format!("{field} must be a CadQuery model path"),
            "permission_denied",
        ));
    }
    if !normalized.ends_with(".py") {
        return Err(PlanPackageError::new(
            format!("{field} must end with .py"),
            "invalid_arguments",
        ));
    }
    Ok(normalized)
}

fn normalize_paths(paths: Vec<String>, field: &str) -> Result<Vec<String>, PlanPackageError> {
    paths
        .into_iter()
        .map(|path| normalize_workspace_path(&path, field))
        .collect()
}

fn validate_target_in_scope(
    target: &str,
    affected: &[String],
    new_files: &[String],
) -> Result<(), PlanPackageError> {
    if affected.iter().any(|path| path == target) || new_files.iter().any(|path| path == target) {
        Ok(())
    } else {
        Err(PlanPackageError::new(
            "target_path must be in affected_files or new_files",
            "invalid_arguments",
        ))
    }
}

fn normalize_export_targets(
    target: &str,
    exports: &[String],
) -> Result<Vec<String>, PlanPackageError> {
    let mut normalized_exports = Vec::new();
    for export in exports {
        let normalized = normalize_workspace_path(export, "export_targets")?;
        if first_segment(&normalized) != "outputs" || !supported_export_extension(&normalized) {
            return Err(PlanPackageError::new(
                "export_targets must use outputs/*.step, .stl, or .3mf",
                "invalid_arguments",
            ));
        }
        if !matches_runner_export_target(target, &normalized) {
            return Err(PlanPackageError::new(
                "export_targets must match runner output names for target_path",
                "invalid_arguments",
            ));
        }
        normalized_exports.push(normalized);
    }
    Ok(normalized_exports)
}

fn matches_runner_export_target(target: &str, export: &str) -> bool {
    let Some(stem) = std::path::Path::new(target)
        .file_stem()
        .and_then(|value| value.to_str())
    else {
        return false;
    };
    ["step", "stl", "3mf"]
        .iter()
        .any(|extension| export == format!("outputs/{stem}.{extension}"))
}

fn supported_export_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".step") || lower.ends_with(".stl") || lower.ends_with(".3mf")
}

fn validate_target_type_matches_path(
    target_path: &str,
    target_type: CadQueryObjectKind,
) -> Result<(), PlanPackageError> {
    let expected = match first_segment(target_path) {
        "assemblies" => CadQueryObjectKind::Assembly,
        "components" => CadQueryObjectKind::Component,
        _ => CadQueryObjectKind::Part,
    };
    if target_type == expected {
        Ok(())
    } else {
        Err(PlanPackageError::new(
            "target_type does not match target_path",
            "invalid_arguments",
        ))
    }
}

fn parse_target_type(value: &str) -> Result<CadQueryObjectKind, PlanPackageError> {
    match value {
        "assembly" => Ok(CadQueryObjectKind::Assembly),
        "component" => Ok(CadQueryObjectKind::Component),
        "part" => Ok(CadQueryObjectKind::Part),
        _ => Err(PlanPackageError::new(
            "target_type must be part, component, or assembly",
            "invalid_arguments",
        )),
    }
}

fn normalize_workspace_path(path: &str, field: &str) -> Result<String, PlanPackageError> {
    let cleaned = path.trim().replace('\\', "/");
    if cleaned.is_empty() || cleaned.starts_with('/') || cleaned.contains(':') {
        return Err(PlanPackageError::new(
            format!("{field} must be workspace-relative"),
            "permission_denied",
        ));
    }
    if cleaned.split('/').any(|segment| segment == "..") {
        return Err(PlanPackageError::new(
            format!("{field} must not contain '..'"),
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

fn first_segment(path: &str) -> &str {
    path.split('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or("")
}
