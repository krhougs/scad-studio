pub use app_server_core::{LogEntry, LogLevel, SlicerInstall};
use scad_scene::{CameraMatrices, OrbitalCamera, RenderSettings};
use std::path::{Path, PathBuf};
use studio_common::{AppConfig, DocumentState};

pub use scad_scene::{ColorMode, ProjectionMode, RenderMode};

#[derive(Debug, Clone, Default)]
pub enum RenderState {
    #[default]
    Idle,
    Rendering(String),
    Ready(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewerState {
    pub render_mode: RenderMode,
    pub color_mode: ColorMode,
    pub projection_mode: ProjectionMode,
    pub wireframe_supported: bool,
    pub show_grid: bool,
    pub show_build_plate: bool,
    pub show_axis_gizmo: bool,
    pub shadows_enabled: bool,
    pub fog_enabled: bool,
    pub clip_plane_enabled: bool,
    pub side_panel_open: bool,
    pub log_panel_open: bool,
    pub camera_overlay_open: bool,
}

#[derive(Debug, Clone)]
pub enum CameraAction {
    SetTargetX(f32),
    SetTargetY(f32),
    SetTargetZ(f32),
    SetDistance(f32),
    SetAzimuth(f32),
    SetElevation(f32),
    ResetView,
    ViewTop,
    ViewBottom,
    ViewFront,
    ViewBack,
    ViewLeft,
    ViewRight,
}

#[derive(Debug, Default, Clone)]
pub struct UiActions {
    pub open_file: bool,
    pub viewer_state_changed: bool,
    pub commands: Vec<UiCommand>,
    pub camera_action: Option<CameraAction>,
}

pub struct UiFrame<'a> {
    pub document: &'a mut DocumentState,
    pub config: &'a mut AppConfig,
    pub settings_open: &'a mut bool,
    pub slicers: &'a [SlicerInstall],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiCommand {
    SavePreset(String),
    DeletePreset(String),
    ExportModel,
    SendToSlicer(String),
    SaveSettings,
}

#[derive(Debug, Default)]
pub struct StudioApp {
    current_file: Option<PathBuf>,
    render_state: RenderState,
    viewer_state: ViewerState,
    logs: Vec<LogEntry>,
}

impl StudioApp {
    #[allow(dead_code)]
    pub fn current_file(&self) -> Option<&Path> {
        self.current_file.as_deref()
    }

    pub fn has_current_file(&self) -> bool {
        self.current_file.is_some()
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

    pub fn viewer_state(&self) -> &ViewerState {
        &self.viewer_state
    }

    pub fn viewer_state_mut(&mut self) -> &mut ViewerState {
        &mut self.viewer_state
    }

    pub fn log_entries(&self) -> &[LogEntry] {
        &self.logs
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    pub fn push_log(&mut self, level: LogLevel, message: impl Into<String>) {
        if level == LogLevel::Error {
            self.viewer_state.log_panel_open = true;
        }
        self.logs.push(LogEntry {
            level,
            message: message.into(),
        });
    }

    pub fn current_file_label(&self) -> &str {
        self.current_file
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("未打开文件")
    }

    pub fn is_rendering(&self) -> bool {
        matches!(self.render_state, RenderState::Rendering(_))
    }

    pub fn status_message(&self) -> &str {
        match &self.render_state {
            RenderState::Idle => "等待打开 .scad 文件",
            RenderState::Rendering(message) => message,
            RenderState::Ready(message) => message,
            RenderState::Error(message) => message,
        }
    }

    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        show_embedded_menu: bool,
        camera_matrices: CameraMatrices,
        camera: &OrbitalCamera,
        frame: UiFrame<'_>,
    ) -> UiActions {
        crate::ui::show_app(
            self,
            ctx,
            show_embedded_menu,
            camera_matrices,
            camera,
            frame,
        )
    }

    #[allow(dead_code)]
    pub fn embedded_ui(
        &mut self,
        ctx: &egui::Context,
        camera_matrices: CameraMatrices,
        camera: &OrbitalCamera,
        frame: UiFrame<'_>,
    ) -> UiActions {
        crate::ui::show_embedded_app(self, ctx, camera_matrices, camera, frame)
    }
}

impl ViewerState {
    pub fn render_settings(&self) -> RenderSettings {
        RenderSettings {
            render_mode: self.render_mode,
            color_mode: self.color_mode,
            show_grid: self.show_grid,
            show_build_plate: self.show_build_plate,
            shadows_enabled: self.shadows_enabled,
            fog_enabled: self.fog_enabled,
        }
    }

    pub fn toggle_side_panel(&mut self) {
        self.side_panel_open = !self.side_panel_open;
    }

    pub fn toggle_log_panel(&mut self) {
        self.log_panel_open = !self.log_panel_open;
    }
}

impl Default for ViewerState {
    fn default() -> Self {
        Self {
            render_mode: RenderMode::Solid,
            color_mode: ColorMode::Color,
            projection_mode: ProjectionMode::Perspective,
            wireframe_supported: false,
            show_grid: true,
            show_build_plate: false,
            show_axis_gizmo: true,
            shadows_enabled: false,
            fog_enabled: false,
            clip_plane_enabled: false,
            side_panel_open: true,
            log_panel_open: false,
            camera_overlay_open: true,
        }
    }
}
