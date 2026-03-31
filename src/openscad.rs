use std::{
    env,
    fmt,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::Duration,
};

use crate::mesh::{self, MeshData};

#[derive(Debug, Clone)]
pub struct RenderedArtifact {
    pub source_path: PathBuf,
    pub mesh: MeshData,
}

#[derive(Debug, Clone)]
pub enum OpenScadMessage {
    Started(PathBuf),
    Finished(Result<RenderedArtifact, OpenScadError>),
}

#[derive(Debug, Clone)]
pub struct OpenScadError(String);

pub struct OpenScadRunner {
    tx: Sender<RunnerCommand>,
}

enum RunnerCommand {
    Render(PathBuf),
}

struct RunningJob {
    source_path: PathBuf,
    stl_path: PathBuf,
    child: Child,
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

    pub fn render(&self, source_path: PathBuf) {
        let _ = self.tx.send(RunnerCommand::Render(source_path));
    }
}

fn worker_loop<F>(rx: Receiver<RunnerCommand>, notify: F)
where
    F: Fn(OpenScadMessage) + Send + 'static,
{
    let mut active_job: Option<RunningJob> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(RunnerCommand::Render(source_path)) => {
                cancel_job(&mut active_job);
                notify(OpenScadMessage::Started(source_path.clone()));
                active_job = start_job(source_path, &notify);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        if let Some(result) = poll_job(&mut active_job) {
            notify(OpenScadMessage::Finished(result));
        }
    }
    cancel_job(&mut active_job);
}

fn start_job<F>(source_path: PathBuf, notify: &F) -> Option<RunningJob>
where
    F: Fn(OpenScadMessage),
{
    match build_job(&source_path) {
        Ok(job) => Some(job),
        Err(error) => {
            notify(OpenScadMessage::Finished(Err(error)));
            None
        }
    }
}

fn build_job(source_path: &Path) -> Result<RunningJob, OpenScadError> {
    let executable = detect_openscad_path()?;
    let stl_path = temp_stl_path(source_path);
    let child = Command::new(executable)
        .arg("--export-format")
        .arg("binstl")
        .arg("-o")
        .arg(&stl_path)
        .arg(source_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| OpenScadError(format!("启动 OpenSCAD CLI 失败: {error}")))?;
    Ok(RunningJob {
        source_path: source_path.to_path_buf(),
        stl_path,
        child,
    })
}

fn poll_job(job: &mut Option<RunningJob>) -> Option<Result<RenderedArtifact, OpenScadError>> {
    let status = job.as_mut()?.child.try_wait().ok()??;
    let RunningJob {
        source_path,
        stl_path,
        child,
    } = job.take()?;
    let output = child
        .wait_with_output()
        .map_err(|error| OpenScadError(format!("等待 OpenSCAD CLI 结束失败: {error}")));
    Some(finalize_job(
        source_path,
        stl_path,
        status.success(),
        output,
    ))
}

fn finalize_job(
    source_path: PathBuf,
    stl_path: PathBuf,
    success: bool,
    output: Result<std::process::Output, OpenScadError>,
) -> Result<RenderedArtifact, OpenScadError> {
    let output = output?;
    if !success {
        let _ = fs::remove_file(&stl_path);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let message = if stderr.is_empty() {
            "OpenSCAD CLI 返回失败状态".to_owned()
        } else {
            format!("OpenSCAD CLI 执行失败: {stderr}")
        };
        return Err(OpenScadError(message));
    }
    let mesh = mesh::load_stl(&stl_path)
        .map_err(|error| OpenScadError(format!("解析 OpenSCAD 输出 STL 失败: {error}")))?;
    let _ = fs::remove_file(&stl_path);
    Ok(RenderedArtifact {
        source_path,
        mesh,
    })
}

fn cancel_job(job: &mut Option<RunningJob>) {
    if let Some(active_job) = job.as_mut() {
        let _ = active_job.child.kill();
        let _ = active_job.child.wait();
    }
    *job = None;
}

fn detect_openscad_path() -> Result<PathBuf, OpenScadError> {
    if let Ok(path) = env::var("OPENSCAD_PATH") {
        let candidate = PathBuf::from(path);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    find_in_path()
        .or_else(find_platform_path)
        .ok_or_else(|| OpenScadError("未找到 OpenSCAD CLI，可设置环境变量 OPENSCAD_PATH".into()))
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
            "C:\\Program Files\\OpenSCAD\\openscad.exe",
            "C:\\Program Files\\OpenSCAD (Nightly)\\openscad.exe",
        ]
    } else {
        vec!["/usr/bin/openscad", "/usr/local/bin/openscad"]
    };
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
}

fn temp_stl_path(source_path: &Path) -> PathBuf {
    let stem = source_path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("preview");
    let filename = format!("scad-studio-{stem}-{}.stl", std::process::id());
    env::temp_dir().join(filename)
}

impl std::error::Error for OpenScadError {}

impl fmt::Display for OpenScadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
