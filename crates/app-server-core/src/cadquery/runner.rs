use std::{
    io::Read,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
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

#[derive(Debug, Clone, PartialEq)]
pub struct CadQueryRunResult {
    pub ready: app_server_protocol::CadQueryResultReady,
    pub mesh: CadQueryMeshPayload,
    pub stderr: String,
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
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| error_io(format!("启动 CadQuery runner 失败: {error}")))?;
    let output = wait_with_timeout(child, config.timeout, is_cancelled)?;
    parse_runner_output(output)
}

fn runner_args(config: &CadQueryRunConfig) -> Vec<String> {
    vec![
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
