use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    fmt,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

const DEBOUNCE: Duration = Duration::from_millis(300);

pub struct FileWatcher {
    tx: UnboundedSender<WatchCommand>,
}

#[derive(Debug, Clone)]
pub enum WatchMessage {
    Changed(Vec<PathBuf>),
    Error(String),
}

enum WatchCommand {
    Watch(Vec<PathBuf>),
}

#[derive(Debug, Default, Clone)]
struct WatchCoalescer {
    watched_files: Vec<PathBuf>,
    subscribed: bool,
    pending_deadline: Option<Instant>,
    pending_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct WatchError(String);

impl FileWatcher {
    pub fn new<F>(notify_change: F) -> Self
    where
        F: Fn(WatchMessage) + Send + 'static,
    {
        let (tx, rx) = unbounded_channel();
        tokio::spawn(worker_loop(rx, notify_change));
        Self { tx }
    }

    pub fn watch_files(&self, paths: Vec<PathBuf>) {
        let _ = self.tx.send(WatchCommand::Watch(paths));
    }
}

async fn worker_loop<F>(mut rx: UnboundedReceiver<WatchCommand>, notify_change: F)
where
    F: Fn(WatchMessage) + Send + 'static,
{
    let (raw_tx, mut raw_rx) = unbounded_channel();
    let mut watcher: Option<RecommendedWatcher> = None;
    let mut coalescer = WatchCoalescer::default();
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            command = rx.recv() => {
                let Some(command) = command else {
                    break;
                };
                match command {
                    WatchCommand::Watch(paths) => {
                        match normalize_paths(&paths).await {
                            Ok(normalized_paths) => coalescer.subscribe(normalized_paths),
                            Err(error) => {
                                notify_change(WatchMessage::Error(error.to_string()));
                                continue;
                            }
                        }
                        match create_watcher(&paths, raw_tx.clone()).await {
                    Ok(created) => watcher = Some(created),
                    Err(error) => {
                        watcher = None;
                        notify_change(WatchMessage::Error(error.to_string()));
                    }
                }
            }
                }
            }
            event = raw_rx.recv() => {
                if let Some(event) = event {
                    for path in event_paths(event, &notify_change) {
                        coalescer.push_raw_path(Instant::now(), canonical_or_original(path).await);
                    }
                }
            }
            _ = tick.tick() => {
                if let Some(message) = coalescer.take_due_notification(Instant::now()) {
                    notify_change(message);
                }
                let _keep_alive = &watcher;
            }
        }
    }
}

async fn create_watcher(
    paths: &[PathBuf],
    raw_tx: UnboundedSender<notify::Result<notify::Event>>,
) -> Result<RecommendedWatcher, WatchError> {
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = raw_tx.send(event);
    })
    .map_err(|error| WatchError(format!("创建文件监控失败: {error}")))?;
    for (root, mode) in watch_roots(paths).await? {
        watcher
            .watch(&root, mode)
            .map_err(|error| WatchError(format!("注册文件监控失败: {error}")))?;
    }
    Ok(watcher)
}

fn event_paths(
    event: notify::Result<notify::Event>,
    notify_change: &impl Fn(WatchMessage),
) -> Vec<PathBuf> {
    match event {
        Ok(event) => event.paths,
        Err(error) => {
            notify_change(WatchMessage::Error(format!("文件监控事件失败: {error}")));
            Vec::new()
        }
    }
}

pub async fn matches_path(paths: &[PathBuf], watched_file: Option<&std::path::Path>) -> bool {
    let Some(watched_file) = watched_file else {
        return false;
    };
    let watched_file = canonical_or_original(watched_file.to_path_buf()).await;
    let mut candidates = Vec::with_capacity(paths.len());
    for path in paths {
        candidates.push(canonical_or_original(path.clone()).await);
    }
    matches_normalized_path(&candidates, &watched_file)
}

pub async fn matches_any_path(paths: &[PathBuf], watched_files: &[PathBuf]) -> bool {
    for watched in watched_files {
        if matches_path(paths, Some(watched.as_path())).await {
            return true;
        }
    }
    false
}

fn matches_normalized_path(paths: &[PathBuf], watched_file: &std::path::Path) -> bool {
    paths.iter().any(|path| {
        let candidate = normalize_path(path.clone());
        candidate == watched_file || candidate.starts_with(watched_file)
    })
}

fn matches_any_normalized_path(paths: &[PathBuf], watched_files: &[PathBuf]) -> bool {
    watched_files
        .iter()
        .any(|watched| matches_normalized_path(paths, watched))
}

async fn watch_roots(paths: &[PathBuf]) -> Result<Vec<(PathBuf, RecursiveMode)>, WatchError> {
    let mut roots = Vec::new();
    for path in paths {
        let is_dir = tokio::fs::metadata(path)
            .await
            .is_ok_and(|metadata| metadata.is_dir());
        let root = if is_dir {
            (
                canonical_or_original(path.clone()).await,
                RecursiveMode::Recursive,
            )
        } else {
            let parent = path
                .parent()
                .map(PathBuf::from)
                .ok_or_else(|| WatchError("待监听文件缺少父目录".into()))?;
            (
                canonical_or_original(parent).await,
                RecursiveMode::NonRecursive,
            )
        };
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    Ok(roots)
}

fn normalize_path(path: PathBuf) -> PathBuf {
    path
}

async fn normalize_paths(paths: &[PathBuf]) -> Result<Vec<PathBuf>, WatchError> {
    let mut normalized = Vec::with_capacity(paths.len());
    for path in paths {
        normalized.push(canonical_or_original(path.clone()).await);
    }
    Ok(normalized)
}

async fn canonical_or_original(path: PathBuf) -> PathBuf {
    tokio::fs::canonicalize(&path).await.unwrap_or(path)
}

impl std::error::Error for WatchError {}

impl fmt::Display for WatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl WatchCoalescer {
    fn subscribe(&mut self, paths: Vec<PathBuf>) {
        self.watched_files = paths;
        self.subscribed = true;
        self.pending_deadline = None;
        self.pending_paths.clear();
    }

    #[cfg(test)]
    fn unsubscribe(&mut self) {
        self.subscribed = false;
        self.pending_deadline = None;
        self.pending_paths.clear();
    }

    #[cfg(test)]
    fn disconnect(&mut self) {
        self.unsubscribe();
    }

    #[cfg(test)]
    fn reconnect(&mut self) {
        self.pending_deadline = None;
        self.pending_paths.clear();
    }

    fn push_raw_path(&mut self, now: Instant, path: PathBuf) {
        if !self.subscribed {
            return;
        }
        if is_filtered_temp_path(&path) {
            return;
        }
        let normalized = normalize_path(path);
        if !matches_any_normalized_path(std::slice::from_ref(&normalized), &self.watched_files) {
            return;
        }
        if !self.pending_paths.contains(&normalized) {
            self.pending_paths.push(normalized);
        }
        self.pending_deadline = Some(now + DEBOUNCE);
    }

    fn take_due_notification(&mut self, now: Instant) -> Option<WatchMessage> {
        let deadline = self.pending_deadline?;
        if now < deadline {
            return None;
        }
        self.pending_deadline = None;
        let paths = std::mem::take(&mut self.pending_paths);
        (!paths.is_empty()).then_some(WatchMessage::Changed(paths))
    }
}

fn is_filtered_temp_path(path: &PathBuf) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name.ends_with('~') || name.starts_with(".~") || name.contains(".~tmp")
}

#[cfg(test)]
mod watch_coalescer_tests {
    use super::*;

    #[test]
    fn coalesces_multiple_writes_into_single_event() {
        let mut state = WatchCoalescer::default();
        let watched = PathBuf::from("/tmp/example.scad");
        state.subscribe(vec![watched.clone()]);
        let now = Instant::now();

        state.push_raw_path(now, watched.clone());
        state.push_raw_path(now + Duration::from_millis(50), watched.clone());
        state.push_raw_path(now + Duration::from_millis(100), watched.clone());

        assert!(
            state
                .take_due_notification(now + Duration::from_millis(200))
                .is_none()
        );
        assert!(matches!(
            state.take_due_notification(now + DEBOUNCE + Duration::from_millis(100)),
            Some(WatchMessage::Changed(paths)) if paths == vec![watched]
        ));
        assert!(
            state
                .take_due_notification(now + DEBOUNCE + Duration::from_millis(200))
                .is_none()
        );
    }

    #[test]
    fn filters_editor_temp_files() {
        let mut state = WatchCoalescer::default();
        state.subscribe(vec![PathBuf::from("/tmp/example.scad")]);
        let now = Instant::now();

        state.push_raw_path(now, PathBuf::from("/tmp/example.scad~"));
        state.push_raw_path(now, PathBuf::from("/tmp/.~tmp-example.scad"));
        assert!(
            state
                .take_due_notification(now + DEBOUNCE + Duration::from_millis(1))
                .is_none()
        );
    }

    #[test]
    fn unsubscribed_watcher_emits_no_event() {
        let mut state = WatchCoalescer::default();
        let watched = PathBuf::from("/tmp/example.scad");
        state.subscribe(vec![watched.clone()]);
        state.unsubscribe();
        let now = Instant::now();
        state.push_raw_path(now, watched);
        assert!(
            state
                .take_due_notification(now + DEBOUNCE + Duration::from_millis(1))
                .is_none()
        );
    }

    #[test]
    fn disconnect_invalidates_subscription_until_resubscribed() {
        let mut state = WatchCoalescer::default();
        let watched = PathBuf::from("/tmp/example.scad");
        let now = Instant::now();
        state.subscribe(vec![watched.clone()]);
        state.disconnect();
        state.push_raw_path(now, watched.clone());
        assert!(
            state
                .take_due_notification(now + DEBOUNCE + Duration::from_millis(1))
                .is_none()
        );

        state.reconnect();
        state.push_raw_path(now + Duration::from_secs(1), watched.clone());
        assert!(
            state
                .take_due_notification(
                    now + Duration::from_secs(1) + DEBOUNCE + Duration::from_millis(1)
                )
                .is_none()
        );

        state.subscribe(vec![watched.clone()]);
        state.push_raw_path(now + Duration::from_secs(2), watched.clone());
        assert!(matches!(
            state.take_due_notification(now + Duration::from_secs(2) + DEBOUNCE + Duration::from_millis(1)),
            Some(WatchMessage::Changed(paths)) if paths == vec![watched]
        ));
    }

    #[test]
    fn preserves_actual_changed_paths_under_watched_directory() {
        let mut state = WatchCoalescer::default();
        let root = PathBuf::from("/tmp/workspace");
        let scad = root.join("examples/cube.scad");
        let settings = root.join("examples/cube.scad.json");
        state.subscribe(vec![root]);
        let now = Instant::now();

        state.push_raw_path(now, scad.clone());
        state.push_raw_path(now + Duration::from_millis(50), settings.clone());

        assert!(matches!(
            state.take_due_notification(now + DEBOUNCE + Duration::from_millis(60)),
            Some(WatchMessage::Changed(paths)) if paths == vec![scad, settings]
        ));
    }
}
