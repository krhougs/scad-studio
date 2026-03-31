use std::{
    fs,
    fmt,
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
    Watch(PathBuf),
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

    pub fn watch(&self, path: PathBuf) {
        let _ = self.tx.send(WatchCommand::Watch(path));
    }
}

fn worker_loop<F>(rx: Receiver<WatchCommand>, notify_change: F)
where
    F: Fn(WatchMessage) + Send + 'static,
{
    let (raw_tx, raw_rx) = mpsc::channel();
    let mut watcher: Option<RecommendedWatcher> = None;
    let mut watched_file: Option<PathBuf> = None;
    let mut pending_deadline: Option<Instant> = None;
    loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(WatchCommand::Watch(path)) => {
                pending_deadline = None;
                watched_file = Some(normalize_path(path.clone()));
                match create_watcher(path, raw_tx.clone()) {
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
        let changed = drain_events(&raw_rx, watched_file.as_deref(), &notify_change);
        if changed {
            pending_deadline = Some(Instant::now() + DEBOUNCE);
        }
        if ready_to_notify(pending_deadline) {
            pending_deadline = None;
            if let Some(path) = watched_file.clone() {
                notify_change(WatchMessage::Changed(path));
            }
        }
        if watcher.is_none() && watched_file.is_none() {
            continue;
        }
    }
}

fn create_watcher(
    path: PathBuf,
    raw_tx: Sender<notify::Result<notify::Event>>,
) -> Result<RecommendedWatcher, WatchError> {
    let watch_root = path
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| WatchError("待监听文件缺少父目录".into()))?;
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = raw_tx.send(event);
    })
    .map_err(|error| WatchError(format!("创建文件监控失败: {error}")))?;
    watcher
        .watch(&watch_root, RecursiveMode::NonRecursive)
        .map_err(|error| WatchError(format!("注册文件监控失败: {error}")))?;
    Ok(watcher)
}

fn drain_events(
    raw_rx: &Receiver<notify::Result<notify::Event>>,
    watched_file: Option<&std::path::Path>,
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
        if matches_path(&event.paths, watched_file) {
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
    paths.iter().any(|path| normalize_path(path.clone()) == watched_file)
}

fn ready_to_notify(deadline: Option<Instant>) -> bool {
    deadline.is_some_and(|deadline| Instant::now() >= deadline)
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
