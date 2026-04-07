use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    source: String,
}

impl MarkdownDocument {
    pub fn parse(source: &str) -> Self {
        Self {
            source: source.to_owned(),
        }
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn show(&self, ui: &mut egui::Ui, cache: &mut CommonMarkCache) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                CommonMarkViewer::new().show(ui, cache, &self.source);
            });
    }
}

pub fn render_markdown(ui: &mut egui::Ui, cache: &mut CommonMarkCache, source: &str) {
    MarkdownDocument::parse(source).show(ui, cache);
}
