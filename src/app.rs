use std::path::{Path, PathBuf};

use scad_data::{LogEntry, LogLevel};
use scad_ui::{
    chat_panel::ChatPanel,
    file_tree::FileTree,
    tab_system::{TabId, TabManager},
};

use crate::{
    welcome::WelcomeTab,
    workspace::{remember_workspace, workspace_name},
};

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
    workspace_path: Option<PathBuf>,
    recent_workspaces: Vec<PathBuf>,
    left_panel_tab: LeftPanelTab,
    left_panel_width: f32,
    left_panel_open: bool,
    log_panel_open: bool,
    logs: Vec<LogEntry>,
    tabs: TabManager,
    chat_panel: ChatPanel,
    file_tree: Option<FileTree>,
}

impl StudioApp {
    pub fn new(recent_workspaces: Vec<PathBuf>) -> Self {
        let mut app = Self {
            workspace_path: None,
            recent_workspaces,
            left_panel_tab: LeftPanelTab::Files,
            left_panel_width: DEFAULT_LEFT_PANEL_WIDTH,
            left_panel_open: true,
            log_panel_open: false,
            logs: Vec::new(),
            tabs: TabManager::default(),
            chat_panel: ChatPanel::default(),
            file_tree: None,
        };
        app.ensure_welcome_tab();
        app
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
        self.refresh_welcome_tab();
    }

    pub fn window_title(&self) -> String {
        self.workspace_name()
            .map(|name| format!("SCAD Studio — {name}"))
            .unwrap_or_else(|| "SCAD Studio".to_string())
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

    pub fn tabs(&self) -> &TabManager {
        &self.tabs
    }

    pub fn tabs_mut(&mut self) -> &mut TabManager {
        &mut self.tabs
    }

    #[allow(dead_code)]
    pub fn tab_ids(&self) -> Vec<TabId> {
        self.tabs.tab_ids()
    }

    #[allow(dead_code)]
    pub fn close_tab(&mut self, id: TabId) {
        self.tabs.close_tab(id);
        self.ensure_welcome_tab();
    }

    pub fn begin_document_tab(&mut self) {
        if self.tabs.contains(WelcomeTab::tab_id()) {
            self.tabs.close_tab(WelcomeTab::tab_id());
        }
    }

    pub fn ensure_welcome_tab(&mut self) {
        if !self.tabs.is_empty() {
            return;
        }
        self.tabs
            .open_tab(Box::new(WelcomeTab::new(self.recent_workspaces.clone())));
    }

    fn refresh_welcome_tab(&mut self) {
        if let Some(tab) = self.tabs.tab_mut(WelcomeTab::tab_id())
            && let Some(tab) = tab.as_any_mut().downcast_mut::<WelcomeTab>()
        {
            tab.set_recent_workspaces(self.recent_workspaces.clone());
        }
    }

    pub fn chat_panel_mut(&mut self) -> &mut ChatPanel {
        &mut self.chat_panel
    }

    pub fn file_tree_mut(&mut self) -> Option<&mut FileTree> {
        self.file_tree.as_mut()
    }
}
