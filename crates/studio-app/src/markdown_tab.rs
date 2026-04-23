use std::{
    any::Any,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use egui_commonmark::CommonMarkCache;
use scad_ui::{
    markdown::MarkdownDocument,
    tab_system::{TabContext, TabId, WorkTab},
};
use winit::{event_loop::EventLoopProxy, window::WindowId};

use crate::{
    UserEvent,
    protocol_client::{DesktopProtocolClient, WatchSubscriptionHandle},
};

pub struct MarkdownTab {
    id: TabId,
    path: PathBuf,
    title: String,
    source: String,
    document: MarkdownDocument,
    cache: CommonMarkCache,
    client: DesktopProtocolClient,
    _watch_subscription: WatchSubscriptionHandle,
}

impl MarkdownTab {
    pub fn open(
        client: DesktopProtocolClient,
        path: PathBuf,
        proxy: EventLoopProxy<UserEvent>,
        window_id: WindowId,
    ) -> Result<Self, String> {
        let source = client.read_text_file(&path, "Markdown")?;
        let document = MarkdownDocument::parse(&source);
        let watch_subscription = client.subscribe_path(
            &path,
            build_changed_notifier(proxy.clone(), window_id, tab_id_for_path("markdown", &path)),
            build_error_notifier(proxy, window_id, tab_id_for_path("markdown", &path)),
        )?;
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
            cache: CommonMarkCache::default(),
            client,
            _watch_subscription: watch_subscription,
        };
        Ok(tab)
    }

    pub fn legacy_tab_id(&self) -> TabId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload(&mut self) -> Result<(), String> {
        self.source = self.client.read_text_file(&self.path, "Markdown")?;
        self.document = MarkdownDocument::parse(&self.source);
        self.cache = CommonMarkCache::default();
        Ok(())
    }

    pub fn show_document(&mut self, ui: &mut egui::Ui) {
        self.document.show(ui, &mut self.cache);
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
        self.show_document(ui);
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

fn build_changed_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(PathBuf) + Send + 'static {
    move |path| {
        let _ = proxy.send_event(UserEvent::SourceChanged(window_id, tab_id, path));
    }
}

fn build_error_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(String) + Send + 'static {
    move |message| {
        let _ = proxy.send_event(UserEvent::WatchError(window_id, tab_id, message));
    }
}
