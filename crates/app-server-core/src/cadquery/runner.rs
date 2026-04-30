use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use app_server_protocol::{CadQueryExportFormat, CadQueryMeshPayload};

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

pub fn run_cadquery_runner(
    config: &CadQueryRunConfig,
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    run_cadquery_runner_with_cancel(config, &|| false)
}

pub fn run_cadquery_runner_with_cancel(
    config: &CadQueryRunConfig,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    let child = Command::new(&config.python)
        .args(runner_args(config))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error_io(format!("启动 CadQuery runner 失败: {error}")))?;
    let output = wait_with_timeout(child, config.timeout, is_cancelled)?;
    parse_runner_output(output)
}

pub fn run_cadquery_contract(
    config: &CadQueryContractConfig,
) -> Result<CadQueryContractResult, CadQueryRunnerError> {
    let contract_file = TemporaryContractFile::write(&config.code)?;
    let child = Command::new(&config.python)
        .args(contract_args(contract_file.path()))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error_io(format!("启动 CadQuery contract runner 失败: {error}")))?;
    let output = wait_with_timeout(child, config.timeout, &|| false)?;
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

fn wait_with_timeout(
    mut child: std::process::Child,
    timeout: Duration,
    is_cancelled: &dyn Fn() -> bool,
) -> Result<Output, CadQueryRunnerError> {
    let stdout_reader = pipe_reader(
        child
            .stdout
            .take()
            .ok_or_else(|| error_io("CadQuery runner stdout pipe was not available"))?,
        "stdout",
    );
    let stderr_reader = pipe_reader(
        child
            .stderr
            .take()
            .ok_or_else(|| error_io("CadQuery runner stderr pipe was not available"))?,
        "stderr",
    );
    let started = Instant::now();
    while started.elapsed() < timeout {
        if is_cancelled() {
            terminate_child(child);
            let _ = join_pipe_reader(stdout_reader, "stdout");
            let _ = join_pipe_reader(stderr_reader, "stderr");
            return Err(CadQueryRunnerError {
                kind: CadQueryRunnerErrorKind::Cancelled,
                message: "CadQuery runner 已取消".into(),
            });
        }
        if let Some(status) = child
            .try_wait()
            .map_err(|error| error_io(format!("轮询 CadQuery runner 失败: {error}")))?
        {
            return Ok(Output {
                status,
                stdout: join_pipe_reader(stdout_reader, "stdout")?,
                stderr: join_pipe_reader(stderr_reader, "stderr")?,
            });
        }
        thread::sleep(Duration::from_millis(10));
    }
    terminate_child(child);
    let _ = join_pipe_reader(stdout_reader, "stdout");
    let _ = join_pipe_reader(stderr_reader, "stderr");
    Err(CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::Timeout,
        message: "CadQuery runner 执行超时".into(),
    })
}

fn pipe_reader<R>(mut reader: R, label: &'static str) -> JoinHandle<std::io::Result<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    thread::Builder::new()
        .name(format!("cadquery-runner-{label}-reader"))
        .spawn(move || {
            let mut bytes = Vec::new();
            reader.read_to_end(&mut bytes)?;
            Ok(bytes)
        })
        .expect("spawn CadQuery runner pipe reader")
}

fn join_pipe_reader(
    handle: JoinHandle<std::io::Result<Vec<u8>>>,
    label: &str,
) -> Result<Vec<u8>, CadQueryRunnerError> {
    let result = handle
        .join()
        .map_err(|_| error_io(format!("CadQuery runner {label} reader panicked")))?;
    result.map_err(|error| error_io(format!("读取 CadQuery runner {label} 失败: {error}")))
}

fn terminate_child(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
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
    fn write(code: &str) -> Result<Self, CadQueryRunnerError> {
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
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(code.as_bytes()) {
                        let _ = fs::remove_file(&path);
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
}

impl Drop for TemporaryContractFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn unique_temp_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}
