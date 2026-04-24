use app_server_core::{
    current_workspace, list_workspace_entries, read_file_response, resolve_workspace_write_path,
};
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
    assert_eq!(
        response.entries[0].path.as_ref().unwrap().display_path(),
        "docs"
    );
    assert_eq!(
        response.entries[1].path.as_ref().unwrap().display_path(),
        "model.scad"
    );

    let _ = fs::remove_file(root.join("docs/readme.md"));
    let _ = fs::remove_file(root.join("model.scad"));
    let _ = fs::remove_dir(root.join("docs"));
    let _ = fs::remove_dir(root);
}

#[test]
fn workspace_list_returns_invalid_entries_without_handle() {
    let root = temp_dir("workspace-invalid-list");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("valid.scad"), "cube();").unwrap();
    fs::write(root.join("CON.scad"), "cube();").unwrap();

    let response = list_workspace_entries(&root, WorkspaceId::new("ws"), None).unwrap();
    let invalid = response
        .entries
        .iter()
        .find(|entry| entry.name == "CON.scad")
        .expect("invalid entry should be returned");
    assert!(invalid.path.is_none());
    assert!(invalid.path_error.as_deref().unwrap().contains("reserved"));

    let valid = response
        .entries
        .iter()
        .find(|entry| entry.name == "valid.scad")
        .expect("valid entry should be returned");
    assert!(valid.path.is_some());

    let _ = fs::remove_file(root.join("valid.scad"));
    let _ = fs::remove_file(root.join("CON.scad"));
    let _ = fs::remove_dir(root);
}

#[test]
fn workspace_list_marks_case_conflicts_invalid() {
    let root = temp_dir("workspace-case-conflict");
    fs::create_dir_all(&root).unwrap();
    let first = root.join("Cube.scad");
    let second = root.join("cube.scad");
    fs::write(&first, "cube();").unwrap();
    fs::write(&second, "cube();").unwrap();

    if fs::read_dir(&root).unwrap().filter_map(Result::ok).count() < 2 {
        let _ = fs::remove_file(&first);
        let _ = fs::remove_dir(root);
        return;
    }

    let response = list_workspace_entries(&root, WorkspaceId::new("ws"), None).unwrap();
    assert_eq!(response.entries.len(), 2);
    assert!(response.entries.iter().all(|entry| entry.path.is_none()));
    assert!(
        response
            .entries
            .iter()
            .all(|entry| entry.path_error.as_deref().unwrap().contains("case"))
    );

    let _ = fs::remove_file(first);
    let _ = fs::remove_file(second);
    let _ = fs::remove_dir(root);
}

#[cfg(unix)]
#[test]
fn workspace_list_marks_non_utf8_name_invalid_without_root_handle() {
    let root = temp_dir("workspace-non-utf8");
    fs::create_dir_all(&root).unwrap();
    let name = "bad\u{fffd}.scad";
    fs::write(root.join(name), "cube();").unwrap();

    let response = list_workspace_entries(&root, WorkspaceId::new("ws"), None).unwrap();
    let entry = response
        .entries
        .iter()
        .find(|entry| entry.name.contains(".scad"))
        .expect("non-UTF-8 entry should be returned");

    assert!(entry.path.is_none());
    assert!(entry.path_error.as_deref().unwrap().contains("UTF-8"));

    let _ = fs::remove_file(root.join(name));
    let _ = fs::remove_dir(root);
}

#[cfg(unix)]
#[test]
fn workspace_list_marks_symlink_escape_invalid_without_failing_directory() {
    let root = temp_dir("workspace-list-symlink");
    let outside = temp_dir("workspace-list-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
    fs::write(root.join("valid.scad"), "cube();").unwrap();

    let response = list_workspace_entries(&root, WorkspaceId::new("ws"), None).unwrap();
    let linked = response
        .entries
        .iter()
        .find(|entry| entry.name == "linked")
        .expect("symlink entry should be returned");
    assert!(linked.path.is_none());
    assert!(linked.path_error.as_deref().unwrap().contains("workspace"));
    assert!(
        response
            .entries
            .iter()
            .any(|entry| entry.name == "valid.scad" && entry.path.is_some())
    );

    let _ = fs::remove_file(root.join("linked"));
    let _ = fs::remove_file(root.join("valid.scad"));
    let _ = fs::remove_dir(root);
    let _ = fs::remove_dir(outside);
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

#[test]
fn workspace_write_path_resolves_existing_parent_inside_workspace() {
    let root = temp_dir("workspace-write");
    fs::create_dir_all(root.join("models")).unwrap();
    let handle = PathHandle::new(WorkspaceId::new("ws"), ["models", "out.3mf"]).unwrap();

    let resolved = resolve_workspace_write_path(&root, &handle).unwrap();

    assert_eq!(
        resolved,
        root.canonicalize().unwrap().join("models/out.3mf")
    );

    let _ = fs::remove_dir(root.join("models"));
    let _ = fs::remove_dir(root);
}

#[cfg(unix)]
#[test]
fn workspace_write_path_rejects_symlink_escape_parent() {
    let root = temp_dir("workspace-write-symlink");
    let outside = temp_dir("workspace-outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();
    let handle = PathHandle::new(WorkspaceId::new("ws"), ["linked", "out.3mf"]).unwrap();

    let error = resolve_workspace_write_path(&root, &handle).unwrap_err();

    assert_eq!(
        error.code,
        app_server_protocol::ProtocolErrorCode::InvalidPathHandle
    );

    let _ = fs::remove_file(root.join("linked"));
    let _ = fs::remove_dir(root);
    let _ = fs::remove_dir(outside);
}

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{stamp}"))
}
