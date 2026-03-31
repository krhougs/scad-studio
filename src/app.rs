use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum RenderState {
    Idle,
    Rendering(String),
    Ready(String),
    Error(String),
}

#[derive(Debug, Default, Clone)]
pub struct UiActions {
    pub open_file: bool,
}

#[derive(Debug, Default)]
pub struct StudioApp {
    current_file: Option<PathBuf>,
    render_state: RenderState,
}

impl StudioApp {
    pub fn current_file(&self) -> Option<&Path> {
        self.current_file.as_deref()
    }

    pub fn set_current_file(&mut self, path: PathBuf) {
        self.current_file = Some(path);
    }

    pub fn set_rendering(&mut self, message: impl Into<String>) {
        self.render_state = RenderState::Rendering(message.into());
    }

    pub fn set_ready(&mut self, message: impl Into<String>) {
        self.render_state = RenderState::Ready(message.into());
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.render_state = RenderState::Error(message.into());
    }

    pub fn ui(&mut self, ctx: &egui::Context, show_embedded_menu: bool) -> UiActions {
        let mut actions = UiActions::default();
        if show_embedded_menu {
            self.show_menu(ctx, &mut actions);
        }
        self.show_status_bar(ctx);
        actions
    }

    fn show_menu(&self, ctx: &egui::Context, actions: &mut UiActions) {
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        actions.open_file = true;
                        ui.close();
                    }
                });
            });
        });
    }

    fn show_status_bar(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let file_label = self
                    .current_file
                    .as_ref()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("未打开文件");
                ui.label(format!("文件: {file_label}"));
                ui.separator();
                ui.label(self.status_message());
            });
        });
    }

    fn status_message(&self) -> &str {
        match &self.render_state {
            RenderState::Idle => "等待打开 .scad 文件",
            RenderState::Rendering(message) => message,
            RenderState::Ready(message) => message,
            RenderState::Error(message) => message,
        }
    }
}

impl Default for RenderState {
    fn default() -> Self {
        Self::Idle
    }
}
