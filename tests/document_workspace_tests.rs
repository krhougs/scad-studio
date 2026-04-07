#![allow(dead_code)]

#[path = "../src/document_session.rs"]
mod document_session;
#[path = "../src/document_workspace.rs"]
mod document_workspace;

use document_session::{DocumentDescriptor, DocumentKind};
use document_workspace::{DocumentOpenOutcome, DocumentSlot, DocumentWorkspace};
use std::path::PathBuf;

#[test]
fn opening_new_document_activates_it_and_preserves_order() {
    let mut workspace = DocumentWorkspace::default();
    let alpha = slot("/tmp/alpha/model.scad", DocumentKind::Viewer);
    let beta = slot("/tmp/beta/readme.md", DocumentKind::Markdown);

    assert_eq!(
        workspace.open_or_activate(alpha),
        DocumentOpenOutcome::Opened
    );
    assert_eq!(
        workspace.open_or_activate(beta),
        DocumentOpenOutcome::Opened
    );

    assert_eq!(
        workspace
            .tabs()
            .into_iter()
            .map(|tab| tab.title)
            .collect::<Vec<_>>(),
        vec!["model.scad".to_string(), "readme.md".to_string()]
    );
    assert_eq!(
        workspace.active_key(),
        Some(
            DocumentDescriptor::new(DocumentKind::Markdown, PathBuf::from("/tmp/beta/readme.md"))
                .key
        ),
    );
}

#[test]
fn opening_same_document_twice_reuses_existing_session() {
    let mut workspace = DocumentWorkspace::default();
    let key =
        DocumentDescriptor::new(DocumentKind::Viewer, PathBuf::from("/tmp/demo/model.scad")).key;

    assert_eq!(
        workspace.open_or_activate(slot("/tmp/demo/model.scad", DocumentKind::Viewer)),
        DocumentOpenOutcome::Opened
    );
    assert_eq!(
        workspace.open_or_activate(slot("/tmp/demo/model.scad", DocumentKind::Viewer)),
        DocumentOpenOutcome::ActivatedExisting
    );

    assert_eq!(workspace.tabs().len(), 1);
    assert_eq!(workspace.active_key(), Some(key));
}

#[test]
fn setting_active_document_changes_focus_before_close_routing() {
    let mut workspace = DocumentWorkspace::default();
    workspace.open_or_activate(slot("/tmp/alpha/model.scad", DocumentKind::Viewer));
    workspace.open_or_activate(slot("/tmp/beta/readme.md", DocumentKind::Markdown));
    workspace.open_or_activate(slot("/tmp/gamma/notes.md", DocumentKind::Markdown));

    let alpha_key =
        DocumentDescriptor::new(DocumentKind::Viewer, PathBuf::from("/tmp/alpha/model.scad")).key;
    let beta_key =
        DocumentDescriptor::new(DocumentKind::Markdown, PathBuf::from("/tmp/beta/readme.md")).key;
    let gamma_key =
        DocumentDescriptor::new(DocumentKind::Markdown, PathBuf::from("/tmp/gamma/notes.md")).key;

    workspace.set_active(alpha_key.clone());
    assert_eq!(workspace.active_key(), Some(alpha_key.clone()));

    let closed = workspace.close(&alpha_key);

    assert!(closed.is_some());
    assert_eq!(workspace.active_key(), Some(beta_key.clone()));

    workspace.close(&gamma_key);
    assert_eq!(workspace.active_key(), Some(beta_key));
}

#[test]
fn conflicting_file_names_receive_short_parent_suffixes() {
    let mut workspace = DocumentWorkspace::default();
    workspace.open_or_activate(slot(
        "/tmp/workspace/docs/README.md",
        DocumentKind::Markdown,
    ));
    workspace.open_or_activate(slot(
        "/tmp/workspace/guides/README.md",
        DocumentKind::Markdown,
    ));

    let titles = workspace
        .tabs()
        .into_iter()
        .map(|tab| tab.title)
        .collect::<Vec<_>>();

    assert_eq!(
        titles,
        vec![
            "README.md · docs/".to_string(),
            "README.md · guides/".to_string(),
        ]
    );
}

fn slot(path: &str, kind: DocumentKind) -> DocumentSlot<()> {
    DocumentSlot::new(DocumentDescriptor::new(kind, PathBuf::from(path)), ())
}
