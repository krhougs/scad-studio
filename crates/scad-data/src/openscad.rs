use std::{
    env, fmt, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

use scad_scene::{MeshData, three_mf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RenderedArtifact {
    pub source_path: PathBuf,
    pub mesh: MeshData,
}

#[derive(Debug, Clone)]
pub enum OpenScadMessage {
    Started(PathBuf),
    Log(LogEntry),
    Finished(Result<RenderedArtifact, OpenScadError>),
}

#[derive(Debug, Clone)]
pub struct OpenScadError(String);

impl OpenScadError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

pub struct OpenScadRunner {
    tx: Sender<RunnerCommand>,
}

enum RunnerCommand {
    Render(RenderRequest),
}

struct RunningJob {
    request: RenderRequest,
    preview_path: PathBuf,
    child: Child,
}

struct JobCompletion {
    logs: Vec<LogEntry>,
    result: Result<RenderedArtifact, OpenScadError>,
}

#[derive(Debug, Clone)]
struct RenderRequest {
    source_path: PathBuf,
    defines: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOutputFormat {
    BinaryStl,
    ThreeMf,
}

impl OpenScadRunner {
    pub fn new<F>(notify: F) -> Self
    where
        F: Fn(OpenScadMessage) + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || worker_loop(rx, notify));
        Self { tx }
    }

    #[allow(dead_code)]
    pub fn render(&self, source_path: PathBuf) {
        self.render_with_defines(source_path, Vec::new());
    }

    pub fn render_with_defines(&self, source_path: PathBuf, defines: Vec<String>) {
        let _ = self.tx.send(RunnerCommand::Render(RenderRequest {
            source_path,
            defines,
        }));
    }
}

fn worker_loop<F>(rx: Receiver<RunnerCommand>, notify: F)
where
    F: Fn(OpenScadMessage) + Send + 'static,
{
    let mut active_job: Option<RunningJob> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(RunnerCommand::Render(request)) => {
                cancel_job(&mut active_job);
                notify(OpenScadMessage::Started(request.source_path.clone()));
                active_job = start_job(request, &notify);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if let Some(completion) = poll_job(&mut active_job) {
            for entry in completion.logs {
                notify(OpenScadMessage::Log(entry));
            }
            notify(OpenScadMessage::Finished(completion.result));
        }
    }
    cancel_job(&mut active_job);
}

fn start_job<F>(request: RenderRequest, notify: &F) -> Option<RunningJob>
where
    F: Fn(OpenScadMessage),
{
    match build_job(&request) {
        Ok(job) => Some(job),
        Err(error) => {
            notify(OpenScadMessage::Finished(Err(error)));
            None
        }
    }
}

fn build_job(request: &RenderRequest) -> Result<RunningJob, OpenScadError> {
    let executable = detect_openscad_path(None).map_err(|_| {
        OpenScadError::new(
            "未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH；3MF 彩色预览需要可用的 OpenSCAD CLI",
        )
    })?;
    let (preview_path, args) = build_preview_job_args(&request.source_path, &request.defines);
    let child = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OpenScadError::new(format!("启动 OpenSCAD CLI 失败: {error}")))?;
    Ok(RunningJob {
        request: request.clone(),
        preview_path,
        child,
    })
}

fn poll_job(job: &mut Option<RunningJob>) -> Option<JobCompletion> {
    let status = job.as_mut()?.child.try_wait().ok()??;
    let RunningJob {
        request,
        preview_path,
        child,
    } = job.take()?;
    let output = child
        .wait_with_output()
        .map_err(|error| OpenScadError::new(format!("等待 OpenSCAD CLI 结束失败: {error}")));
    let logs = output
        .as_ref()
        .map(|output| collect_process_logs(&output.stdout, &output.stderr, status.success()))
        .unwrap_or_default();
    let result = finalize_job(request.source_path, preview_path, status.success(), output);
    Some(JobCompletion { logs, result })
}

pub fn finalize_job(
    source_path: PathBuf,
    preview_path: PathBuf,
    success: bool,
    output: Result<std::process::Output, OpenScadError>,
) -> Result<RenderedArtifact, OpenScadError> {
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            remove_preview_file(&preview_path);
            return Err(error);
        }
    };
    if !success {
        remove_preview_file(&preview_path);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            "OpenSCAD 3MF 预览失败：CLI 返回非零状态，当前环境可能不支持 3MF 导出".to_owned()
        } else {
            format!("OpenSCAD 3MF 预览失败: {stderr}")
        };
        return Err(OpenScadError::new(message));
    }
    if !preview_path.is_file() {
        return Err(OpenScadError::new(
            "OpenSCAD 3MF 预览失败：CLI 未生成可解析的 3MF 输出文件",
        ));
    }
    let mesh = three_mf::load_3mf(&preview_path)
        .map_err(|error| OpenScadError::new(format!("解析 OpenSCAD 3MF 预览失败: {error}")));
    remove_preview_file(&preview_path);
    let mesh = mesh?;
    Ok(RenderedArtifact { source_path, mesh })
}

fn cancel_job(job: &mut Option<RunningJob>) {
    if let Some(active_job) = job.as_mut() {
        let _ = active_job.child.kill();
        let _ = active_job.child.wait();
        remove_preview_file(&active_job.preview_path);
    }
    *job = None;
}

pub fn collect_process_logs(stdout: &[u8], stderr: &[u8], success: bool) -> Vec<LogEntry> {
    let mut logs = Vec::new();
    extend_logs(&mut logs, stdout, LogLevel::Info);
    extend_logs(
        &mut logs,
        stderr,
        if success {
            LogLevel::Warning
        } else {
            LogLevel::Error
        },
    );
    logs
}

fn extend_logs(entries: &mut Vec<LogEntry>, bytes: &[u8], level: LogLevel) {
    for line in String::from_utf8_lossy(bytes).lines() {
        let message = line.trim();
        if message.is_empty() {
            continue;
        }
        entries.push(LogEntry {
            level,
            message: message.to_owned(),
        });
    }
}

pub fn detect_openscad_path(configured_path: Option<PathBuf>) -> Result<PathBuf, OpenScadError> {
    let env_path = env::var("OPENSCAD_PATH").ok().map(PathBuf::from);
    resolve_openscad_path(
        configured_path,
        env_path,
        find_in_path().or_else(find_platform_path),
    )
}

fn find_in_path() -> Option<PathBuf> {
    let binary = if cfg!(target_os = "windows") {
        "openscad.exe"
    } else {
        "openscad"
    };
    env::var_os("PATH").and_then(|value| {
        env::split_paths(&value)
            .map(|dir| dir.join(binary))
            .find(|candidate| candidate.is_file())
    })
}

fn find_platform_path() -> Option<PathBuf> {
    let candidates = if cfg!(target_os = "macos") {
        vec![
            "/Applications/OpenSCAD.app/Contents/MacOS/OpenSCAD",
            "/Applications/OpenSCAD-nightly.app/Contents/MacOS/OpenSCAD",
        ]
    } else if cfg!(target_os = "windows") {
        vec![
            "C:\\Program Files\\OpenSCAD (Nightly)\\openscad.exe",
            "C:\\Program Files\\OpenSCAD\\openscad.exe",
        ]
    } else {
        vec!["/usr/bin/openscad", "/usr/local/bin/openscad"]
    };
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

fn temp_preview_path(source_path: &Path) -> PathBuf {
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("preview");
    let filename = format!("scad-studio-{stem}-{}.3mf", std::process::id());
    env::temp_dir().join(filename)
}

pub fn build_preview_job_args(source_path: &Path, defines: &[String]) -> (PathBuf, Vec<String>) {
    let output_path = temp_preview_path(source_path);
    let args = build_cli_args(CliOutputFormat::ThreeMf, &output_path, defines, source_path);
    (output_path, args)
}

pub fn build_cli_args(
    format: CliOutputFormat,
    output_path: &Path,
    defines: &[String],
    source_path: &Path,
) -> Vec<String> {
    let mut args = vec![
        "--export-format".to_string(),
        format.as_arg().to_string(),
        "-o".to_string(),
        output_path.display().to_string(),
    ];
    for define in defines {
        args.push("-D".to_string());
        args.push(define.clone());
    }
    args.push(source_path.display().to_string());
    args
}

pub fn resolve_openscad_path(
    configured_path: Option<PathBuf>,
    env_path: Option<PathBuf>,
    auto_path: Option<PathBuf>,
) -> Result<PathBuf, OpenScadError> {
    configured_path
        .or(env_path)
        .or(auto_path)
        .ok_or_else(|| OpenScadError::new("未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH"))
}

impl CliOutputFormat {
    fn as_arg(self) -> &'static str {
        match self {
            Self::BinaryStl => "binstl",
            Self::ThreeMf => "3mf",
        }
    }
}

impl std::error::Error for OpenScadError {}

impl fmt::Display for OpenScadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn remove_preview_file(path: &Path) {
    if let Err(error) = fs::remove_file(path)
        && error.kind() != ErrorKind::NotFound
    {
        log::warn!("清理临时 3MF 预览文件失败: {error}");
    }
}
