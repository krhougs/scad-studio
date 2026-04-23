use std::path::{Path, PathBuf};

use scad_ui::{
    chat_panel::ChatPanel,
    file_tree::{FileTree, FileTreeEntry},
};
use scad_viewer::app::{LogEntry, LogLevel};
use studio_common::{
    DocumentKey, DocumentTab, DocumentWorkspace, remember_workspace, workspace_name,
};

use crate::{
    image_tab::ImageTab, markdown_tab::MarkdownTab, studio_document::StudioDocumentSession,
    viewer_tab::ViewerTab,
};
use scad_ui::tab_system::TabId;
use studio_common::{DocumentOpenOutcome, DocumentSlot};

const DEFAULT_LEFT_PANEL_WIDTH: f32 = 280.0;
const MIN_LEFT_PANEL_WIDTH: f32 = 220.0;
const MAX_LEFT_PANEL_WIDTH: f32 = 480.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LeftPanelTab {
    Chat,
    #[default]
    Files,
}

pub struct StudioApp {
    root_viewport_fullscreen: bool,
    workspace_path: Option<PathBuf>,
    recent_workspaces: Vec<PathBuf>,
    left_panel_tab: LeftPanelTab,
    left_panel_width: f32,
    left_panel_open: bool,
    log_panel_open: bool,
    logs: Vec<LogEntry>,
    documents: DocumentWorkspace<StudioDocumentSession>,
    chat_panel: ChatPanel,
    file_tree: Option<FileTree>,
}

impl StudioApp {
    pub fn new(recent_workspaces: Vec<PathBuf>) -> Self {
        Self {
            root_viewport_fullscreen: false,
            workspace_path: None,
            recent_workspaces,
            left_panel_tab: LeftPanelTab::Files,
            left_panel_width: DEFAULT_LEFT_PANEL_WIDTH,
            left_panel_open: true,
            log_panel_open: false,
            logs: Vec::new(),
            documents: DocumentWorkspace::default(),
            chat_panel: ChatPanel::default(),
            file_tree: None,
        }
    }

    pub fn workspace_path(&self) -> Option<&Path> {
        self.workspace_path.as_deref()
    }

    pub fn workspace_name(&self) -> Option<String> {
        self.workspace_path().map(workspace_name)
    }

    pub fn set_workspace_path(&mut self, path: PathBuf) {
        self.recent_workspaces = remember_workspace(&self.recent_workspaces, &path);
        self.file_tree = Some(FileTree::new(path.clone()));
        self.workspace_path = Some(path);
    }

    pub fn set_file_tree_children(&mut self, path: PathBuf, entries: Vec<FileTreeEntry>) {
        if let Some(tree) = self.file_tree.as_mut() {
            tree.set_children(path, entries);
        }
    }

    pub fn window_title(&self) -> String {
        self.workspace_name()
            .map(|name| format!("SCAD Studio — {name}"))
            .unwrap_or_else(|| "SCAD Studio".to_string())
    }

    pub fn root_viewport_fullscreen(&self) -> bool {
        self.root_viewport_fullscreen
    }

    pub fn set_root_viewport_fullscreen(&mut self, fullscreen: bool) {
        self.root_viewport_fullscreen = fullscreen;
    }

    pub fn recent_workspaces(&self) -> &[PathBuf] {
        &self.recent_workspaces
    }

    pub fn left_panel_tab(&self) -> LeftPanelTab {
        self.left_panel_tab
    }

    pub fn set_left_panel_tab(&mut self, tab: LeftPanelTab) {
        self.left_panel_tab = tab;
    }

    pub fn left_panel_width(&self) -> f32 {
        self.left_panel_width
    }

    pub fn set_left_panel_width(&mut self, width: f32) {
        self.left_panel_width = width.clamp(MIN_LEFT_PANEL_WIDTH, MAX_LEFT_PANEL_WIDTH);
    }

    pub fn left_panel_open(&self) -> bool {
        self.left_panel_open
    }

    pub fn toggle_left_panel(&mut self) {
        self.left_panel_open = !self.left_panel_open;
    }

    pub fn log_panel_open(&self) -> bool {
        self.log_panel_open
    }

    pub fn toggle_log_panel(&mut self) {
        self.log_panel_open = !self.log_panel_open;
    }

    pub fn log_entries(&self) -> &[LogEntry] {
        &self.logs
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn push_log(&mut self, level: LogLevel, message: impl Into<String>) {
        if level == LogLevel::Error {
            self.log_panel_open = true;
        }
        self.logs.push(LogEntry {
            level,
            message: message.into(),
        });
    }

    pub fn status_text(&self) -> String {
        self.workspace_path()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未打开 Workspace".to_string())
    }

    pub fn show_welcome_state(&self) -> bool {
        self.workspace_path.is_none() && self.documents.is_empty()
    }

    pub fn has_open_documents(&self) -> bool {
        !self.documents.is_empty()
    }

    pub fn document_tabs(&self) -> Vec<DocumentTab> {
        self.documents.tabs()
    }

    pub fn set_active_document(&mut self, key: DocumentKey) {
        self.documents.set_active(key);
    }

    pub fn close_document(&mut self, key: &DocumentKey) {
        let _ = self.documents.close(key);
    }

    pub fn contains_document(&self, key: &DocumentKey) -> bool {
        self.documents.contains(key)
    }

    pub fn open_document(&mut self, document: StudioDocumentSession) -> DocumentOpenOutcome {
        let descriptor = document.descriptor();
        self.documents
            .open_or_activate(DocumentSlot::new(descriptor, document))
    }

    pub fn active_viewer(&self) -> Option<&ViewerTab> {
        self.documents.active()?.session().as_viewer()
    }

    pub fn active_viewer_mut(&mut self) -> Option<&mut ViewerTab> {
        self.documents.active_mut()?.session_mut().as_viewer_mut()
    }

    pub fn active_markdown_mut(&mut self) -> Option<&mut MarkdownTab> {
        self.documents.active_mut()?.session_mut().as_markdown_mut()
    }

    pub fn active_image_mut(&mut self) -> Option<&mut ImageTab> {
        self.documents.active_mut()?.session_mut().as_image_mut()
    }

    pub fn document_by_legacy_tab_id_mut(
        &mut self,
        id: TabId,
    ) -> Option<&mut StudioDocumentSession> {
        self.documents
            .slots_mut()
            .iter_mut()
            .find(|slot| slot.session().legacy_tab_id() == id)
            .map(|slot| slot.session_mut())
    }

    pub fn chat_panel_mut(&mut self) -> &mut ChatPanel {
        &mut self.chat_panel
    }

    pub fn file_tree_mut(&mut self) -> Option<&mut FileTree> {
        self.file_tree.as_mut()
    }
}
