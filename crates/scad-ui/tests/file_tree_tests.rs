use scad_ui::document_tabs::DocumentTabKind;
use scad_ui::file_tree::{FileTree, FileTreeEntryKind, supported_document_tab_kind};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be monotonic")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir should be created");
    dir
}

fn write_file(path: &PathBuf, content: &str) {
    fs::write(path, content).expect("file should be written");
}

#[test]
fn directory_children_are_sorted_with_directories_first() {
    let root = unique_temp_dir("scad-ui-tree-sort");
    fs::create_dir(root.join("zeta")).expect("dir should exist");
    fs::create_dir(root.join("alpha")).expect("dir should exist");
    write_file(&root.join("delta.md"), "delta");
    write_file(&root.join("beta.scad"), "beta");

    let mut tree = FileTree::new(root.clone());
    let children = tree.ensure_children(&root).expect("children should load");
    let names: Vec<_> = children.iter().map(|entry| entry.name.clone()).collect();
    let kinds: Vec<_> = children.iter().map(|entry| entry.kind).collect();

    assert_eq!(names, vec!["alpha", "zeta", "beta.scad", "delta.md"]);
    assert_eq!(
        kinds,
        vec![
            FileTreeEntryKind::Directory,
            FileTreeEntryKind::Directory,
            FileTreeEntryKind::File,
            FileTreeEntryKind::File,
        ]
    );

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn children_cache_is_reused_until_invalidated() {
    let root = unique_temp_dir("scad-ui-tree-cache");
    write_file(&root.join("first.scad"), "first");

    let mut tree = FileTree::new(root.clone());
    let initial = tree.ensure_children(&root).expect("children should load");
    assert_eq!(initial.len(), 1);

    write_file(&root.join("second.scad"), "second");
    let cached = tree
        .ensure_children(&root)
        .expect("cached children should load");
    assert_eq!(cached.len(), 1);

    tree.invalidate(&root);
    let refreshed = tree
        .ensure_children(&root)
        .expect("children should refresh");
    let names: Vec<_> = refreshed.iter().map(|entry| entry.name.clone()).collect();
    assert_eq!(names, vec!["first.scad", "second.scad"]);

    fs::remove_dir_all(root).expect("temp dir should be removed");
}

#[test]
fn supported_tab_kind_maps_scad_and_markdown() {
    assert_eq!(
        supported_document_tab_kind(Path::new("a.scad")),
        Some(DocumentTabKind::Viewer)
    );
    assert_eq!(
        supported_document_tab_kind(Path::new("b.MD")),
        Some(DocumentTabKind::Markdown)
    );
    assert_eq!(
        supported_document_tab_kind(Path::new("long.markdown")),
        Some(DocumentTabKind::Markdown)
    );
    assert_eq!(
        supported_document_tab_kind(Path::new("photo.PNG")),
        Some(DocumentTabKind::Image)
    );
}

#[test]
fn unsupported_extensions_have_no_tab_kind() {
    assert_eq!(supported_document_tab_kind(Path::new("x.rs")), None);
    assert_eq!(supported_document_tab_kind(Path::new("Makefile")), None);
    assert_eq!(supported_document_tab_kind(Path::new("noext.")), None);
}
