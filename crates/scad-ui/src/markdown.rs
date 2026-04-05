use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::{
    theme::{self, palette},
    widgets::section_header,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownDocument {
    blocks: Vec<MarkdownBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    Heading {
        level: u8,
        content: MarkdownInlineContent,
    },
    Paragraph(MarkdownInlineContent),
    List {
        kind: MarkdownListKind,
        items: Vec<MarkdownInlineContent>,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownListKind {
    Unordered,
    Ordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownInline {
    Text(String),
    Emphasis(String),
    Strong(String),
    StrongEmphasis(String),
    Code(String),
    Link { text: String, url: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarkdownInlineContent {
    spans: Vec<MarkdownInline>,
}

impl MarkdownInlineContent {
    pub fn plain_text(&self) -> String {
        self.spans
            .iter()
            .map(MarkdownInline::plain_text)
            .collect::<Vec<_>>()
            .join("")
    }

    pub fn iter(&self) -> std::slice::Iter<'_, MarkdownInline> {
        self.spans.iter()
    }
}

impl MarkdownInline {
    fn plain_text(&self) -> String {
        match self {
            MarkdownInline::Text(text)
            | MarkdownInline::Emphasis(text)
            | MarkdownInline::Strong(text)
            | MarkdownInline::StrongEmphasis(text)
            | MarkdownInline::Code(text) => text.clone(),
            MarkdownInline::Link { text, .. } => text.clone(),
        }
    }
}

impl MarkdownDocument {
    pub fn parse(source: &str) -> Self {
        parse_document(source)
    }

    pub fn blocks(&self) -> &[MarkdownBlock] {
        &self.blocks
    }

    pub fn show(&self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                section_header(ui, "markdown");
                for block in &self.blocks {
                    render_block(ui, block);
                    ui.add_space(8.0);
                }
            });
    }
}

pub fn render_markdown(ui: &mut egui::Ui, source: &str) {
    MarkdownDocument::parse(source).show(ui);
}

fn parse_document(source: &str) -> MarkdownDocument {
    let options = Options::empty();
    let parser = Parser::new_ext(source, options);
    let mut blocks = Vec::new();
    let mut heading: Option<(u8, InlineCollector)> = None;
    let mut paragraph: Option<InlineCollector> = None;
    let mut code_block: Option<(Option<String>, String)> = None;
    let mut list: Option<(
        MarkdownListKind,
        Vec<MarkdownInlineContent>,
        Option<InlineCollector>,
    )> = None;

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    heading = Some((heading_level(level), InlineCollector::default()))
                }
                Tag::Paragraph => {
                    if list
                        .as_ref()
                        .and_then(|(_, _, current)| current.as_ref())
                        .is_none()
                    {
                        paragraph = Some(InlineCollector::default());
                    }
                }
                Tag::List(start) => {
                    list = Some((
                        if start.is_some() {
                            MarkdownListKind::Ordered
                        } else {
                            MarkdownListKind::Unordered
                        },
                        Vec::new(),
                        None,
                    ));
                }
                Tag::Item => {
                    if let Some((_, _, current)) = list.as_mut() {
                        *current = Some(InlineCollector::default());
                    }
                }
                Tag::CodeBlock(kind) => {
                    code_block = Some((code_language(&kind), String::new()));
                }
                Tag::Emphasis => {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).start_emphasis()
                }
                Tag::Strong => {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).start_strong()
                }
                Tag::Link { dest_url, .. } => {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list)
                        .start_link(dest_url.to_string());
                }
                _ => {}
            },
            Event::End(tag_end) => match tag_end {
                TagEnd::Heading(_) => {
                    if let Some((level, collector)) = heading.take()
                        && !collector.is_empty()
                    {
                        blocks.push(MarkdownBlock::Heading {
                            level,
                            content: collector.finish(),
                        });
                    }
                }
                TagEnd::Paragraph => {
                    if let Some(collector) = paragraph.take()
                        && !collector.is_empty()
                    {
                        blocks.push(MarkdownBlock::Paragraph(collector.finish()));
                    }
                }
                TagEnd::List(_) => {
                    if let Some((kind, items, current)) = list.take() {
                        let mut items = items;
                        if let Some(collector) = current
                            && !collector.is_empty()
                        {
                            items.push(collector.finish());
                        }
                        blocks.push(MarkdownBlock::List { kind, items });
                    }
                }
                TagEnd::Item => {
                    if let Some((_, items, current)) = list.as_mut()
                        && let Some(collector) = current.take()
                        && !collector.is_empty()
                    {
                        items.push(collector.finish());
                    }
                }
                TagEnd::CodeBlock => {
                    if let Some((language, content)) = code_block.take() {
                        blocks.push(MarkdownBlock::CodeBlock { language, content });
                    }
                }
                TagEnd::Emphasis => {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).end_emphasis()
                }
                TagEnd::Strong => {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).end_strong()
                }
                TagEnd::Link => {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).end_link()
                }
                _ => {}
            },
            Event::Text(text) => {
                if let Some(content) = code_block.as_mut() {
                    content.1.push_str(&text);
                } else {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).push_text(&text);
                }
            }
            Event::Code(text) => {
                active_collector_mut(&mut heading, &mut paragraph, &mut list).push_code(&text);
            }
            Event::SoftBreak => {
                if let Some(content) = code_block.as_mut() {
                    content.1.push('\n');
                } else {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).push_text(" ");
                }
            }
            Event::HardBreak => {
                if let Some(content) = code_block.as_mut() {
                    content.1.push('\n');
                } else {
                    active_collector_mut(&mut heading, &mut paragraph, &mut list).push_text("\n");
                }
            }
            _ => {}
        }
    }

    MarkdownDocument { blocks }
}

fn active_collector_mut<'a>(
    heading: &'a mut Option<(u8, InlineCollector)>,
    paragraph: &'a mut Option<InlineCollector>,
    list: &'a mut Option<(
        MarkdownListKind,
        Vec<MarkdownInlineContent>,
        Option<InlineCollector>,
    )>,
) -> &'a mut InlineCollector {
    if let Some((_, collector)) = heading.as_mut() {
        return collector;
    }
    if let Some(collector) = paragraph.as_mut() {
        return collector;
    }
    if let Some((_, _, current)) = list.as_mut() {
        return current.get_or_insert_with(InlineCollector::default);
    }
    panic!("inline content without active collector")
}

fn heading_level(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn code_language(kind: &CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Indented => None,
        CodeBlockKind::Fenced(info) if info.is_empty() => None,
        CodeBlockKind::Fenced(info) => Some(info.to_string()),
    }
}

fn render_block(ui: &mut egui::Ui, block: &MarkdownBlock) {
    match block {
        MarkdownBlock::Heading { level, content } => {
            let size = match level {
                1 => 24.0,
                2 => 20.0,
                3 => 18.0,
                4 => 16.0,
                5 => 14.5,
                _ => 13.5,
            };
            ui.label(
                egui::RichText::new(content.plain_text())
                    .size(size)
                    .strong()
                    .color(palette::TEXT_BRIGHT),
            );
        }
        MarkdownBlock::Paragraph(content) => {
            ui.label(inline_job(content, ui.available_width()));
        }
        MarkdownBlock::List { kind, items } => {
            for (index, item) in items.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.add_space(8.0);
                    let bullet = match kind {
                        MarkdownListKind::Unordered => "•".to_string(),
                        MarkdownListKind::Ordered => format!("{}.", index + 1),
                    };
                    ui.label(egui::RichText::new(bullet).color(palette::TEXT_SECONDARY));
                    ui.add_space(6.0);
                    ui.label(inline_job(item, ui.available_width()));
                });
            }
        }
        MarkdownBlock::CodeBlock { language, content } => {
            theme::floating_frame(1.0).show(ui, |ui| {
                if let Some(language) = language {
                    ui.label(
                        egui::RichText::new(language)
                            .size(10.0)
                            .color(palette::TEXT_SECONDARY),
                    );
                }
                ui.label(
                    egui::RichText::new(content)
                        .monospace()
                        .color(palette::TEXT_PRIMARY),
                );
            });
        }
    }
}

fn inline_job(content: &MarkdownInlineContent, width: f32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};

    let mut job = LayoutJob::default();
    job.wrap.max_width = width.max(1.0);
    for span in content.iter() {
        let (text, format) = match span {
            MarkdownInline::Text(text) => (
                text,
                TextFormat {
                    font_id: egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    color: palette::TEXT_PRIMARY,
                    ..Default::default()
                },
            ),
            MarkdownInline::Emphasis(text) => (
                text,
                TextFormat {
                    font_id: egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    color: palette::TEXT_PRIMARY,
                    italics: true,
                    ..Default::default()
                },
            ),
            MarkdownInline::Strong(text) => (
                text,
                TextFormat {
                    font_id: egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    color: palette::TEXT_BRIGHT,
                    ..Default::default()
                },
            ),
            MarkdownInline::StrongEmphasis(text) => (
                text,
                TextFormat {
                    font_id: egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    color: palette::TEXT_BRIGHT,
                    italics: true,
                    ..Default::default()
                },
            ),
            MarkdownInline::Code(text) => (
                text,
                TextFormat {
                    font_id: egui::FontId::new(13.0, egui::FontFamily::Monospace),
                    color: palette::TEXT_BRIGHT,
                    background: palette::BG_WIDGET,
                    ..Default::default()
                },
            ),
            MarkdownInline::Link { text, .. } => (
                text,
                TextFormat {
                    font_id: egui::FontId::new(13.0, egui::FontFamily::Proportional),
                    color: palette::TEXT_ACCENT,
                    ..Default::default()
                },
            ),
        };
        job.append(text, 0.0, format);
    }
    job
}

#[derive(Default)]
struct InlineCollector {
    spans: Vec<MarkdownInline>,
    buffer: String,
    strong_depth: usize,
    emphasis_depth: usize,
    link: Option<String>,
}

impl InlineCollector {
    fn start_strong(&mut self) {
        self.flush();
        self.strong_depth += 1;
    }

    fn end_strong(&mut self) {
        self.flush();
        self.strong_depth = self.strong_depth.saturating_sub(1);
    }

    fn start_emphasis(&mut self) {
        self.flush();
        self.emphasis_depth += 1;
    }

    fn end_emphasis(&mut self) {
        self.flush();
        self.emphasis_depth = self.emphasis_depth.saturating_sub(1);
    }

    fn start_link(&mut self, url: String) {
        self.flush();
        self.link = Some(url);
    }

    fn end_link(&mut self) {
        self.flush();
        self.link = None;
    }

    fn push_text(&mut self, text: &str) {
        self.buffer.push_str(text);
    }

    fn push_code(&mut self, text: &str) {
        self.flush();
        self.spans.push(MarkdownInline::Code(text.to_string()));
    }

    fn finish(mut self) -> MarkdownInlineContent {
        self.flush();
        MarkdownInlineContent { spans: self.spans }
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty() && self.spans.is_empty()
    }

    fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.buffer);
        let span = if let Some(url) = self.link.clone() {
            MarkdownInline::Link { text, url }
        } else if self.strong_depth > 0 && self.emphasis_depth > 0 {
            MarkdownInline::StrongEmphasis(text)
        } else if self.strong_depth > 0 {
            MarkdownInline::Strong(text)
        } else if self.emphasis_depth > 0 {
            MarkdownInline::Emphasis(text)
        } else {
            MarkdownInline::Text(text)
        };
        self.spans.push(span);
    }
}
