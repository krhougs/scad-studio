#![allow(dead_code)]

#[path = "../src/app.rs"]
mod app;
#[path = "../src/document_session.rs"]
mod document_session;
#[path = "../src/document_workspace.rs"]
mod document_workspace;
#[path = "../src/welcome.rs"]
mod welcome;
#[path = "../src/workspace.rs"]
mod workspace;

use app::StudioApp;
use std::path::PathBuf;
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

    assert!(app.show_welcome_state());
    assert!(app.documents().is_empty());
}

#[test]
fn setting_workspace_hides_welcome_state_without_creating_document() {
    let mut app = StudioApp::new(Vec::new());
    app.set_workspace_path(PathBuf::from("/tmp/demo-workspace"));

    assert!(!app.show_welcome_state());
    assert!(app.documents().is_empty());
}
