use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    thread,
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
    let started = Instant::now();
    while started.elapsed() < timeout {
        if is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CadQueryRunnerError {
                kind: CadQueryRunnerErrorKind::Cancelled,
                message: "CadQuery runner 已取消".into(),
            });
        }
        if child
            .try_wait()
            .map_err(|error| error_io(format!("轮询 CadQuery runner 失败: {error}")))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|error| error_io(format!("读取 CadQuery runner 输出失败: {error}")));
        }
        thread::sleep(Duration::from_millis(10));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(CadQueryRunnerError {
        kind: CadQueryRunnerErrorKind::Timeout,
        message: "CadQuery runner 执行超时".into(),
    })
}

fn parse_runner_output(output: Output) -> Result<CadQueryRunResult, CadQueryRunnerError> {
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if !output.status.success() {
        return Err(CadQueryRunnerError {
            kind: status_error_kind(output.status.code()),
            message: runner_error_message(&stdout, &stderr),
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

fn runner_error_message(stdout: &str, stderr: &str) -> String {
    let stderr = stderr.trim();
    let parsed = serde_json::from_str::<serde_json::Value>(stdout)
        .ok()
        .and_then(|value| structured_runner_error(&value));
    match (parsed, stderr.is_empty()) {
        (Some(message), true) => message,
        (Some(message), false) => format!("{message}\n{stderr}"),
        (None, false) => stderr.to_owned(),
        (None, true) => String::new(),
    }
}

fn structured_runner_error(value: &serde_json::Value) -> Option<String> {
    let status = value.get("status")?.as_str()?;
    let error_type = value
        .get("error_type")
        .and_then(|value| value.as_str())
        .unwrap_or("RunnerError");
    let error = value
        .get("error")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    Some(format!("{status}:{error_type}:{error}"))
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

fn status_error_kind(code: Option<i32>) -> CadQueryRunnerErrorKind {
    match code {
        Some(1) => CadQueryRunnerErrorKind::Build,
        _ => CadQueryRunnerErrorKind::Runner,
    }
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
