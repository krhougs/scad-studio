use std::path::{Component, Path, PathBuf};

use super::{ensure_not_cancelled, staging_id};
use crate::cadquery::runner::{CadQueryRunnerError, error_invalid_path, error_io};
use tokio::fs;

#[derive(Debug, Clone)]
pub(super) struct CommitFile {
    pub(super) source: PathBuf,
    pub(super) target: PathBuf,
}

#[derive(Debug)]
struct CommitBackup {
    source: PathBuf,
    target: PathBuf,
    previous_bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
struct CommitPlan {
    backups: Vec<CommitBackup>,
    created_dirs: Vec<PathBuf>,
}

pub(super) async fn commit_files(
    workspace_root: PathBuf,
    files: Vec<CommitFile>,
) -> Result<(), CadQueryRunnerError> {
    commit_files_cancellable(workspace_root, files, &|| false).await
}

pub(super) async fn commit_files_cancellable(
    workspace_root: PathBuf,
    files: Vec<CommitFile>,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<(), CadQueryRunnerError> {
    ensure_not_cancelled(is_cancelled)?;
    let CommitPlan {
        backups,
        created_dirs,
    } = prepare_commit_files(workspace_root, files).await?;
    let mut applied = Vec::new();
    for backup in backups {
        if let Err(error) = ensure_not_cancelled(is_cancelled) {
            rollback_commit(&applied).await;
            rollback_created_dirs(&created_dirs).await;
            return Err(error);
        }
        if let Err(error) = atomic_copy_file(backup.source.clone(), backup.target.clone()).await {
            rollback_commit(&applied).await;
            rollback_created_dirs(&created_dirs).await;
            return Err(error_io(format!("提交 CadQuery 文件失败: {error}")));
        }
        applied.push(backup);
    }
    Ok(())
}

async fn prepare_commit_files(
    workspace_root: PathBuf,
    files: Vec<CommitFile>,
) -> Result<CommitPlan, CadQueryRunnerError> {
    let mut backups = Vec::with_capacity(files.len());
    let mut created_dirs = Vec::new();
    for file in files {
        if !fs::metadata(file.source.clone())
            .await
            .is_ok_and(|metadata| metadata.is_file())
        {
            rollback_created_dirs(&created_dirs).await;
            return Err(error_io(format!(
                "CadQuery staging 文件不存在: {}",
                file.source.display()
            )));
        }
        let target = match resolve_commit_target(
            workspace_root.clone(),
            file.target.clone(),
            &mut created_dirs,
        )
        .await
        {
            Ok(target) => target,
            Err(error) => {
                rollback_created_dirs(&created_dirs).await;
                return Err(error);
            }
        };
        let previous_bytes = match capture_regular_file_backup(target.clone()).await {
            Ok(previous_bytes) => previous_bytes,
            Err(error) => {
                rollback_created_dirs(&created_dirs).await;
                return Err(error);
            }
        };
        backups.push(CommitBackup {
            source: file.source,
            target,
            previous_bytes,
        });
    }
    Ok(CommitPlan {
        backups,
        created_dirs,
    })
}

async fn resolve_commit_target(
    workspace_root: PathBuf,
    target: PathBuf,
    created_dirs: &mut Vec<PathBuf>,
) -> Result<PathBuf, CadQueryRunnerError> {
    let parent = target
        .parent()
        .ok_or_else(|| error_invalid_path("CadQuery 提交目标缺少父目录"))?;
    let parent = ensure_commit_parent_inside_workspace(
        workspace_root.clone(),
        parent.to_path_buf(),
        created_dirs,
    )
    .await?;
    let file_name = target
        .file_name()
        .ok_or_else(|| error_invalid_path("CadQuery 提交目标缺少文件名"))?;
    Ok(parent.join(file_name))
}

async fn ensure_commit_parent_inside_workspace(
    workspace_root: PathBuf,
    parent: PathBuf,
    created_dirs: &mut Vec<PathBuf>,
) -> Result<PathBuf, CadQueryRunnerError> {
    let canonical_root = fs::canonicalize(workspace_root.clone())
        .await
        .map_err(|error| error_io(format!("读取 workspace 真实路径失败: {error}")))?;
    let relative_parent = parent
        .strip_prefix(&workspace_root)
        .map_err(|_| error_invalid_path("CadQuery 提交目标不在 workspace 内"))?;
    let mut current = canonical_root.clone();
    for component in relative_parent.components() {
        let Component::Normal(segment) = component else {
            return Err(error_invalid_path("CadQuery 提交目标不能逃逸 workspace"));
        };
        let next = current.join(segment);
        current =
            ensure_commit_dir_inside_workspace(canonical_root.clone(), next, created_dirs).await?;
    }
    Ok(current)
}

async fn ensure_commit_dir_inside_workspace(
    canonical_root: PathBuf,
    path: PathBuf,
    created_dirs: &mut Vec<PathBuf>,
) -> Result<PathBuf, CadQueryRunnerError> {
    match fs::symlink_metadata(path.clone()).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(error_invalid_path("CadQuery 提交目录不能是符号链接"));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(error_invalid_path("CadQuery 提交目录不能是普通文件"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path.clone())
                .await
                .map_err(|error| error_io(format!("创建 CadQuery 提交目录失败: {error}")))?;
            created_dirs.push(path.clone());
        }
        Err(error) => return Err(error_io(format!("读取 CadQuery 提交目录失败: {error}"))),
    }
    let resolved = fs::canonicalize(path)
        .await
        .map_err(|error| error_io(format!("读取 CadQuery 提交目录真实路径失败: {error}")))?;
    if !resolved.starts_with(&canonical_root) {
        return Err(error_invalid_path(
            "CadQuery 提交目录真实路径不在 workspace 内",
        ));
    }
    Ok(resolved)
}

async fn capture_regular_file_backup(
    path: PathBuf,
) -> Result<Option<Vec<u8>>, CadQueryRunnerError> {
    match fs::symlink_metadata(path.clone()).await {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(error_invalid_path("CadQuery 提交目标不能是符号链接"))
        }
        Ok(metadata) if metadata.is_dir() => Err(error_invalid_path("CadQuery 提交目标不能是目录")),
        Ok(_) => fs::read(path)
            .await
            .map(Some)
            .map_err(|error| error_io(format!("读取 CadQuery 提交备份失败: {error}"))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error_io(format!("读取 CadQuery 提交目标失败: {error}"))),
    }
}

async fn rollback_commit(applied: &[CommitBackup]) {
    for backup in applied.iter().rev() {
        match &backup.previous_bytes {
            Some(bytes) => {
                let _ = atomic_write_file(backup.target.clone(), bytes.clone()).await;
            }
            None => {
                let _ = fs::remove_file(backup.target.clone()).await;
            }
        }
    }
}

async fn rollback_created_dirs(created_dirs: &[PathBuf]) {
    for dir in created_dirs.iter().rev() {
        let _ = fs::remove_dir(dir).await;
    }
}

async fn atomic_copy_file(source: PathBuf, target: PathBuf) -> std::io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "target has no parent")
        })?
        .to_path_buf();
    fs::create_dir_all(parent.clone()).await?;
    let temp = parent.join(format!(".{}.tmp-{}", temp_stem(&target), staging_id()));
    if let Err(error) = fs::copy(source, temp.clone()).await {
        let _ = fs::remove_file(temp.clone()).await;
        return Err(error);
    }
    if let Err(error) = fs::rename(temp.clone(), target).await {
        let _ = fs::remove_file(temp).await;
        return Err(error);
    }
    Ok(())
}

async fn atomic_write_file(target: PathBuf, bytes: Vec<u8>) -> std::io::Result<()> {
    let parent = target
        .parent()
        .ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "target has no parent")
        })?
        .to_path_buf();
    fs::create_dir_all(parent.clone()).await?;
    let temp = parent.join(format!(".{}.tmp-{}", temp_stem(&target), staging_id()));
    if let Err(error) = fs::write(temp.clone(), bytes).await {
        let _ = fs::remove_file(temp.clone()).await;
        return Err(error);
    }
    if let Err(error) = fs::rename(temp.clone(), target).await {
        let _ = fs::remove_file(temp).await;
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
