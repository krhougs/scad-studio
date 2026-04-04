use std::{
    fmt, fs,
    path::PathBuf,
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread,
    time::{Duration, Instant},
};

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct FileWatcher {
    tx: Sender<WatchCommand>,
}

#[derive(Debug, Clone)]
pub enum WatchMessage {
    Changed(PathBuf),
    Error(String),
}

enum WatchCommand {
    Watch(Vec<PathBuf>),
}

#[derive(Debug, Clone)]
pub struct WatchError(String);

impl FileWatcher {
    pub fn new<F>(notify_change: F) -> Self
    where
        F: Fn(WatchMessage) + Send + 'static,
    {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || worker_loop(rx, notify_change));
        Self { tx }
    }

    #[allow(dead_code)]
    pub fn watch(&self, path: PathBuf) {
        self.watch_files(vec![path]);
    }

    pub fn watch_files(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(WatchCommand::Watch(paths));
    }
}

fn worker_loop<F>(rx: Receiver<WatchCommand>, notify_change: F)
where
    F: Fn(WatchMessage) + Send + 'static,
{
    let (raw_tx, raw_rx) = mpsc::channel();
    let mut watcher: Option<RecommendedWatcher> = None;
    let mut watched_files = Vec::new();
    let mut pending_deadline: Option<Instant> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(WatchCommand::Watch(paths)) => {
                pending_deadline = None;
                watched_files = paths.iter().cloned().map(normalize_path).collect();
                match create_watcher(&paths, raw_tx.clone()) {
                    Ok(created) => watcher = Some(created),
                    Err(error) => {
                        watcher = None;
                        notify_change(WatchMessage::Error(error.to_string()));
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        let changed = drain_events(&raw_rx, &watched_files, &notify_change);
        if changed {
            pending_deadline = Some(Instant::now() + DEBOUNCE);
        }
        if ready_to_notify(pending_deadline) {
            pending_deadline = None;
            if let Some(path) = watched_files.first().cloned() {
                notify_change(WatchMessage::Changed(path));
            }
        }
        if watcher.is_none() && watched_files.is_empty() {
            continue;
        }
    }
}

fn create_watcher(
    paths: &[PathBuf],
    raw_tx: Sender<notify::Result<notify::Event>>,
) -> Result<RecommendedWatcher, WatchError> {
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = raw_tx.send(event);
    })
    .map_err(|error| WatchError(format!("创建文件监控失败: {error}")))?;
    for root in watch_roots(paths)? {
        watcher
            .watch(&root, RecursiveMode::NonRecursive)
            .map_err(|error| WatchError(format!("注册文件监控失败: {error}")))?;
    }
    Ok(watcher)
}

fn drain_events(
    raw_rx: &Receiver<notify::Result<notify::Event>>,
    watched_files: &[PathBuf],
    notify_change: &impl Fn(WatchMessage),
) -> bool {
    let mut changed = false;
    while let Ok(event) = raw_rx.try_recv() {
        let Ok(event) = event else {
            if let Err(error) = event {
                notify_change(WatchMessage::Error(format!("文件监控事件失败: {error}")));
            }
            continue;
        };
        if matches_any_path(&event.paths, watched_files) {
            changed = true;
        }
    }
    changed
}

pub(crate) fn matches_path(paths: &[PathBuf], watched_file: Option<&std::path::Path>) -> bool {
    let Some(watched_file) = watched_file else {
        return false;
    };
    let watched_file = normalize_path(watched_file.to_path_buf());
    paths
        .iter()
        .any(|path| normalize_path(path.clone()) == watched_file)
}

pub(crate) fn matches_any_path(paths: &[PathBuf], watched_files: &[PathBuf]) -> bool {
    watched_files
        .iter()
        .any(|watched| matches_path(paths, Some(watched.as_path())))
}

fn ready_to_notify(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
}

fn watch_roots(paths: &[PathBuf]) -> Result<Vec<PathBuf>, WatchError> {
    let mut roots = Vec::new();
    for path in paths {
        let root = path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| WatchError("待监听文件缺少父目录".into()))?;
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    Ok(roots)
}

fn normalize_path(path: PathBuf) -> PathBuf {
    fs::canonicalize(&path).unwrap_or(path)
}

impl std::error::Error for WatchError {}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
