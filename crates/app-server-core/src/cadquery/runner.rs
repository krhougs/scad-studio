use std::{
    path::PathBuf,
    process::{Output, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use app_server_protocol::{CadQueryExportFormat, CadQueryMeshPayload};
use tokio::{
    fs,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    process::Command,
    task::JoinHandle,
    time,
};

use super::{cadquery_result_ready, parse_cadquery_success_json};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CadQueryRunnerErrorKind {
    Build,
    Cancelled,
    FileConflict,
    InvalidProjectPath,
    Io,
    PermissionDenied,
    PythonImport,
    Runner,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadQueryRunnerError {
    pub kind: CadQueryRunnerErrorKind,
    pub message: String,
}

impl std::fmt::Display for CadQueryRunnerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for CadQueryRunnerError {}

#[derive(Debug, Clone)]
pub struct CadQueryRunConfig {
    pub python: PathBuf,
    pub project_root: PathBuf,
    pub script: String,
    pub output_dir: PathBuf,
    pub export_formats: Vec<CadQueryExportFormat>,
    pub params_json: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct CadQueryContractConfig {
    pub python: PathBuf,
    pub code: String,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CadQueryRunResult {
    pub ready: app_server_protocol::CadQueryResultReady,
    pub mesh: CadQueryMeshPayload,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CadQueryContractResult {
    pub has_model_description: bool,
}

pub async fn run_cadquery_runner(
    config: CadQueryRunConfig,
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    run_cadquery_runner_with_cancel(config, &|| false).await
}

pub async fn run_cadquery_runner_with_cancel(
    config: CadQueryRunConfig,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    let mut command = Command::new(&config.python);
    command
        .args(runner_args(&config))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command
        .spawn()
        .map_err(|error| error_io(format!("启动 CadQuery runner 失败: {error}")))?;
    let output = wait_with_timeout(child, config.timeout, is_cancelled).await?;
    parse_runner_output(output)
}

pub async fn run_cadquery_contract(
    config: CadQueryContractConfig,
) -> Result<CadQueryContractResult, CadQueryRunnerError> {
    let contract_file = TemporaryContractFile::write(config.code.clone()).await?;
    let mut command = Command::new(&config.python);
    command
        .args(contract_args(contract_file.path()))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            contract_file.remove().await;
            return Err(error_io(format!(
                "启动 CadQuery contract runner 失败: {error}"
            )));
        }
    };
    let output = wait_with_timeout(child, config.timeout, &|| false).await;
    contract_file.remove().await;
    let output = output?;
    parse_contract_output(output)
}

fn runner_args(config: &CadQueryRunConfig) -> Vec<String> {
    vec![
        "-B".into(),
        "-m".into(),
        "budn_cad_runner".into(),
        "--script".into(),
        config.script.clone(),
        "--project-root".into(),
        config.project_root.to_string_lossy().into_owned(),
        "--output-dir".into(),
        config.output_dir.to_string_lossy().into_owned(),
        "--exports".into(),
        export_arg(&config.export_formats),
        "--params".into(),
        config.params_json.clone(),
    ]
}

fn contract_args(path: &std::path::Path) -> Vec<String> {
    vec![
        "-B".into(),
        "-m".into(),
        "budn_cad_runner".into(),
        "--contract-file".into(),
        path.to_string_lossy().into_owned(),
    ]
}

async fn wait_with_timeout(
    mut child: tokio::process::Child,
    timeout: Duration,
    is_cancelled: &(dyn Fn() -> bool + Sync),
) -> Result<Output, CadQueryRunnerError> {
    let stdout_task = read_child_stream(child.stdout.take());
    let stderr_task = read_child_stream(child.stderr.take());
    let timeout = time::sleep(timeout);
    tokio::pin!(timeout);
    let mut tick = time::interval(Duration::from_millis(10));
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| error_io(format!("等待 CadQuery runner 结束失败: {error}")))?
        {
            let stdout = read_child_output(stdout_task, "stdout").await?;
            let stderr = read_child_output(stderr_task, "stderr").await?;
            return Ok(Output {
                status,
                stdout,
                stderr,
            });
        }
        tokio::select! {
            _ = &mut timeout => {
                terminate_child(child, stdout_task, stderr_task).await;
                return Err(CadQueryRunnerError {
                    kind: CadQueryRunnerErrorKind::Timeout,
                    message: "CadQuery runner 执行超时".into(),
                });
            }
            _ = tick.tick() => {
                if is_cancelled() {
                    terminate_child(child, stdout_task, stderr_task).await;
                return Err(CadQueryRunnerError {
                    kind: CadQueryRunnerErrorKind::Cancelled,
                    message: "CadQuery runner 已取消".into(),
                });
                }
            }
        }
    }
}

fn read_child_stream<R>(stream: Option<R>) -> JoinHandle<std::io::Result<Vec<u8>>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            stream.read_to_end(&mut bytes).await?;
        }
        Ok(bytes)
    })
}

async fn read_child_output(
    task: JoinHandle<std::io::Result<Vec<u8>>>,
    label: &'static str,
) -> Result<Vec<u8>, CadQueryRunnerError> {
    match task.await {
        Ok(Ok(bytes)) => Ok(bytes),
        Ok(Err(error)) => Err(error_io(format!(
            "读取 CadQuery runner {label} 失败: {error}"
        ))),
        Err(error) => Err(error_io(format!(
            "等待 CadQuery runner {label} 读取任务失败: {error}"
        ))),
    }
}

async fn terminate_child(
    mut child: tokio::process::Child,
    stdout_task: JoinHandle<std::io::Result<Vec<u8>>>,
    stderr_task: JoinHandle<std::io::Result<Vec<u8>>>,
) {
    let _ = child.start_kill();
    let _ = child.wait().await;
    let _ = read_child_output(stdout_task, "stdout").await;
    let _ = read_child_output(stderr_task, "stderr").await;
}

fn parse_runner_output(output: Output) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let parsed = parse_structured_runner_error(&stdout);
        return Err(CadQueryRunnerError {
            kind: runner_error_kind(output.status.code(), parsed.as_ref()),
            message: runner_error_message(parsed.as_ref(), &stderr),
        });
    }
    let mesh = parse_cadquery_success_json(&stdout).map_err(|error| CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::Runner,
        message: error.message,
    })?;
    Ok(CadQueryRunResult {
        ready: cadquery_result_ready(&mesh),
        mesh,
        stderr,
    })
}

fn parse_contract_output(output: Output) -> Result<CadQueryContractResult, CadQueryRunnerError> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        let parsed = parse_structured_runner_error(&stdout);
        return Err(CadQueryRunnerError {
            kind: runner_error_kind(output.status.code(), parsed.as_ref()),
            message: runner_error_message(parsed.as_ref(), &stderr),
        });
    }
    let value = serde_json::from_str::<serde_json::Value>(&stdout).map_err(|error| {
        CadQueryRunnerError {
            kind: CadQueryRunnerErrorKind::Runner,
            message: format!("解析 CadQuery contract runner 输出失败: {error}"),
        }
    })?;
    let has_model_description = value
        .get("contract")
        .and_then(|contract| contract.get("has_model_description"))
        .and_then(|value| value.as_bool())
        .ok_or_else(|| CadQueryRunnerError {
            kind: CadQueryRunnerErrorKind::Runner,
            message: "CadQuery contract runner 输出缺少 contract.has_model_description".into(),
        })?;
    Ok(CadQueryContractResult {
        has_model_description,
    })
}

fn runner_error_message(parsed: Option<&StructuredRunnerError>, stderr: &str) -> String {
    let stderr = stderr.trim();
    match (parsed, stderr.is_empty()) {
        (Some(error), true) => error.message(),
        (Some(error), false) => format!("{}\n{stderr}", error.message()),
        (None, false) => stderr.to_owned(),
        (None, true) => String::new(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StructuredRunnerError {
    status: String,
    error_type: String,
    error: String,
}

impl StructuredRunnerError {
    fn message(&self) -> String {
        format!("{}:{}:{}", self.status, self.error_type, self.error)
    }
}

fn parse_structured_runner_error(stdout: &str) -> Option<StructuredRunnerError> {
    serde_json::from_str::<serde_json::Value>(stdout)
        .ok()
        .and_then(|value| structured_runner_error(&value))
}

fn structured_runner_error(value: &serde_json::Value) -> Option<StructuredRunnerError> {
    let status = value.get("status")?.as_str()?;
    let error_type = value
        .get("error_type")
        .and_then(|value| value.as_str())
        .unwrap_or("RunnerError");
    let error = value
        .get("error")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    Some(StructuredRunnerError {
        status: status.into(),
        error_type: error_type.into(),
        error: error.into(),
    })
}

fn export_arg(formats: &[CadQueryExportFormat]) -> String {
    formats
        .iter()
        .map(|format| match format {
            CadQueryExportFormat::Step => "step",
            CadQueryExportFormat::Stl => "stl",
            CadQueryExportFormat::ThreeMf => "3mf",
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn runner_error_kind(
    code: Option<i32>,
    parsed: Option<&StructuredRunnerError>,
) -> CadQueryRunnerErrorKind {
    if parsed.is_some_and(is_python_import_error) {
        return CadQueryRunnerErrorKind::PythonImport;
    }
    status_error_kind(code)
}

fn status_error_kind(code: Option<i32>) -> CadQueryRunnerErrorKind {
    match code {
        Some(1) => CadQueryRunnerErrorKind::Build,
        _ => CadQueryRunnerErrorKind::Runner,
    }
}

fn is_python_import_error(error: &StructuredRunnerError) -> bool {
    matches!(
        error.error_type.as_str(),
        "ImportError" | "ModuleNotFoundError"
    ) || error.error.contains("No module named")
        || error.error.contains("cannot import name")
}

pub(super) fn error_io(message: impl Into<String>) -> CadQueryRunnerError {
    CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::Io,
        message: message.into(),
    }
}

pub(super) fn error_invalid_path(message: impl Into<String>) -> CadQueryRunnerError {
    CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::InvalidProjectPath,
        message: message.into(),
    }
}

pub(super) fn error_permission_denied(message: impl Into<String>) -> CadQueryRunnerError {
    CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::PermissionDenied,
        message: message.into(),
    }
}

struct TemporaryContractFile {
    path: PathBuf,
}

impl TemporaryContractFile {
    async fn write(code: String) -> Result<Self, CadQueryRunnerError> {
        for attempt in 0..16 {
            let path = std::env::temp_dir().join(format!(
                "budn-cq-contract-{}-{}-{attempt}.py",
                std::process::id(),
                unique_temp_suffix()
            ));
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                options.mode(0o600);
            }
            match options.open(path.clone()).await {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(code.as_bytes()).await {
                        let _ = fs::remove_file(path.clone()).await;
                        return Err(error_io(format!(
                            "写入 CadQuery contract 临时文件失败: {error}"
                        )));
                    }
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error_io(format!(
                        "创建 CadQuery contract 临时文件失败: {error}"
                    )));
                }
            }
        }
        Err(error_io("创建 CadQuery contract 临时文件失败: 文件名冲突"))
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }

    async fn remove(self) {
        let _ = fs::remove_file(self.path).await;
    }
}

fn unique_temp_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}
