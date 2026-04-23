use std::path::{Path, PathBuf};

use egui::{Sense, Stroke, pos2};

use crate::{
    document_tabs::{self, DocumentTabKind},
    rail_style,
    theme::palette,
};

const INDENT: f32 = 12.0;
const CHEVRON_COL_W: f32 = 17.0;
const TREE_ROW_SPACING_X: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: FileTreeEntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileTreeAction {
    Select(PathBuf),
    OpenFile(PathBuf),
    LoadDirectory(PathBuf),
}

#[derive(Debug, Clone)]
pub struct FileTree {
    root: PathBuf,
    expanded: std::collections::HashSet<PathBuf>,
    selected: Option<PathBuf>,
    children_cache: std::collections::HashMap<PathBuf, Vec<FileTreeEntry>>,
}

impl FileTree {
    pub fn new(root: PathBuf) -> Self {
        let mut expanded = std::collections::HashSet::new();
        expanded.insert(root.clone());
        Self {
            root,
            expanded,
            selected: None,
            children_cache: std::collections::HashMap::new(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn selected(&self) -> Option<&Path> {
        self.selected.as_deref()
    }

    pub fn set_selected<P: Into<PathBuf>>(&mut self, path: P) {
        self.selected = Some(path.into());
    }

    pub fn toggle(&mut self, path: &Path) {
        if !self.expanded.remove(path) {
            self.expanded.insert(path.to_path_buf());
        }
    }

    pub fn expand(&mut self, path: &Path) {
        self.expanded.insert(path.to_path_buf());
    }

    pub fn collapse(&mut self, path: &Path) {
        self.expanded.remove(path);
    }

    pub fn invalidate(&mut self, path: &Path) {
        self.children_cache
            .retain(|cached, _| !path_relation_matches(path, cached));
    }

    pub fn set_children(&mut self, path: PathBuf, entries: Vec<FileTreeEntry>) {
        self.children_cache.insert(path, entries);
    }

    pub fn cached_children(&self, path: &Path) -> Option<&[FileTreeEntry]> {
        self.children_cache.get(path).map(Vec::as_slice)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<FileTreeAction> {
        self.show_dir(ui, self.root.clone(), 0, &[])
    }

    fn show_dir(
        &mut self,
        ui: &mut egui::Ui,
        dir: PathBuf,
        depth: usize,
        stack: &[bool],
    ) -> Option<FileTreeAction> {
        debug_assert_eq!(stack.len(), depth);
        let mut action = None;
        let row_h = row_height(ui);
        let is_expanded = self.expanded.contains(&dir);
        let label = dir
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| dir.display().to_string(), ToOwned::to_owned);

        let selected = self.selected.as_ref() == Some(&dir);
        let mut folder_row_clicked = false;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TREE_ROW_SPACING_X;
            let guides_clicked = draw_indent_guides(ui, depth, stack, row_h, Sense::click());
            let chevron = expand_toggle(ui, is_expanded, row_h);
            let label_response =
                document_tabs::show_document_tab_inner_row_sized(ui, &label, selected, None, row_h);
            if guides_clicked || chevron.clicked() || label_response.clicked() {
                folder_row_clicked = true;
                self.selected = Some(dir.clone());
                action = Some(FileTreeAction::Select(dir.clone()));
            }
        });
        if folder_row_clicked {
            self.toggle(&dir);
        }

        if is_expanded {
            if let Some(entries) = self.children_cache.get(&dir).cloned() {
                if let Some(child_action) = self.show_children(ui, &entries, depth, stack) {
                    action = Some(child_action);
                }
            } else {
                action = Some(FileTreeAction::LoadDirectory(dir));
            }
        }

        action
    }

    fn show_children(
        &mut self,
        ui: &mut egui::Ui,
        children: &[FileTreeEntry],
        depth: usize,
        stack: &[bool],
    ) -> Option<FileTreeAction> {
        let mut action = None;
        let n = children.len();
        for (i, child) in children.iter().enumerate() {
            let mut next_stack = stack.to_vec();
            next_stack.push(i + 1 < n);
            let child_action = match child.kind {
                FileTreeEntryKind::Directory => {
                    self.show_dir(ui, child.path.clone(), depth + 1, &next_stack)
                }
                FileTreeEntryKind::File => self.show_file(ui, child, depth + 1, &next_stack),
            };
            if child_action.is_some() {
                action = child_action;
            }
        }
        action
    }

    fn show_file(
        &mut self,
        ui: &mut egui::Ui,
        entry: &FileTreeEntry,
        depth: usize,
        stack: &[bool],
    ) -> Option<FileTreeAction> {
        debug_assert_eq!(stack.len(), depth);
        let selected = self.selected.as_ref() == Some(&entry.path);
        let mut action = None;
        let row_h = row_height(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = TREE_ROW_SPACING_X;
            draw_indent_guides(ui, depth, stack, row_h, Sense::hover());
            ui.allocate_exact_size(egui::vec2(CHEVRON_COL_W, row_h), Sense::hover());
            let kind = supported_document_tab_kind(&entry.path);
            let response = document_tabs::show_document_tab_inner_row_sized(
                ui,
                &entry.name,
                selected,
                kind,
                row_h,
            );
            if response.clicked() {
                self.selected = Some(entry.path.clone());
                action = Some(FileTreeAction::Select(entry.path.clone()));
            }
            if response.double_clicked() {
                action = Some(FileTreeAction::OpenFile(entry.path.clone()));
            }
        });
        action
    }
}

fn row_height(_ui: &egui::Ui) -> f32 {
    rail_style::content_height()
}

fn draw_indent_guides(
    ui: &mut egui::Ui,
    depth: usize,
    stack: &[bool],
    row_h: f32,
    sense: Sense,
) -> bool {
    let mut any_clicked = false;
    let stroke = Stroke::new(1.0, palette::STROKE_DIM);
    for k in 0..depth {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(INDENT, row_h), sense);
        if response.clicked() {
            any_clicked = true;
        }
        if stack[k] {
            let cx = rect.center().x;
            ui.painter().line_segment(
                [egui::pos2(cx, rect.top()), egui::pos2(cx, rect.bottom())],
                stroke,
            );
        }
    }
    any_clicked
}

fn expand_toggle(ui: &mut egui::Ui, expanded: bool, row_h: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(CHEVRON_COL_W, row_h), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    paint_tree_chevron(ui, rect, expanded, response.hovered());
    response
}

fn paint_tree_chevron(ui: &egui::Ui, rect: egui::Rect, expanded: bool, hovered: bool) {
    let c = rect.center();
    let color = if hovered {
        palette::TEXT_PRIMARY
    } else {
        palette::TEXT_SECONDARY
    };
    let stroke = Stroke::new(1.1, color);
    let hh = (rect.height() * 0.19).clamp(2.8, 4.0);
    let hw = (rect.width() * 0.32).clamp(2.8, 4.2);
    let painter = ui.painter();

    if expanded {
        let tip = pos2(c.x, c.y + hh * 0.55);
        let left = pos2(c.x - hw, c.y - hh * 0.2);
        let right = pos2(c.x + hw, c.y - hh * 0.2);
        painter.line_segment([left, tip], stroke);
        painter.line_segment([right, tip], stroke);
    } else {
        let tip = pos2(c.x + hw * 0.5, c.y);
        let top = pos2(c.x - hw * 0.35, c.y - hh);
        let bot = pos2(c.x - hw * 0.35, c.y + hh);
        painter.line_segment([top, tip], stroke);
        painter.line_segment([bot, tip], stroke);
    }
}

/// 与文档标签左侧类型块对应的扩展名：支持的模型、Markdown、常见栅格图在树行显示类型块。
pub fn supported_document_tab_kind(path: &Path) -> Option<DocumentTabKind> {
    let ext = path.extension()?.to_str()?;
    match ext.to_ascii_lowercase().as_str() {
        "scad" => Some(DocumentTabKind::Viewer),
        "md" | "markdown" => Some(DocumentTabKind::Markdown),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ico" => {
            Some(DocumentTabKind::Image)
        }
        _ => None,
    }
}

fn path_relation_matches(base: &Path, candidate: &Path) -> bool {
    candidate == base || candidate.starts_with(base) || base.starts_with(candidate)
}
