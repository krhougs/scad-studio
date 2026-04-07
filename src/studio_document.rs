use scad_ui::tab_system::TabId;

use crate::{
    document_session::{DocumentDescriptor, DocumentKind},
    markdown_tab::MarkdownTab,
    viewer_tab::ViewerTab,
};

pub enum StudioDocumentSession {
    Viewer(ViewerTab),
    Markdown(MarkdownTab),
}

impl StudioDocumentSession {
    pub fn descriptor(&self) -> DocumentDescriptor {
        match self {
            Self::Viewer(viewer) => {
                DocumentDescriptor::new(DocumentKind::Viewer, viewer.path().to_path_buf())
            }
            Self::Markdown(markdown) => {
                DocumentDescriptor::new(DocumentKind::Markdown, markdown.path().to_path_buf())
            }
        }
    }

    pub fn legacy_tab_id(&self) -> TabId {
        match self {
            Self::Viewer(viewer) => viewer.legacy_tab_id(),
            Self::Markdown(markdown) => markdown.legacy_tab_id(),
        }
    }

    pub fn as_viewer(&self) -> Option<&ViewerTab> {
        match self {
            Self::Viewer(viewer) => Some(viewer),
            Self::Markdown(_) => None,
        }
    }

    pub fn as_viewer_mut(&mut self) -> Option<&mut ViewerTab> {
        match self {
            Self::Viewer(viewer) => Some(viewer),
            Self::Markdown(_) => None,
        }
    }

    pub fn as_markdown_mut(&mut self) -> Option<&mut MarkdownTab> {
        match self {
            Self::Viewer(_) => None,
            Self::Markdown(markdown) => Some(markdown),
        }
    }
}
