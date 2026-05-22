use app_server_protocol::{
    PreviewArtifact, PreviewArtifact3mf, PreviewArtifactStl, PreviewReadyResponse,
    PreviewRequestKind,
};
use scad_scene::three_mf;
use std::{
    env, fmt,
    io::Cursor,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::{fs, process::Command};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliOutputFormat {
    BinaryStl,
    ThreeMf,
}

pub async fn preview_ready_response(
    configured_openscad_path: Option<PathBuf>,
    source_path: PathBuf,
    defines: Vec<String>,
) -> Result<PreviewReadyResponse, OpenScadError> {
    let artifact = preview_artifact(configured_openscad_path, source_path, defines).await?;
    Ok(PreviewReadyResponse {
        requested_kind: PreviewRequestKind::GeometryArtifact,
        artifact,
    })
}

pub async fn preview_artifact(
    configured_openscad_path: Option<PathBuf>,
    source_path: PathBuf,
    defines: Vec<String>,
) -> Result<PreviewArtifact, OpenScadError> {
    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "stl" => read_preview_bytes(source_path).await.map(|bytes| {
            PreviewArtifact::Stl(PreviewArtifactStl {
                bytes,
                media_type: "model/stl".into(),
            })
        }),
        "3mf" => read_preview_bytes(source_path).await.map(|bytes| {
            PreviewArtifact::ThreeMf(PreviewArtifact3mf {
                bytes,
                media_type: "model/3mf".into(),
            })
        }),
        "scad" => render_scad_preview(configured_openscad_path, source_path, defines).await,
        _ => Err(OpenScadError::new("暂不支持的预览文件类型")),
    }
}

async fn read_preview_bytes(path: PathBuf) -> Result<Vec<u8>, OpenScadError> {
    fs::read(path)
        .await
        .map_err(|error| OpenScadError::new(format!("读取预览文件失败: {error}")))
}

async fn render_scad_preview(
    configured_openscad_path: Option<PathBuf>,
    source_path: PathBuf,
    defines: Vec<String>,
) -> Result<PreviewArtifact, OpenScadError> {
    let executable = detect_openscad_path(configured_openscad_path).await?;
    let (preview_path, args) = build_preview_job_args(&source_path, &defines);
    let output = Command::new(executable)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|error| OpenScadError::new(format!("启动 OpenSCAD CLI 失败: {error}")))?;
    let artifact = finalize_job(
        source_path,
        preview_path,
        output.status.success(),
        Ok(output),
    )
    .await?;
    Ok(PreviewArtifact::ThreeMf(PreviewArtifact3mf {
        bytes: artifact.bytes,
        media_type: "model/3mf".into(),
    }))
}

pub async fn finalize_job(
    source_path: PathBuf,
    preview_path: PathBuf,
    success: bool,
    output: Result<std::process::Output, OpenScadError>,
) -> Result<RenderedArtifact, OpenScadError> {
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            remove_preview_file(preview_path.clone()).await;
            return Err(error);
        }
    };
    if !success {
        remove_preview_file(preview_path.clone()).await;
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            "OpenSCAD 3MF 预览失败：CLI 返回非零状态，当前环境可能不支持 3MF 导出".to_owned()
        } else {
            format!("OpenSCAD 3MF 预览失败: {stderr}")
        };
        return Err(OpenScadError::new(message));
    }
    if !fs::try_exists(preview_path.clone()).await.unwrap_or(false) {
        return Err(OpenScadError::new(
            "OpenSCAD 3MF 预览失败：CLI 未生成可解析的 3MF 输出文件",
        ));
    }
    let bytes = fs::read(preview_path.clone())
        .await
        .map_err(|error| OpenScadError::new(format!("读取 OpenSCAD 3MF 预览失败: {error}")));
    remove_preview_file(preview_path).await;
    let bytes = bytes?;
    let mut cursor = Cursor::new(&bytes);
    three_mf::load_3mf_from_reader(&mut cursor)
        .map_err(|error| OpenScadError::new(format!("解析 OpenSCAD 3MF 预览失败: {error}")))?;
    Ok(RenderedArtifact { source_path, bytes })
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

pub async fn detect_openscad_path(
    configured_path: Option<PathBuf>,
) -> Result<PathBuf, OpenScadError> {
    let env_path = env::var_os("OPENSCAD_PATH").map(PathBuf::from);
    match resolve_openscad_path(configured_path, env_path, find_in_path().await).await {
        Ok(path) => Ok(path),
        Err(_) => resolve_openscad_path(None, None, find_platform_path().await).await,
    }
}

async fn find_in_path() -> Option<PathBuf> {
    find_command_in_path(default_openscad_command_names()).await
}

async fn find_platform_path() -> Option<PathBuf> {
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
    let candidates = candidates
        .into_iter()
        .map(|path| PathBuf::from(path.to_owned()))
        .collect::<Vec<_>>();
    first_existing_file(candidates).await
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

pub async fn resolve_openscad_path(
    configured_path: Option<PathBuf>,
    env_path: Option<PathBuf>,
    auto_path: Option<PathBuf>,
) -> Result<PathBuf, OpenScadError> {
    if let Some(path) = configured_path {
        for candidate in expand_configured_openscad_candidate(path).await {
            if is_usable_openscad_candidate(candidate.clone()).await {
                return Ok(candidate);
            }
        }
    }
    if let Some(path) = env_path {
        for candidate in expand_configured_openscad_candidate(path).await {
            if is_usable_openscad_candidate(candidate.clone()).await {
                return Ok(candidate);
            }
        }
    }
    if let Some(candidate) = auto_path
        && is_usable_openscad_candidate(candidate.clone()).await
    {
        return Ok(candidate);
    }
    Err(OpenScadError::new(openscad_not_found_message()))
}

fn openscad_not_found_message() -> &'static str {
    "未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH 或在 Settings 中配置 OpenSCAD 路径"
}

async fn expand_configured_openscad_candidate(path: PathBuf) -> Vec<PathBuf> {
    let mut candidates = if is_bare_command_name(&path) {
        Vec::new()
    } else {
        vec![path.clone()]
    };
    if is_macos_app_bundle_path(&path) {
        candidates.push(path.join("Contents/MacOS/OpenSCAD"));
    }
    if is_bare_command_name(&path) {
        candidates.extend(
            find_command_in_path(command_names_for_configured_path(&path))
                .await
                .into_iter(),
        );
    }
    candidates
}

async fn is_usable_openscad_candidate(path: PathBuf) -> bool {
    tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file())
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

async fn find_command_in_path(names: Vec<&'static str>) -> Option<PathBuf> {
    let value = env::var_os("PATH")?;
    let paths = env::split_paths(&value)
        .flat_map(|dir| names.clone().into_iter().map(move |name| dir.join(name)))
        .collect::<Vec<_>>();
    first_existing_file(paths).await
}

async fn first_existing_file(paths: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    for path in paths {
        if is_usable_openscad_candidate(path.clone()).await {
            return Some(path);
        }
    }
    None
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

async fn remove_preview_file(path: PathBuf) {
    if let Err(error) = fs::remove_file(path).await
        && error.kind() != ErrorKind::NotFound
    {
        log::warn!("清理临时 3MF 预览文件失败: {error}");
    }
}
