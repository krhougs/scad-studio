use std::path::{Path, PathBuf};

use app_server_protocol::CadQueryObjectKind;
use jiff::Zoned;
use tokio::fs;

mod front_matter;
use front_matter::{parse_plan_metadata, target_type_label};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPackageError {
    pub message: String,
    pub error_type: &'static str,
}

impl PlanPackageError {
    fn new(message: impl Into<String>, error_type: &'static str) -> Self {
        Self {
            message: message.into(),
            error_type,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanTimestamp {
    pub date_prefix: String,
    pub created_at: String,
}

impl PlanTimestamp {
    pub fn now() -> Self {
        let now = Zoned::now();
        Self {
            date_prefix: now.strftime("%Y%m%d").to_string(),
            created_at: now.strftime("%Y-%m-%dT%H:%M:%S%z").to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveCadPlanPackageInput {
    pub title: String,
    pub request: String,
    pub target_ref: String,
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub affected_files: Vec<String>,
    pub new_files: Vec<String>,
    pub export_targets: Vec<String>,
    pub strategy: String,
    pub risks: Vec<String>,
    pub acceptance: Vec<String>,
    pub execution_scope: String,
    pub source_chat_session: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPackagePaths {
    pub plan_id: String,
    pub plan_ref: String,
    pub request_path: String,
    pub plan_path: String,
    pub result_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedPlanPackage {
    pub paths: PlanPackagePaths,
    pub hash_source: String,
    pub plan_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPlanPackage {
    pub plan_id: String,
    pub plan_ref: String,
    pub request_path: String,
    pub plan_path: String,
    pub result_path: String,
    pub title: String,
    pub status: String,
    pub target_path: String,
    pub target_type: CadQueryObjectKind,
    pub affected_files: Vec<String>,
    pub new_files: Vec<String>,
    pub export_targets: Vec<String>,
}

pub async fn save_plan_package(
    workspace_root: &Path,
    input: &SaveCadPlanPackageInput,
) -> Result<SavedPlanPackage, PlanPackageError> {
    save_plan_package_with_timestamp(workspace_root, input, PlanTimestamp::now()).await
}

pub async fn save_plan_package_with_timestamp(
    workspace_root: &Path,
    input: &SaveCadPlanPackageInput,
    timestamp: PlanTimestamp,
) -> Result<SavedPlanPackage, PlanPackageError> {
    let plans_dir = safe_plans_dir(workspace_root).await?;
    let plan_id = allocate_plan_id(&plans_dir, &timestamp.date_prefix, &input.title).await?;
    let paths = package_paths(&plan_id);
    let absolute_dir = workspace_root.join(&paths.plan_ref);
    fs::create_dir(&absolute_dir).await.map_err(|error| {
        PlanPackageError::new(format!("创建 plan package 失败: {error}"), "file_conflict")
    })?;
    write_package_files(&absolute_dir, input, &paths, &timestamp).await?;
    Ok(SavedPlanPackage {
        paths,
        hash_source: render_plan_markdown(input, &plan_id, &timestamp),
        plan_status: "planned".into(),
    })
}

pub async fn parse_plan_package(
    workspace_root: &Path,
    plan_ref: &str,
) -> Result<ParsedPlanPackage, PlanPackageError> {
    let normalized_ref = normalize_workspace_path(plan_ref, "plan_ref")?;
    let plan_id = plan_id_from_ref(&normalized_ref)?;
    let plans_dir = safe_existing_plans_dir(workspace_root).await?;
    let dir = safe_package_dir(&plans_dir.join(&plan_id)).await?;
    require_package_file(&dir, "request.md").await?;
    let plan_path = require_package_file(&dir, "plan.md").await?;
    require_package_file(&dir, "plan-result.md").await?;
    let plan_text = read_utf8(&plan_path, "plan.md").await?;
    let metadata = parse_plan_metadata(&plan_text, &plan_id)?;
    let title = extract_plan_title(&plan_text).unwrap_or_else(|| plan_id.clone());
    let paths = package_paths(&plan_id);
    Ok(ParsedPlanPackage {
        plan_id,
        plan_ref: paths.plan_ref,
        request_path: paths.request_path,
        plan_path: paths.plan_path,
        result_path: paths.result_path,
        title,
        status: metadata.status,
        target_path: metadata.target_path,
        target_type: metadata.target_type,
        affected_files: metadata.affected_files,
        new_files: metadata.new_files,
        export_targets: metadata.export_targets,
    })
}

pub async fn collect_plan_packages(workspace_root: &Path) -> (Vec<ParsedPlanPackage>, Vec<String>) {
    let mut packages = Vec::new();
    let mut warnings = Vec::new();
    let plans_dir = match safe_existing_plans_dir(workspace_root).await {
        Ok(dir) => dir,
        Err(error) if error.error_type == "not_found" => return (packages, warnings),
        Err(error) => {
            warnings.push(error.message);
            return (packages, warnings);
        }
    };
    let Ok(mut entries) = fs::read_dir(&plans_dir).await else {
        return (packages, warnings);
    };
    loop {
        let entry = match entries.next_entry().await {
            Ok(Some(entry)) => entry,
            Ok(None) => break,
            Err(_) => continue,
        };
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !looks_like_plan_id(&file_name) {
            continue;
        }
        match parse_plan_package(workspace_root, &format!("plans/{file_name}")).await {
            Ok(package) => packages.push(package),
            Err(error) => warnings.push(format!("plans/{file_name}: {}", error.message)),
        }
    }
    packages.sort_by(|left, right| left.plan_ref.cmp(&right.plan_ref));
    (packages, warnings)
}

pub fn slugify_plan_title(title: &str) -> String {
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

async fn write_package_files(
    absolute_dir: &Path,
    input: &SaveCadPlanPackageInput,
    paths: &PlanPackagePaths,
    timestamp: &PlanTimestamp,
) -> Result<(), PlanPackageError> {
    let request = render_request_markdown(input);
    let plan = render_plan_markdown(input, &paths.plan_id, timestamp);
    let result = render_initial_result_markdown(input, &paths.plan_id, timestamp);
    write_file_or_rollback(absolute_dir, "request.md", &request).await?;
    write_file_or_rollback(absolute_dir, "plan.md", &plan).await?;
    write_file_or_rollback(absolute_dir, "plan-result.md", &result).await?;
    Ok(())
}

async fn write_file_or_rollback(
    absolute_dir: &Path,
    file_name: &str,
    contents: &str,
) -> Result<(), PlanPackageError> {
    if let Err(error) = fs::write(absolute_dir.join(file_name), contents.as_bytes()).await {
        let _ = fs::remove_dir_all(absolute_dir).await;
        Err(PlanPackageError::new(
            format!("写入 plan package 失败: {error}"),
            "file_conflict",
        ))
    } else {
        Ok(())
    }
}

async fn safe_plans_dir(workspace_root: &Path) -> Result<PathBuf, PlanPackageError> {
    let plans_dir = workspace_root.join("plans");
    match fs::symlink_metadata(&plans_dir).await {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(PlanPackageError::new(
            "plans directory must not be a symlink",
            "permission_denied",
        )),
        Ok(metadata) if metadata.is_dir() => Ok(plans_dir),
        Ok(_) => Err(PlanPackageError::new(
            "plans path must be a directory",
            "invalid_arguments",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(&plans_dir).await.map_err(|error| {
                PlanPackageError::new(format!("创建 plans 目录失败: {error}"), "file_conflict")
            })?;
            Ok(plans_dir)
        }
        Err(error) => Err(PlanPackageError::new(
            format!("读取 plans 目录失败: {error}"),
            "file_conflict",
        )),
    }
}

async fn allocate_plan_id(
    plans_dir: &Path,
    date_prefix: &str,
    title: &str,
) -> Result<String, PlanPackageError> {
    let next = next_daily_sequence(plans_dir, date_prefix).await?;
    let slug = slugify_plan_title(title);
    Ok(format!("{date_prefix}{next:02}-{slug}"))
}

async fn next_daily_sequence(plans_dir: &Path, date_prefix: &str) -> Result<u8, PlanPackageError> {
    let mut max_seen: Option<u8> = None;
    let mut entries = fs::read_dir(plans_dir).await.map_err(read_dir_error)?;
    while let Some(entry) = entries.next_entry().await.map_err(read_dir_error)? {
        let name = entry.file_name().to_string_lossy().to_string();
        let Some(sequence) = sequence_for_date(&name, date_prefix) else {
            continue;
        };
        max_seen = Some(max_seen.map_or(sequence, |max| max.max(sequence)));
    }
    max_seen
        .and_then(|value| value.checked_add(1))
        .or(Some(0))
        .filter(|value| *value <= 99)
        .ok_or_else(|| PlanPackageError::new("daily plan sequence is exhausted", "file_conflict"))
}

fn sequence_for_date(name: &str, date_prefix: &str) -> Option<u8> {
    let rest = name.strip_prefix(date_prefix)?;
    if rest.len() < 3 || rest.as_bytes().get(2) != Some(&b'-') {
        return None;
    }
    rest.get(0..2)?.parse().ok()
}

fn read_dir_error(error: std::io::Error) -> PlanPackageError {
    PlanPackageError::new(format!("读取 plans 目录失败: {error}"), "file_conflict")
}

fn package_paths(plan_id: &str) -> PlanPackagePaths {
    let plan_ref = format!("plans/{plan_id}");
    PlanPackagePaths {
        plan_id: plan_id.into(),
        request_path: format!("{plan_ref}/request.md"),
        plan_path: format!("{plan_ref}/plan.md"),
        result_path: format!("{plan_ref}/plan-result.md"),
        plan_ref,
    }
}

fn render_request_markdown(input: &SaveCadPlanPackageInput) -> String {
    format!(
        "# Request\n\n{}\n\n## Target Ref\n\n{}\n",
        input.request.trim(),
        input.target_ref
    )
}

fn render_plan_markdown(
    input: &SaveCadPlanPackageInput,
    plan_id: &str,
    timestamp: &PlanTimestamp,
) -> String {
    format!(
        "---\n{}---\n\n# CAD Plan: {}\n\n## Goal\n\n{}\n\n## Current Context\n\nTarget ref: `{}`\n\n## CadQuery Strategy\n\n{}\n\n## Risks\n{}\n\n## Acceptance\n{}\n\n## Execution Scope\n\n{}\n",
        render_front_matter(input, plan_id, timestamp),
        input.title,
        input.request.trim(),
        input.target_ref,
        input.strategy.trim(),
        markdown_list(&input.risks),
        markdown_list(&input.acceptance),
        input.execution_scope.trim()
    )
}

fn render_front_matter(
    input: &SaveCadPlanPackageInput,
    plan_id: &str,
    timestamp: &PlanTimestamp,
) -> String {
    format!(
        "plan_id: {plan_id}\nmode: plan\ntarget_path: {}\ntarget_type: {}\naffected_files:\n{}new_files:\n{}export_targets:\n{}status: planned\ncreated_at: {}\nsource_chat_session: {}\n",
        input.target_path,
        target_type_label(input.target_type),
        yaml_array(&input.affected_files),
        yaml_array_or_empty(&input.new_files),
        yaml_array(&input.export_targets),
        timestamp.created_at,
        input.source_chat_session.as_deref().unwrap_or("")
    )
}

fn render_initial_result_markdown(
    input: &SaveCadPlanPackageInput,
    plan_id: &str,
    timestamp: &PlanTimestamp,
) -> String {
    format!(
        "status: pending\nplan_id: {plan_id}\ncreated_at: {}\n\n# Plan Result: {}\n",
        timestamp.created_at, input.title
    )
}

fn yaml_array(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("  - {item}\n"))
        .collect::<String>()
}

fn yaml_array_or_empty(items: &[String]) -> String {
    if items.is_empty() {
        " []\n".into()
    } else {
        format!("\n{}", yaml_array(items))
    }
}

fn markdown_list(items: &[String]) -> String {
    if items.is_empty() {
        "- none".into()
    } else {
        items
            .iter()
            .map(|item| format!("- {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

async fn safe_existing_plans_dir(workspace_root: &Path) -> Result<PathBuf, PlanPackageError> {
    let plans_dir = workspace_root.join("plans");
    match fs::symlink_metadata(&plans_dir).await {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(PlanPackageError::new(
            "plans directory must not be a symlink",
            "permission_denied",
        )),
        Ok(metadata) if metadata.is_dir() => Ok(plans_dir),
        Ok(_) => Err(PlanPackageError::new(
            "plans path must be a directory",
            "invalid_arguments",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(PlanPackageError::new(
            "plans directory not found",
            "not_found",
        )),
        Err(error) => Err(PlanPackageError::new(
            format!("读取 plans 目录失败: {error}"),
            "file_conflict",
        )),
    }
}

async fn safe_package_dir(dir: &Path) -> Result<PathBuf, PlanPackageError> {
    let metadata = fs::symlink_metadata(&dir).await.map_err(|error| {
        PlanPackageError::new(
            format!("读取 plan package 失败: {error}"),
            "invalid_arguments",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(PlanPackageError::new(
            "plan_ref must point to a plan package directory",
            "permission_denied",
        ));
    }
    Ok(dir.to_path_buf())
}

async fn require_package_file(dir: &Path, file_name: &str) -> Result<PathBuf, PlanPackageError> {
    let path = dir.join(file_name);
    let metadata = fs::symlink_metadata(&path).await.map_err(|_| {
        PlanPackageError::new(
            format!("plan package missing {file_name}"),
            "invalid_arguments",
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        Err(PlanPackageError::new(
            format!("{file_name} must be a regular file"),
            "permission_denied",
        ))
    } else {
        Ok(path)
    }
}

async fn read_utf8(path: &Path, label: &str) -> Result<String, PlanPackageError> {
    fs::read_to_string(path).await.map_err(|error| {
        PlanPackageError::new(format!("读取 {label} 失败: {error}"), "invalid_arguments")
    })
}

fn plan_id_from_ref(plan_ref: &str) -> Result<String, PlanPackageError> {
    let parts = plan_ref.split('/').collect::<Vec<_>>();
    if parts.len() == 2 && parts[0] == "plans" && looks_like_plan_id(parts[1]) {
        Ok(parts[1].to_owned())
    } else {
        Err(PlanPackageError::new(
            "plan_ref must be plans/YYYYmmddnn-name",
            "invalid_arguments",
        ))
    }
}

fn looks_like_plan_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 12 && bytes[..10].iter().all(u8::is_ascii_digit) && bytes.get(10) == Some(&b'-')
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

fn extract_plan_title(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .find_map(|line| line.strip_prefix("# CAD Plan: "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}
