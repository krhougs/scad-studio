use std::{
    collections::HashSet,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::fs;

mod commit;

use app_server_protocol::CadQueryExportFormat;

use self::commit::{CommitFile, commit_files, commit_files_cancellable};
use super::runner::{
    CadQueryRunConfig, CadQueryRunResult, CadQueryRunnerError, CadQueryRunnerErrorKind,
    error_invalid_path, error_io, error_permission_denied, run_cadquery_runner_with_cancel,
};

#[derive(Debug, Clone)]
pub struct CadQueryExecuteConfig {
    pub python: PathBuf,
    pub workspace_root: PathBuf,
    pub target_relative_path: PathBuf,
    pub code: String,
    pub export_formats: Vec<CadQueryExportFormat>,
    pub params_json: String,
    pub timeout: Duration,
}

#[must_use = "StagedCadQueryProject must be committed or cleaned up"]
pub struct StagedCadQueryProject {
    root: PathBuf,
    workspace_root: PathBuf,
    target_relative_path: PathBuf,
    original_target_path: PathBuf,
    baseline: FileBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CadQueryCommitScope {
    AllOutputs,
    ExactOutputs(Vec<PathBuf>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileBaseline {
    bytes: Option<Vec<u8>>,
}

pub async fn execute_cadquery_with_staging(
    config: &CadQueryExecuteConfig,
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    execute_cadquery_with_staging_cancellable(config, &|| false).await
}

pub async fn execute_cadquery_with_staging_cancellable(
    config: &CadQueryExecuteConfig,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    execute_cadquery_with_staging_cancellable_scoped(
        config,
        is_cancelled,
        &CadQueryCommitScope::AllOutputs,
    )
    .await
}

pub async fn execute_cadquery_with_staging_cancellable_scoped(
    config: &CadQueryExecuteConfig,
    is_cancelled: &(dyn Fn() -> bool + Sync),
    commit_scope: &CadQueryCommitScope,
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    ensure_not_cancelled(is_cancelled)?;
    let staged = stage_cadquery_project(
        &config.workspace_root,
        &config.target_relative_path,
        &config.code,
    )
    .await?;
    if let Err(error) = ensure_not_cancelled(is_cancelled) {
        staged.cleanup().await;
        return Err(error);
    }
    let result = run_cadquery_runner_with_cancel(
        CadQueryRunConfig {
            python: config.python.clone(),
            project_root: staged.root().to_path_buf(),
            script: staged.script_arg(),
            output_dir: staged.output_dir(),
            export_formats: config.export_formats.clone(),
            params_json: config.params_json.clone(),
            timeout: config.timeout,
        },
        is_cancelled,
    )
    .await;
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            staged.cleanup().await;
            return Err(error);
        }
    };
    if let Err(error) = ensure_not_cancelled(is_cancelled) {
        staged.cleanup().await;
        return Err(error);
    }
    staged
        .commit_success_with_scope_cancellable(commit_scope, is_cancelled)
        .await?;
    Ok(result)
}

pub async fn stage_cadquery_project(
    workspace_root: &Path,
    target_relative_path: &Path,
    code: &str,
) -> Result<StagedCadQueryProject, CadQueryRunnerError> {
    stage_cadquery_project_owned(
        workspace_root.to_path_buf(),
        target_relative_path.to_path_buf(),
        code.to_owned(),
    )
    .await
}

pub async fn stage_cadquery_project_owned(
    workspace_root: PathBuf,
    target_relative_path: PathBuf,
    code: String,
) -> Result<StagedCadQueryProject, CadQueryRunnerError> {
    let relative_path = validate_relative_path(&target_relative_path)?;
    let original_target_path = workspace_root.join(&relative_path);
    let baseline = FileBaseline::capture(original_target_path.clone()).await?;
    let root = workspace_root.join(".budn_staging").join(staging_id());
    let staged = build_staged_project(
        workspace_root.clone(),
        root,
        relative_path,
        original_target_path,
        baseline,
        code,
    )
    .await;
    if let Err(_) = &staged {
        let _ = fs::remove_dir_all(workspace_root.join(".budn_staging")).await;
    }
    staged
}

impl StagedCadQueryProject {
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn output_dir(&self) -> PathBuf {
        self.root.join("outputs")
    }

    pub fn script_arg(&self) -> String {
        self.target_relative_path
            .to_string_lossy()
            .replace('\\', "/")
    }

    pub async fn commit_target(self) -> Result<(), CadQueryRunnerError> {
        let result = if let Err(error) = self
            .baseline
            .ensure_unchanged(self.original_target_path.clone())
            .await
        {
            Err(error)
        } else {
            commit_files(self.workspace_root.clone(), vec![self.target_commit_file()]).await
        };
        self.cleanup().await;
        result
    }

    pub async fn commit_success(self) -> Result<(), CadQueryRunnerError> {
        self.commit_success_with_scope(&CadQueryCommitScope::AllOutputs)
            .await
    }

    pub async fn commit_success_with_scope(
        self,
        scope: &CadQueryCommitScope,
    ) -> Result<(), CadQueryRunnerError> {
        self.commit_success_with_scope_cancellable(scope, &|| false)
            .await
    }

    pub async fn commit_success_with_scope_cancellable(
        self,
        scope: &CadQueryCommitScope,
        is_cancelled: &(dyn Fn() -> bool + Sync),
    ) -> Result<(), CadQueryRunnerError> {
        let result = async {
            self.baseline
                .ensure_unchanged(self.original_target_path.clone())
                .await?;
            let mut files = self.output_commit_files_for_scope(scope).await?;
            files.push(self.target_commit_file());
            commit_files_cancellable(self.workspace_root.clone(), files, is_cancelled).await
        }
        .await;
        self.cleanup().await;
        result
    }

    pub async fn commit_outputs(self) -> Result<(), CadQueryRunnerError> {
        self.commit_outputs_with_scope(&CadQueryCommitScope::AllOutputs)
            .await
    }

    pub async fn commit_outputs_with_scope(
        self,
        scope: &CadQueryCommitScope,
    ) -> Result<(), CadQueryRunnerError> {
        let result = async {
            self.baseline
                .ensure_unchanged(self.original_target_path.clone())
                .await?;
            let files = self.output_commit_files_for_scope(scope).await?;
            commit_files(self.workspace_root.clone(), files).await
        }
        .await;
        self.cleanup().await;
        result
    }

    fn target_commit_file(&self) -> CommitFile {
        CommitFile {
            source: self.root.join(&self.target_relative_path),
            target: self.original_target_path.clone(),
        }
    }

    async fn output_commit_files(&self) -> Result<Vec<CommitFile>, CadQueryRunnerError> {
        let staged_outputs = self.output_dir();
        if !fs::try_exists(staged_outputs.clone())
            .await
            .unwrap_or(false)
        {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        collect_dir_commit_files(
            staged_outputs,
            self.workspace_root.join("outputs"),
            &mut files,
        )
        .await?;
        files.sort_by(|left, right| left.target.cmp(&right.target));
        Ok(files)
    }

    async fn output_commit_files_for_scope(
        &self,
        scope: &CadQueryCommitScope,
    ) -> Result<Vec<CommitFile>, CadQueryRunnerError> {
        let files = self.output_commit_files().await?;
        match scope {
            CadQueryCommitScope::AllOutputs => Ok(files),
            CadQueryCommitScope::ExactOutputs(paths) => {
                let allowed = allowed_output_paths(paths)?;
                validate_output_commit_files(&self.workspace_root, &files, &allowed)?;
                Ok(files)
            }
        }
    }
    pub async fn cleanup(self) {
        let parent = self.root.parent().map(Path::to_path_buf);
        let _ = fs::remove_dir_all(self.root.clone()).await;
        if let Some(parent) = parent {
            let _ = fs::remove_dir(parent).await;
        }
    }
}

impl FileBaseline {
    async fn capture(path: PathBuf) -> Result<Self, CadQueryRunnerError> {
        match fs::read(path).await {
            Ok(bytes) => Ok(Self { bytes: Some(bytes) }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self { bytes: None }),
            Err(error) => Err(error_io(format!("读取 CadQuery 文件基线失败: {error}"))),
        }
    }

    async fn ensure_unchanged(&self, path: PathBuf) -> Result<(), CadQueryRunnerError> {
        if *self == Self::capture(path).await? {
            return Ok(());
        }
        Err(CadQueryRunnerError {
            kind: CadQueryRunnerErrorKind::FileConflict,
            message: "CadQuery 目标文件已被外部修改".into(),
        })
    }
}

async fn build_staged_project(
    workspace_root: PathBuf,
    root: PathBuf,
    relative_path: PathBuf,
    original_target_path: PathBuf,
    baseline: FileBaseline,
    code: String,
) -> Result<StagedCadQueryProject, CadQueryRunnerError> {
    copy_workspace(workspace_root.clone(), root.clone()).await?;
    write_staged_target(root.clone(), relative_path.clone(), code).await?;
    Ok(StagedCadQueryProject {
        root,
        workspace_root,
        target_relative_path: relative_path,
        original_target_path,
        baseline,
    })
}

async fn copy_workspace(source: PathBuf, target: PathBuf) -> Result<(), CadQueryRunnerError> {
    copy_workspace_inner(source.clone(), target, source).await
}

async fn copy_workspace_inner(
    source: PathBuf,
    target: PathBuf,
    root: PathBuf,
) -> Result<(), CadQueryRunnerError> {
    fs::create_dir_all(target.clone())
        .await
        .map_err(|error| error_io(format!("创建 CadQuery staging 目录失败: {error}")))?;
    let mut directories = vec![(source, target)];
    while let Some((source, target)) = directories.pop() {
        fs::create_dir_all(target.clone())
            .await
            .map_err(|error| error_io(format!("创建 CadQuery staging 目录失败: {error}")))?;
        let mut entries = fs::read_dir(source.clone())
            .await
            .map_err(|error| error_io(format!("读取 workspace 目录失败: {error}")))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| error_io(error.to_string()))?
        {
            copy_workspace_entry(
                entry,
                source.clone(),
                target.clone(),
                root.clone(),
                &mut directories,
            )
            .await?;
        }
    }
    Ok(())
}

async fn copy_workspace_entry(
    entry: fs::DirEntry,
    source: PathBuf,
    target: PathBuf,
    root: PathBuf,
    directories: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), CadQueryRunnerError> {
    let file_name = entry.file_name();
    if file_name == ".budn_staging" || (source == root && file_name == "outputs") {
        return Ok(());
    }
    let metadata = entry
        .file_type()
        .await
        .map_err(|error| error_io(format!("读取 workspace 条目类型失败: {error}")))?;
    let next_source = source.join(&file_name);
    let next_target = target.join(&file_name);
    if metadata.is_symlink() {
        return Err(error_invalid_path("CadQuery staging 不复制符号链接"));
    }
    if metadata.is_dir() {
        directories.push((next_source, next_target));
        return Ok(());
    }
    fs::copy(next_source, next_target)
        .await
        .map_err(|error| error_io(format!("复制 workspace 文件失败: {error}")))?;
    Ok(())
}

async fn write_staged_target(
    root: PathBuf,
    target_relative_path: PathBuf,
    code: String,
) -> Result<(), CadQueryRunnerError> {
    let target = root.join(target_relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| error_io(format!("创建 CadQuery staging 目标目录失败: {error}")))?;
    }
    fs::write(target, code)
        .await
        .map_err(|error| error_io(format!("写入 CadQuery staging 目标失败: {error}")))
}

async fn collect_dir_commit_files(
    source: PathBuf,
    target: PathBuf,
    files: &mut Vec<CommitFile>,
) -> Result<(), CadQueryRunnerError> {
    let mut directories = vec![(source, target)];
    while let Some((source, target)) = directories.pop() {
        let mut entries = fs::read_dir(source)
            .await
            .map_err(|error| error_io(format!("读取 CadQuery 输出目录失败: {error}")))?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| error_io(error.to_string()))?
        {
            let file_type = entry
                .file_type()
                .await
                .map_err(|error| error_io(format!("读取 CadQuery 输出条目失败: {error}")))?;
            let target_path = target.join(entry.file_name());
            if file_type.is_dir() {
                directories.push((entry.path(), target_path));
            } else if file_type.is_file() {
                files.push(CommitFile {
                    source: entry.path(),
                    target: target_path,
                });
            }
        }
    }
    Ok(())
}

fn allowed_output_paths(paths: &[PathBuf]) -> Result<HashSet<PathBuf>, CadQueryRunnerError> {
    let mut allowed = HashSet::with_capacity(paths.len());
    for path in paths {
        let normalized = validate_relative_path(path)?;
        if !normalized.starts_with("outputs") {
            return Err(error_permission_denied(
                "CadQuery export target 必须位于 outputs/ 目录",
            ));
        }
        allowed.insert(normalized);
    }
    Ok(allowed)
}

fn validate_output_commit_files(
    workspace_root: &Path,
    files: &[CommitFile],
    allowed: &HashSet<PathBuf>,
) -> Result<(), CadQueryRunnerError> {
    for file in files {
        let relative = file
            .target
            .strip_prefix(workspace_root)
            .map_err(|_| error_invalid_path("CadQuery 输出提交目标不在 workspace 内"))?;
        let relative = validate_relative_path(relative)?;
        if !allowed.contains(&relative) {
            return Err(error_permission_denied(format!(
                "CadQuery runner 生成了未确认输出: {}",
                display_relative_path(&relative)
            )));
        }
    }
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<PathBuf, CadQueryRunnerError> {
    if path.is_absolute() {
        return Err(error_invalid_path("CadQuery 目标路径必须是相对路径"));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => normalized.push(value),
            _ => return Err(error_invalid_path("CadQuery 目标路径不能逃逸 workspace")),
        }
    }
    Ok(normalized)
}

fn display_relative_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn ensure_not_cancelled(
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), CadQueryRunnerError> {
    if is_cancelled() {
        return Err(CadQueryRunnerError {
            kind: CadQueryRunnerErrorKind::Cancelled,
            message: "CadQuery runner 已取消".into(),
        });
    }
    Ok(())
}

fn staging_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_nanos())
        .unwrap_or_default();
    format!("cq-{}-{nanos}", std::process::id())
}
