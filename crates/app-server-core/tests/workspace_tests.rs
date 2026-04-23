use app_server_core::{current_workspace, list_workspace_entries, read_file_response};
use app_server_protocol::{PathHandle, WorkspaceId};
use std::fs;

#[test]
fn workspace_current_uses_root_name() {
    let response = current_workspace(
        std::path::Path::new("/tmp/my-workspace"),
        WorkspaceId::new("ws"),
    );
    assert_eq!(response.workspace_id.0, "ws");
    assert_eq!(response.root_name, "my-workspace");
}

#[test]
fn workspace_list_returns_sorted_entries() {
    let root = temp_dir("workspace-list");
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/readme.md"), "# hi").unwrap();
    fs::write(root.join("model.scad"), "cube();").unwrap();

    let response = list_workspace_entries(&root, WorkspaceId::new("ws"), None).unwrap();
    assert_eq!(response.entries.len(), 2);
    assert_eq!(response.entries[0].path.display_path(), "docs");
    assert_eq!(response.entries[1].path.display_path(), "model.scad");

    let _ = fs::remove_file(root.join("docs/readme.md"));
    let _ = fs::remove_file(root.join("model.scad"));
    let _ = fs::remove_dir(root.join("docs"));
    let _ = fs::remove_dir(root);
}

#[test]
fn file_read_honors_extension_denylists() {
    let root = temp_dir("workspace-read");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("guide.md"), "# hi").unwrap();
    fs::write(root.join("model.scad"), "cube();").unwrap();

    let guide = PathHandle::new(WorkspaceId::new("ws"), ["guide.md"]).unwrap();
    let response = read_file_response(&root, &guide, &[]).unwrap();
    assert_eq!(response.media_type, "text/markdown");

    let scad = PathHandle::new(WorkspaceId::new("ws"), ["model.scad"]).unwrap();
    let error = read_file_response(&root, &scad, &[".scad".to_string()]).unwrap_err();
    assert_eq!(
        error.code,
        app_server_protocol::ProtocolErrorCode::UnsupportedFileTypeForClient
    );

    let _ = fs::remove_file(root.join("guide.md"));
    let _ = fs::remove_file(root.join("model.scad"));
    let _ = fs::remove_dir(root);
}

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{stamp}"))
}
