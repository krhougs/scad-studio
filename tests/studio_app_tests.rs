#![allow(dead_code)]

#[path = "../src/app.rs"]
mod app;
#[path = "../src/welcome.rs"]
mod welcome;
#[path = "../src/workspace.rs"]
mod workspace;

use app::StudioApp;
use scad_ui::tab_system::WorkTab;
use std::path::PathBuf;
use welcome::WelcomeTab;
use workspace::{remember_workspace, sanitize_recent_workspaces, workspace_name};

#[test]
fn remember_workspace_moves_existing_path_to_front() {
    let recent = vec![
        PathBuf::from("/tmp/alpha"),
        PathBuf::from("/tmp/beta"),
        PathBuf::from("/tmp/gamma"),
    ];

    let updated = remember_workspace(&recent, &PathBuf::from("/tmp/beta"));

    assert_eq!(updated[0], PathBuf::from("/tmp/beta"));
    assert_eq!(updated.len(), 3);
}

#[test]
fn window_title_uses_workspace_name_when_workspace_exists() {
    let mut app = StudioApp::new(Vec::new());
    app.set_workspace_path(PathBuf::from("/tmp/demo-workspace"));

    assert_eq!(app.window_title(), "SCAD Studio — demo-workspace");
}

#[test]
fn workspace_name_falls_back_to_display_when_path_has_no_tail() {
    let name = workspace_name(std::path::Path::new("/"));

    assert!(!name.is_empty());
}

#[test]
fn sanitize_recent_workspaces_keeps_existing_directories_only_once() {
    let root = std::env::temp_dir().join(format!("studio-recent-{}", std::process::id()));
    let missing = root.join("missing");
    std::fs::create_dir_all(&root).expect("temp workspace should exist");

    let cleaned = sanitize_recent_workspaces(&[root.clone(), missing, root.clone()]);

    assert_eq!(cleaned, vec![root.clone()]);
    std::fs::remove_dir_all(root).expect("temp workspace should be removed");
}

#[test]
fn new_studio_app_starts_with_welcome_tab() {
    let app = StudioApp::new(Vec::new());

    assert_eq!(app.tab_ids(), vec![WelcomeTab::tab_id()]);
}

#[test]
fn closing_last_closable_tab_restores_welcome_tab() {
    let mut app = StudioApp::new(Vec::new());
    app.tabs_mut()
        .open_tab(Box::new(FakeClosableTab::new(99, "README.md")));

    app.close_tab(99);

    assert_eq!(app.tab_ids(), vec![WelcomeTab::tab_id()]);
}

struct FakeClosableTab {
    id: u64,
    title: String,
}

impl FakeClosableTab {
    fn new(id: u64, title: &str) -> Self {
        Self {
            id,
            title: title.to_owned(),
        }
    }
}

impl WorkTab for FakeClosableTab {
    fn id(&self) -> u64 {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn is_closable(&self) -> bool {
        true
    }

    fn show(&mut self, _ui: &mut egui::Ui, _ctx: &mut scad_ui::tab_system::TabContext<'_>) {}

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
