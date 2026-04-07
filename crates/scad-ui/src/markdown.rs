use std::sync::Arc;

use egui::{FontId, TextStyle, UiBuilder};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

/// 与常见网页正文 `max-width` 相当的阅读栏宽度（屏幕点），宽屏时居中留白。
const MARKDOWN_MAX_CONTENT_WIDTH: f32 = 720.0;

/// 阅读区内边距：水平与垂直（`Margin` 为 i8，与常见文章页 padding 量级一致）。
const MARKDOWN_INNER_MARGIN_H: i8 = 24;
const MARKDOWN_INNER_MARGIN_V: i8 = 20;

/// 正文字号（pt），与 egui 默认 `Body` 一致，阅读区略紧凑。
const MARKDOWN_BODY_PT: f32 = 14.0;
/// 一级标题使用的 `TextStyle::Heading` 字号上界；`egui_commonmark` 用其与正文插值得到 H2–H6。
const MARKDOWN_HEADING_MAX_PT: f32 = 26.0;
/// 行内代码与代码块等宽字号（pt）。
const MARKDOWN_CODE_PT: f32 = 12.5;
/// 脚注等次要文字。
const MARKDOWN_SMALL_PT: f32 = 12.0;

/// 嵌套列表每级缩进空格数（渲染器选项，略小于默认 4 以接近常见网页列表缩进观感）。
const MARKDOWN_LIST_INDENT_SPACES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    source: String,
}

/// 基于当前主题生成 Markdown 阅读区专用字号刻度（正文、标题上界、等宽、脚注）。
pub fn reading_typography_style(base: &egui::Style) -> egui::Style {
    let mut style = base.clone();
    let body_font = TextStyle::Body.resolve(base);
    let mono_font = TextStyle::Monospace.resolve(base);
    let prose_family = body_font.family.clone();
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(MARKDOWN_BODY_PT, prose_family.clone()),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(MARKDOWN_HEADING_MAX_PT, prose_family.clone()),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(MARKDOWN_CODE_PT, mono_font.family),
    );
    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(MARKDOWN_SMALL_PT, prose_family),
    );
    style
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
                ui.vertical_centered(|ui| {
                    let column_width = MARKDOWN_MAX_CONTENT_WIDTH.min(ui.available_width());
                    ui.set_max_width(column_width);
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(
                            MARKDOWN_INNER_MARGIN_H,
                            MARKDOWN_INNER_MARGIN_V,
                        ))
                        .show(ui, |ui| {
                            ui.scope_builder(
                                UiBuilder::new()
                                    .id_salt("scad_markdown_reading")
                                    .style(Arc::new(reading_typography_style(ui.style().as_ref()))),
                                |ui| {
                                    CommonMarkViewer::new()
                                        .indentation_spaces(MARKDOWN_LIST_INDENT_SPACES)
                                        .show(ui, cache, &self.source);
                                },
                            );
                        });
                });
            });
    }
}

pub fn render_markdown(ui: &mut egui::Ui, cache: &mut CommonMarkCache, source: &str) {
    MarkdownDocument::parse(source).show(ui, cache);
}
