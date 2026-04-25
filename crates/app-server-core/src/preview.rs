use crate::terminate_child;
use app_server_protocol::{
    PreviewArtifact, PreviewArtifact3mf, PreviewArtifactStl, PreviewReadyResponse,
    PreviewRequestKind,
};
use scad_scene::three_mf;
use std::{
    env, fmt, fs,
    io::Cursor,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

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
    pub bytes: Vec<u8>,
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

    pub fn render_with_defines(&self, source_path: PathBuf, defines: Vec<String>) {
        let _ = self.tx.send(RunnerCommand::Render(RenderRequest {
            source_path,
            defines,
        }));
    }
}

pub fn preview_ready_response(
    configured_openscad_path: Option<PathBuf>,
    source_path: &Path,
    defines: &[String],
) -> Result<PreviewReadyResponse, OpenScadError> {
    let artifact = preview_artifact(configured_openscad_path, source_path, defines)?;
    Ok(PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact,
    })
}

pub fn preview_artifact(
    configured_openscad_path: Option<PathBuf>,
    source_path: &Path,
    defines: &[String],
) -> Result<PreviewArtifact, OpenScadError> {
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "stl" => read_preview_bytes(source_path).map(|bytes| {
            PreviewArtifact::Stl(PreviewArtifactStl {
                bytes,
                media_type: "model/stl".into(),
            })
        }),
        "3mf" => read_preview_bytes(source_path).map(|bytes| {
            PreviewArtifact::ThreeMf(PreviewArtifact3mf {
                bytes,
                media_type: "model/3mf".into(),
            })
        }),
        "scad" => render_scad_preview(configured_openscad_path, source_path, defines),
        _ => Err(OpenScadError::new("暂不支持的预览文件类型")),
    }
}

fn read_preview_bytes(path: &Path) -> Result<Vec<u8>, OpenScadError> {
    fs::read(path).map_err(|error| OpenScadError::new(format!("读取预览文件失败: {error}")))
}

fn render_scad_preview(
    configured_openscad_path: Option<PathBuf>,
    source_path: &Path,
    defines: &[String],
) -> Result<PreviewArtifact, OpenScadError> {
    let executable = detect_openscad_path(configured_openscad_path)?;
    let (preview_path, args) = build_preview_job_args(source_path, defines);
    let output = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|error| OpenScadError::new(format!("启动 OpenSCAD CLI 失败: {error}")))?;
    let artifact = finalize_job(
        source_path.to_path_buf(),
        preview_path,
        output.status.success(),
        Ok(output),
    )?;
    Ok(PreviewArtifact::ThreeMf(PreviewArtifact3mf {
        bytes: artifact.bytes,
        media_type: "model/3mf".into(),
    }))
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
    let executable = detect_openscad_path(None)?;
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
    let bytes = fs::read(&preview_path)
        .map_err(|error| OpenScadError::new(format!("读取 OpenSCAD 3MF 预览失败: {error}")));
    remove_preview_file(&preview_path);
    let bytes = bytes?;
    let mut cursor = Cursor::new(&bytes);
    three_mf::load_3mf_from_reader(&mut cursor)
        .map_err(|error| OpenScadError::new(format!("解析 OpenSCAD 3MF 预览失败: {error}")))?;
    Ok(RenderedArtifact { source_path, bytes })
}

fn cancel_job(job: &mut Option<RunningJob>) {
    if let Some(active_job) = job.as_mut() {
        terminate_child(&mut active_job.child);
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
    let env_path = env::var_os("OPENSCAD_PATH").map(PathBuf::from);
    resolve_openscad_path(
        configured_path,
        env_path,
        find_in_path().or_else(find_platform_path),
    )
}

fn find_in_path() -> Option<PathBuf> {
    find_command_in_path(default_openscad_command_names())
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
        .into_iter()
        .flat_map(expand_configured_openscad_candidate)
        .chain(
            env_path
                .into_iter()
                .flat_map(expand_configured_openscad_candidate),
        )
        .chain(auto_path)
        .find(|candidate| is_usable_openscad_candidate(candidate))
        .ok_or_else(|| OpenScadError::new(openscad_not_found_message()))
}

fn openscad_not_found_message() -> &'static str {
    "未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH 或在 Settings 中配置 OpenSCAD 路径"
}

fn expand_configured_openscad_candidate(path: PathBuf) -> Vec<PathBuf> {
    let mut candidates = if is_bare_command_name(&path) {
        Vec::new()
    } else {
        vec![path.clone()]
    };
    if is_macos_app_bundle_path(&path) {
        candidates.push(path.join("Contents/MacOS/OpenSCAD"));
    }
    if is_bare_command_name(&path) {
        candidates.extend(find_command_in_path(command_names_for_configured_path(
            &path,
        )));
    }
    candidates
}

fn is_usable_openscad_candidate(path: &Path) -> bool {
    path.is_file()
}

fn is_macos_app_bundle_path(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("app"))
}

fn is_bare_command_name(path: &Path) -> bool {
    let mut components = path.components();
    matches!(components.next(), Some(std::path::Component::Normal(_)))
        && components.next().is_none()
}

fn find_command_in_path(names: Vec<&'static str>) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|value| {
        env::split_paths(&value)
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|candidate| candidate.is_file())
    })
}

fn default_openscad_command_names() -> Vec<&'static str> {
    if cfg!(target_os = "windows") {
        vec!["openscad.exe", "openscad"]
    } else {
        vec!["openscad"]
    }
}

fn command_names_for_configured_path(path: &Path) -> Vec<&'static str> {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if name.eq_ignore_ascii_case("openscad.exe") {
        vec!["openscad.exe"]
    } else if name.eq_ignore_ascii_case("openscad") {
        default_openscad_command_names()
    } else {
        Vec::new()
    }
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
