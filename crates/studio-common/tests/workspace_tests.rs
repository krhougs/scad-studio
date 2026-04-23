use std::path::PathBuf;

use studio_common::{remember_workspace, sanitize_recent_workspaces, workspace_name};

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
