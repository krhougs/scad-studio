use std::{
    any::Any,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use scad_data::{FileWatcher, WatchMessage};
use scad_ui::{
    markdown::MarkdownDocument,
    tab_system::{TabContext, TabId, WorkTab},
};
use winit::{event_loop::EventLoopProxy, window::WindowId};

use crate::UserEvent;

pub struct MarkdownTab {
    id: TabId,
    path: PathBuf,
    title: String,
    source: String,
    document: MarkdownDocument,
    watcher: FileWatcher,
}

impl MarkdownTab {
    pub fn open(
        path: PathBuf,
        proxy: EventLoopProxy<UserEvent>,
        window_id: WindowId,
    ) -> Result<Self, String> {
        let source =
            std::fs::read_to_string(&path).map_err(|error| format!("读取 Markdown 失败: {error}"))?;
        let document = MarkdownDocument::parse(&source);
        let watcher = FileWatcher::new(build_source_notifier(proxy, window_id, tab_id_for_path("markdown", &path)));
        let tab = Self {
            id: tab_id_for_path("markdown", &path),
            title: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Markdown")
                .to_owned(),
            path: path.clone(),
            source,
            document,
            watcher,
        };
        tab.watcher.watch_files(vec![path]);
        Ok(tab)
    }

    pub fn tab_id_for(path: &Path) -> TabId {
        tab_id_for_path("markdown", path)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload(&mut self) -> Result<(), String> {
        self.source = std::fs::read_to_string(&self.path)
            .map_err(|error| format!("读取 Markdown 失败: {error}"))?;
        self.document = MarkdownDocument::parse(&self.source);
        Ok(())
    }
}

impl WorkTab for MarkdownTab {
    fn id(&self) -> TabId {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn is_closable(&self) -> bool {
        true
    }

    fn show(&mut self, ui: &mut egui::Ui, _ctx: &mut TabContext<'_>) {
        self.document.show(ui);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn tab_id_for_path(kind: &str, path: &Path) -> TabId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish()
}

fn build_source_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(WatchMessage) + Send + 'static {
    move |message| match message {
        WatchMessage::Changed(path) => {
            let _ = proxy.send_event(UserEvent::SourceChanged(window_id, tab_id, path));
        }
        WatchMessage::Error(message) => {
            let _ = proxy.send_event(UserEvent::WatchError(window_id, tab_id, message));
        }
    }
}
