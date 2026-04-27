use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use app_server_protocol::CadQueryExportFormat;

use super::runner::{
    CadQueryRunConfig, CadQueryRunResult, CadQueryRunnerError, CadQueryRunnerErrorKind,
    error_invalid_path, error_io, run_cadquery_runner,
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

pub struct StagedCadQueryProject {
    root: PathBuf,
    workspace_root: PathBuf,
    target_relative_path: PathBuf,
    original_target_path: PathBuf,
    baseline: FileBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileBaseline {
    bytes: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct CommitFile {
    source: PathBuf,
    target: PathBuf,
}

#[derive(Debug)]
struct CommitBackup {
    source: PathBuf,
    target: PathBuf,
    previous_bytes: Option<Vec<u8>>,
}

pub fn execute_cadquery_with_staging(
    config: &CadQueryExecuteConfig,
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    let staged = stage_cadquery_project(
        &config.workspace_root,
        &config.target_relative_path,
        &config.code,
    )?;
    let result = run_cadquery_runner(&CadQueryRunConfig {
        python: config.python.clone(),
        project_root: staged.root().to_path_buf(),
        script: staged.script_arg(),
        output_dir: staged.output_dir(),
        export_formats: config.export_formats.clone(),
        params_json: config.params_json.clone(),
        timeout: config.timeout,
    })?;
    staged.commit_success()?;
    Ok(result)
}

pub fn stage_cadquery_project(
    workspace_root: &Path,
    target_relative_path: &Path,
    code: &str,
) -> Result<StagedCadQueryProject, CadQueryRunnerError> {
    let relative_path = validate_relative_path(target_relative_path)?;
    let original_target_path = workspace_root.join(&relative_path);
    let baseline = FileBaseline::capture(&original_target_path)?;
    let root = workspace_root.join(".budn_staging").join(staging_id());
    let staged = build_staged_project(
        workspace_root,
        root,
        relative_path,
        original_target_path,
        baseline,
        code,
    );
    if let Err(_) = &staged {
        let _ = fs::remove_dir_all(workspace_root.join(".budn_staging"));
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

    pub fn commit_target(self) -> Result<(), CadQueryRunnerError> {
        self.baseline.ensure_unchanged(&self.original_target_path)?;
        commit_files(&self.workspace_root, vec![self.target_commit_file()])
    }

    pub fn commit_success(self) -> Result<(), CadQueryRunnerError> {
        self.baseline.ensure_unchanged(&self.original_target_path)?;
        let mut files = self.output_commit_files()?;
        files.push(self.target_commit_file());
        commit_files(&self.workspace_root, files)
    }

    pub fn commit_outputs(self) -> Result<(), CadQueryRunnerError> {
        self.baseline.ensure_unchanged(&self.original_target_path)?;
        commit_files(&self.workspace_root, self.output_commit_files()?)
    }

    fn target_commit_file(&self) -> CommitFile {
        CommitFile {
            source: self.root.join(&self.target_relative_path),
            target: self.original_target_path.clone(),
        }
    }

    fn output_commit_files(&self) -> Result<Vec<CommitFile>, CadQueryRunnerError> {
        let staged_outputs = self.output_dir();
        if !staged_outputs.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        collect_dir_commit_files(
            &staged_outputs,
            &self.workspace_root.join("outputs"),
            &mut files,
        )?;
        files.sort_by(|left, right| left.target.cmp(&right.target));
        Ok(files)
    }
}

impl Drop for StagedCadQueryProject {
    fn drop(&mut self) {
        let parent = self.root.parent().map(Path::to_path_buf);
        let _ = fs::remove_dir_all(&self.root);
        if let Some(parent) = parent {
            let _ = fs::remove_dir(&parent);
        }
    }
}

impl FileBaseline {
    fn capture(path: &Path) -> Result<Self, CadQueryRunnerError> {
        match fs::read(path) {
            Ok(bytes) => Ok(Self { bytes: Some(bytes) }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self { bytes: None }),
            Err(error) => Err(error_io(format!("读取 CadQuery 文件基线失败: {error}"))),
        }
    }

    fn ensure_unchanged(&self, path: &Path) -> Result<(), CadQueryRunnerError> {
        if *self == Self::capture(path)? {
            return Ok(());
        }
        Err(CadQueryRunnerError {
            kind: CadQueryRunnerErrorKind::FileConflict,
            message: "CadQuery 目标文件已被外部修改".into(),
        })
    }
}

fn build_staged_project(
    workspace_root: &Path,
    root: PathBuf,
    relative_path: PathBuf,
    original_target_path: PathBuf,
    baseline: FileBaseline,
    code: &str,
) -> Result<StagedCadQueryProject, CadQueryRunnerError> {
    copy_workspace(workspace_root, &root)?;
    write_staged_target(&root, &relative_path, code)?;
    Ok(StagedCadQueryProject {
        root,
        workspace_root: workspace_root.to_path_buf(),
        target_relative_path: relative_path,
        original_target_path,
        baseline,
    })
}

fn copy_workspace(source: &Path, target: &Path) -> Result<(), CadQueryRunnerError> {
    copy_workspace_inner(source, target, source)
}

fn copy_workspace_inner(
    source: &Path,
    target: &Path,
    root: &Path,
) -> Result<(), CadQueryRunnerError> {
    fs::create_dir_all(target)
        .map_err(|error| error_io(format!("创建 CadQuery staging 目录失败: {error}")))?;
    for entry in fs::read_dir(source)
        .map_err(|error| error_io(format!("读取 workspace 目录失败: {error}")))?
    {
        copy_workspace_entry(
            &entry.map_err(|error| error_io(error.to_string()))?,
            source,
            target,
            root,
        )?;
    }
    Ok(())
}

fn copy_workspace_entry(
    entry: &fs::DirEntry,
    source: &Path,
    target: &Path,
    root: &Path,
) -> Result<(), CadQueryRunnerError> {
    let file_name = entry.file_name();
    if file_name == ".budn_staging" || (source == root && file_name == "outputs") {
        return Ok(());
    }
    let metadata = entry
        .file_type()
        .map_err(|error| error_io(format!("读取 workspace 条目类型失败: {error}")))?;
    let next_source = source.join(&file_name);
    let next_target = target.join(&file_name);
    if metadata.is_symlink() {
        return Err(error_invalid_path("CadQuery staging 不复制符号链接"));
    }
    if metadata.is_dir() {
        return copy_workspace_inner(&next_source, &next_target, root);
    }
    fs::copy(&next_source, &next_target)
        .map_err(|error| error_io(format!("复制 workspace 文件失败: {error}")))?;
    Ok(())
}

fn write_staged_target(
    root: &Path,
    target_relative_path: &Path,
    code: &str,
) -> Result<(), CadQueryRunnerError> {
    let target = root.join(target_relative_path);
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| error_io(format!("创建 CadQuery staging 目标目录失败: {error}")))?;
    }
    fs::write(target, code)
        .map_err(|error| error_io(format!("写入 CadQuery staging 目标失败: {error}")))
}

fn collect_dir_commit_files(
    source: &Path,
    target: &Path,
    files: &mut Vec<CommitFile>,
) -> Result<(), CadQueryRunnerError> {
    for entry in fs::read_dir(source)
        .map_err(|error| error_io(format!("读取 CadQuery 输出目录失败: {error}")))?
    {
        let entry = entry.map_err(|error| error_io(error.to_string()))?;
        let file_type = entry
            .file_type()
            .map_err(|error| error_io(format!("读取 CadQuery 输出条目失败: {error}")))?;
        let target_path = target.join(entry.file_name());
        if file_type.is_dir() {
            collect_dir_commit_files(&entry.path(), &target_path, files)?;
        } else if file_type.is_file() {
            files.push(CommitFile {
                source: entry.path(),
                target: target_path,
            });
        }
    }
    Ok(())
}

fn commit_files(workspace_root: &Path, files: Vec<CommitFile>) -> Result<(), CadQueryRunnerError> {
    let backups = prepare_commit_files(workspace_root, files)?;
    let mut applied = Vec::new();
    for backup in backups {
        if let Err(error) = atomic_copy_file(&backup.source, &backup.target) {
            rollback_commit(&applied);
            return Err(error_io(format!("提交 CadQuery 文件失败: {error}")));
        }
        applied.push(backup);
    }
    Ok(())
}

fn prepare_commit_files(
    workspace_root: &Path,
    files: Vec<CommitFile>,
) -> Result<Vec<CommitBackup>, CadQueryRunnerError> {
    let mut backups = Vec::with_capacity(files.len());
    for file in files {
        if !file.source.is_file() {
            return Err(error_io(format!(
                "CadQuery staging 文件不存在: {}",
                file.source.display()
            )));
        }
        let target = resolve_commit_target(workspace_root, &file.target)?;
        let previous_bytes = capture_regular_file_backup(&target)?;
        backups.push(CommitBackup {
            source: file.source,
            target,
            previous_bytes,
        });
    }
    Ok(backups)
}

fn resolve_commit_target(
    workspace_root: &Path,
    target: &Path,
) -> Result<PathBuf, CadQueryRunnerError> {
    let parent = target
        .parent()
        .ok_or_else(|| error_invalid_path("CadQuery 提交目标缺少父目录"))?;
    let parent = ensure_commit_parent_inside_workspace(workspace_root, parent)?;
    let file_name = target
        .file_name()
        .ok_or_else(|| error_invalid_path("CadQuery 提交目标缺少文件名"))?;
    Ok(parent.join(file_name))
}

fn ensure_commit_parent_inside_workspace(
    workspace_root: &Path,
    parent: &Path,
) -> Result<PathBuf, CadQueryRunnerError> {
    let canonical_root = workspace_root
        .canonicalize()
        .map_err(|error| error_io(format!("读取 workspace 真实路径失败: {error}")))?;
    let relative_parent = parent
        .strip_prefix(workspace_root)
        .map_err(|_| error_invalid_path("CadQuery 提交目标不在 workspace 内"))?;
    let mut current = canonical_root.clone();
    for component in relative_parent.components() {
        let Component::Normal(segment) = component else {
            return Err(error_invalid_path("CadQuery 提交目标不能逃逸 workspace"));
        };
        let next = current.join(segment);
        current = ensure_commit_dir_inside_workspace(&canonical_root, &next)?;
    }
    Ok(current)
}

fn ensure_commit_dir_inside_workspace(
    canonical_root: &Path,
    path: &Path,
) -> Result<PathBuf, CadQueryRunnerError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(error_invalid_path("CadQuery 提交目录不能是符号链接"));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(error_invalid_path("CadQuery 提交目录不能是普通文件"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path)
                .map_err(|error| error_io(format!("创建 CadQuery 提交目录失败: {error}")))?;
        }
        Err(error) => return Err(error_io(format!("读取 CadQuery 提交目录失败: {error}"))),
    }
    let resolved = path
        .canonicalize()
        .map_err(|error| error_io(format!("读取 CadQuery 提交目录真实路径失败: {error}")))?;
    if !resolved.starts_with(canonical_root) {
        return Err(error_invalid_path(
            "CadQuery 提交目录真实路径不在 workspace 内",
        ));
    }
    Ok(resolved)
}

fn capture_regular_file_backup(path: &Path) -> Result<Option<Vec<u8>>, CadQueryRunnerError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(error_invalid_path("CadQuery 提交目标不能是符号链接"))
        }
        Ok(metadata) if metadata.is_dir() => Err(error_invalid_path("CadQuery 提交目标不能是目录")),
        Ok(_) => fs::read(path)
            .map(Some)
            .map_err(|error| error_io(format!("读取 CadQuery 提交备份失败: {error}"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error_io(format!("读取 CadQuery 提交目标失败: {error}"))),
    }
}

fn rollback_commit(applied: &[CommitBackup]) {
    for backup in applied.iter().rev() {
        match &backup.previous_bytes {
            Some(bytes) => {
                let _ = atomic_write_file(&backup.target, bytes);
            }
            None => {
                let _ = fs::remove_file(&backup.target);
            }
        }
    }
}

fn atomic_copy_file(source: &Path, target: &Path) -> std::io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "target has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".{}.tmp-{}", temp_stem(target), staging_id()));
    if let Err(error) = fs::copy(source, &temp).and_then(|_| fs::rename(&temp, target)) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

fn atomic_write_file(target: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = target.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "target has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".{}.tmp-{}", temp_stem(target), staging_id()));
    if let Err(error) = fs::write(&temp, bytes).and_then(|_| fs::rename(&temp, target)) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(())
}

fn temp_stem(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("cadquery")
        .to_owned()
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

fn staging_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_nanos())
        .unwrap_or_default();
    format!("cq-{}-{nanos}", std::process::id())
}
