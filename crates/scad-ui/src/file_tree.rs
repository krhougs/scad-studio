use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::{
    theme::palette,
    widgets::{section_header, small_button},
};

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

    pub fn ensure_children(&mut self, path: &Path) -> io::Result<&[FileTreeEntry]> {
        if !self.children_cache.contains_key(path) {
            let entries = read_children(path)?;
            self.children_cache.insert(path.to_path_buf(), entries);
        }
        Ok(self
            .children_cache
            .get(path)
            .expect("cache entry should exist"))
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<FileTreeAction> {
        let mut action = None;
        egui::Frame::default()
            .fill(palette::BG_PANEL)
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                section_header(ui, "files");
                action = self.show_dir(ui, self.root.clone(), 0);
            });
        action
    }

    fn show_dir(
        &mut self,
        ui: &mut egui::Ui,
        dir: PathBuf,
        depth: usize,
    ) -> Option<FileTreeAction> {
        let mut action = None;
        let indent = 14.0 * depth as f32;
        let is_expanded = self.expanded.contains(&dir);
        let label = dir
            .file_name()
            .and_then(|name| name.to_str())
            .map_or_else(|| dir.display().to_string(), ToOwned::to_owned);

        ui.horizontal(|ui| {
            ui.add_space(indent);
            if small_button(ui, if is_expanded { "▼" } else { "▶" }).clicked() {
                self.toggle(&dir);
            }
            let selected = self.selected.as_ref() == Some(&dir);
            let response = ui.selectable_label(selected, label);
            if response.clicked() {
                self.selected = Some(dir.clone());
                action = Some(FileTreeAction::Select(dir.clone()));
            }
        });

        if is_expanded && let Ok(children) = self.ensure_children(&dir) {
            let children = children.to_vec();
            for child in children {
                match child.kind {
                    FileTreeEntryKind::Directory => {
                        if let Some(child_action) = self.show_dir(ui, child.path.clone(), depth + 1)
                        {
                            action = Some(child_action);
                        }
                    }
                    FileTreeEntryKind::File => {
                        if let Some(file_action) = self.show_file(ui, &child, depth + 1) {
                            action = Some(file_action);
                        }
                    }
                }
            }
        }

        action
    }

    fn show_file(
        &mut self,
        ui: &mut egui::Ui,
        entry: &FileTreeEntry,
        depth: usize,
    ) -> Option<FileTreeAction> {
        let indent = 14.0 * depth as f32;
        let selected = self.selected.as_ref() == Some(&entry.path);
        let color = file_color(&entry.path);
        let mut action = None;

        ui.horizontal(|ui| {
            ui.add_space(indent + 16.0);
            let label = egui::RichText::new(&entry.name).color(color);
            let response = ui.selectable_label(selected, label);
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

fn read_children(path: &Path) -> io::Result<Vec<FileTreeEntry>> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let kind = if file_type.is_dir() {
            FileTreeEntryKind::Directory
        } else {
            FileTreeEntryKind::File
        };
        let name = entry.file_name().to_string_lossy().to_string();
        entries.push(FileTreeEntry {
            name,
            path: entry.path(),
            kind,
        });
    }
    entries.sort_by_key(sort_key);
    Ok(entries)
}

fn sort_key(entry: &FileTreeEntry) -> (u8, String, String) {
    let kind_rank = match entry.kind {
        FileTreeEntryKind::Directory => 0,
        FileTreeEntryKind::File => 1,
    };
    (kind_rank, entry.name.to_lowercase(), entry.name.clone())
}

fn path_relation_matches(base: &Path, candidate: &Path) -> bool {
    candidate == base || candidate.starts_with(base) || base.starts_with(candidate)
}

fn file_color(path: &Path) -> egui::Color32 {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
    {
        Some(ext) if ext == "scad" => palette::TEXT_ACCENT,
        Some(ext) if ext == "md" || ext == "markdown" => palette::TEXT_BRIGHT,
        _ => palette::TEXT_SECONDARY,
    }
}
