use scad_ui::document_tabs::DocumentTabKind;
use scad_ui::file_tree::{FileTree, FileTreeEntry, FileTreeEntryKind, supported_document_tab_kind};
use std::path::{Path, PathBuf};

#[test]
fn set_children_updates_cache_for_requested_directory() {
    let root = PathBuf::from("/tmp/workspace");
    let mut tree = FileTree::new(root.clone());
    tree.set_children(
        root.clone(),
        vec![
            FileTreeEntry {
                name: "alpha".into(),
                path: root.join("alpha"),
                kind: FileTreeEntryKind::Directory,
            },
            FileTreeEntry {
                name: "beta.scad".into(),
                path: root.join("beta.scad"),
                kind: FileTreeEntryKind::File,
            },
        ],
    );
    let children = tree.cached_children(&root).expect("children should exist");
    let names: Vec<_> = children.iter().map(|entry| entry.name.clone()).collect();
    let kinds: Vec<_> = children.iter().map(|entry| entry.kind).collect();

    assert_eq!(names, vec!["alpha", "beta.scad"]);
    assert_eq!(
        kinds,
        vec![FileTreeEntryKind::Directory, FileTreeEntryKind::File]
    );
}

#[test]
fn invalidate_drops_cached_children_for_related_paths() {
    let root = PathBuf::from("/tmp/workspace");
    let nested = root.join("nested");
    let mut tree = FileTree::new(root.clone());
    tree.set_children(root.clone(), Vec::new());
    tree.set_children(nested.clone(), Vec::new());
    assert!(tree.cached_children(&root).is_some());
    assert!(tree.cached_children(&nested).is_some());

    tree.invalidate(&root);
    assert!(tree.cached_children(&root).is_none());
    assert!(tree.cached_children(&nested).is_none());
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
