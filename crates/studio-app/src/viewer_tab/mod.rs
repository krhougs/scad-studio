mod input;
mod io;

use std::{
    any::Any,
    path::{Path, PathBuf},
};

use glam::{Vec2, Vec4Swizzles};
use scad_scene::{
    Bounds, CameraInteraction, ClipPlane, EditMode, MeshData, OrbitalCamera, RenderSettings,
};
use scad_ui::{
    tab_system::{TabContext, TabId, WorkTab},
    theme, viewer_camera, viewer_viewport,
};
use scad_viewer::app::{
    CameraAction, LogLevel, SlicerInstall, StudioApp as ViewerStudioApp, UiActions, UiCommand,
    UiFrame,
};
use scad_viewer::ui::{show_viewer_overlays, status_bar, toolbar};
use studio_common::{AppConfig, DocumentState};
use winit::{
    event::{ElementState, WindowEvent},
    event_loop::EventLoopProxy,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};

use crate::{
    UserEvent, macos_fused_titlebar,
    protocol_client::{DesktopProtocolClient, WatchSubscriptionHandle},
};

pub struct ViewerTab {
    id: TabId,
    path: PathBuf,
    title: String,
    kind: ViewerSourceKind,
    viewer: ViewerStudioApp,
    document: DocumentState,
    camera: OrbitalCamera,
    camera_interaction: CameraInteraction,
    client: DesktopProtocolClient,
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    watch_subscriptions: Vec<WatchSubscriptionHandle>,
    preview_request_serial: u64,
    settings_open: bool,
    clip_plane: ClipPlane,
    clip_edit_mode: EditMode,
    clip_drag_active: bool,
    cursor_position: Option<Vec2>,
    ctrl_pressed: bool,
    current_bounds: Option<Bounds>,
    mesh: Option<MeshData>,
    mesh_revision: u64,
    slicers: Vec<SlicerInstall>,
    slicer_config_snapshot: Vec<(String, PathBuf)>,
    cached_openscad_path: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewerSourceKind {
    Scad,
    Stl,
    ThreeMf,
}

pub struct ViewerUiOutcome {
    pub save_settings: bool,
    pub render_requested: bool,
    pub pending_render: bool,
    pub viewport_rect: egui::Rect,
    pub commands: Vec<UiCommand>,
}

impl ViewerTab {
    pub fn open(
        client: DesktopProtocolClient,
        path: PathBuf,
        aspect_ratio: f32,
        configured_openscad_path: Option<PathBuf>,
        proxy: EventLoopProxy<UserEvent>,
        window_id: WindowId,
    ) -> Result<Self, String> {
        let id = io::tab_id_for_path("viewer", &path);
        let title = io::file_label(&path);
        let kind = io::detect_viewer_kind(&path)?;
        let mut tab = Self {
            id,
            path: path.clone(),
            title,
            kind,
            viewer: ViewerStudioApp::default(),
            document: DocumentState::default(),
            camera: OrbitalCamera::new(aspect_ratio),
            camera_interaction: CameraInteraction::default(),
            client,
            proxy,
            window_id,
            watch_subscriptions: Vec::new(),
            preview_request_serial: 0,
            settings_open: false,
            clip_plane: ClipPlane::default(),
            clip_edit_mode: EditMode::Translate,
            clip_drag_active: false,
            cursor_position: None,
            ctrl_pressed: false,
            current_bounds: None,
            mesh: None,
            mesh_revision: 0,
            slicers: Vec::new(),
            slicer_config_snapshot: Vec::new(),
            cached_openscad_path: configured_openscad_path,
        };
        tab.viewer.viewer_state_mut().wireframe_supported = true;
        tab.load_initial_state()?;
        Ok(tab)
    }

    pub fn legacy_tab_id(&self) -> TabId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn mesh_signature(&self) -> Option<(TabId, u64)> {
        self.mesh.as_ref().map(|_| (self.id, self.mesh_revision))
    }

    pub fn mesh(&self) -> Option<&MeshData> {
        self.mesh.as_ref()
    }

    pub fn camera(&self) -> &OrbitalCamera {
        &self.camera
    }

    pub fn render_settings(&self) -> RenderSettings {
        self.viewer.viewer_state().render_settings()
    }

    pub fn clip_plane(&self) -> Option<&ClipPlane> {
        self.viewer
            .viewer_state()
            .clip_plane_enabled
            .then_some(&self.clip_plane)
    }

    pub fn run_model_tab_frame(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        config: &mut AppConfig,
    ) -> ViewerUiOutcome {
        let previous_state = self.viewer.viewer_state().clone();
        let pending_render = self.document.has_pending_render();
        self.refresh_slicers_if_needed(config);
        self.cached_openscad_path = config.openscad_path.clone();
        let mut actions = UiActions::default();

        let viewport_rect = ui
            .vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let strip_outer_w = ui.available_width();
                let toolbar_inner_w = (strip_outer_w - 16.0).max(1.0);
                let status_inner_w = (strip_outer_w - 20.0).max(1.0);
                let toolbar_h = toolbar::embedded_height(toolbar_inner_w, false);
                let status_h = status_bar::embedded_height(status_inner_w);

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), toolbar_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let _ = viewer_viewport::allocate_filled_strip_ui(
                            ui,
                            egui::vec2(ui.available_width(), toolbar_h),
                            egui::Margin::symmetric(8, 1),
                            theme::palette::BG_PANEL,
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.horizontal(|ui| {
                                    #[cfg(target_os = "macos")]
                                    {
                                        let total = ui.available_width();
                                        let drag_reserve = 40.0f32;
                                        ui.scope(|ui| {
                                            ui.set_max_width((total - drag_reserve).max(1.0));
                                            let mut settings_sink = false;
                                            toolbar::paint_toolbar_row(
                                                ui,
                                                &mut self.viewer,
                                                &mut actions,
                                                false,
                                                &mut settings_sink,
                                            );
                                        });
                                        macos_fused_titlebar::horizontal_drag_tail(ui, 8.0);
                                    }
                                    #[cfg(not(target_os = "macos"))]
                                    {
                                        let mut settings_sink = false;
                                        toolbar::paint_toolbar_row(
                                            ui,
                                            &mut self.viewer,
                                            &mut actions,
                                            false,
                                            &mut settings_sink,
                                        );
                                    }
                                });
                            },
                        );
                    },
                );

                let mid_h = (ui.available_height() - status_h).max(1.0);
                let (viewport_rect, ()) = viewer_viewport::allocate_viewport_ui(
                    ui,
                    egui::vec2(ui.available_width(), mid_h),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        if self.mesh.is_none() {
                            ui.vertical_centered(|ui| {
                                let space = (ui.available_height() * 0.35).max(0.0);
                                ui.add_space(space);
                                ui.spinner();
                                ui.add_space(12.0);
                                ui.label(self.viewer.status_message());
                            });
                        }
                    },
                );

                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), status_h),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        let _ = viewer_viewport::allocate_filled_strip_ui(
                            ui,
                            egui::vec2(ui.available_width(), status_h),
                            egui::Margin::symmetric(10, 3),
                            theme::palette::BG_PANEL,
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                status_bar::paint_status_row(ui, &self.viewer);
                            },
                        );
                    },
                );

                viewport_rect
            })
            .inner;
        viewer_camera::sync_camera_to_viewport(
            &mut self.camera,
            self.viewer.viewer_state().projection_mode,
            viewport_rect,
        );
        let camera_matrices = self.camera.matrices_for_bounds(self.current_bounds);
        let viewport_rect_physical =
            input::physical_viewport_rect(viewport_rect, ctx.pixels_per_point());

        show_viewer_overlays(
            ctx,
            &mut self.viewer,
            camera_matrices,
            &self.camera,
            UiFrame {
                document: &mut self.document,
                config,
                settings_open: &mut self.settings_open,
                slicers: &self.slicers,
            },
            viewport_rect,
            &mut actions,
        );

        if let Some(action) = actions.camera_action.take() {
            input::apply_camera_action(&mut self.camera, action, self.current_bounds);
        }

        ViewerUiOutcome {
            save_settings: actions
                .commands
                .iter()
                .any(|cmd| matches!(cmd, UiCommand::SaveSettings)),
            render_requested: self.document.take_pending_render(),
            pending_render: pending_render || previous_state != *self.viewer.viewer_state(),
            viewport_rect: viewport_rect_physical,
            commands: actions.commands,
        }
    }

    pub fn handle_window_event(&mut self, event: &WindowEvent, viewport_rect: egui::Rect) -> bool {
        input::handle_cross_section_event(self, event, viewport_rect)
            || input::handle_camera_event(self, event, viewport_rect)
    }

    pub fn captures_pointer(&self) -> bool {
        self.clip_drag_active || self.camera_interaction.is_dragging()
    }

    pub fn handle_preview_ready(
        &mut self,
        serial: u64,
        result: Result<crate::protocol_client::PreviewSuccess, String>,
    ) {
        self.apply_preview_ready(serial, result);
    }

    pub fn handle_source_change(&mut self, path: &Path) {
        if self.document.current_source() == Some(path) {
            if let Err(error) = self.reload_source_document(path) {
                self.viewer.set_error(error.clone());
                self.viewer.push_log(LogLevel::Error, error);
                return;
            }
            self.start_render();
            return;
        }
        if self.document.preset_path().as_deref() == Some(path) {
            self.refresh_presets();
            return;
        }
        if self.path == path {
            let _ = self.load_direct_mesh();
        }
    }

    pub fn handle_watch_error(&mut self, message: String) {
        self.viewer.set_error(message.clone());
        self.viewer.push_log(LogLevel::Error, message);
    }

    pub fn save_preset(&mut self, name: String) {
        let Some(path) = self.document.preset_path() else {
            return;
        };
        let Some(parameters) = self.document.parameters() else {
            return;
        };
        let mut next = self.document.presets.clone();
        next.presets
            .insert(name.clone(), parameters.current_values());
        match serde_json::to_string_pretty(&next)
            .map_err(|error| error.to_string())
            .and_then(|json| self.client.write_text_file(&path, json))
        {
            Ok(()) => {
                self.document.preset_name_input.clear();
                self.document.selected_preset = Some(name.clone());
                self.document.set_presets(next);
                self.refresh_presets();
                self.viewer
                    .push_log(LogLevel::Info, format!("已保存预设 {name}"));
            }
            Err(error) => self.viewer.push_log(LogLevel::Error, error),
        }
    }

    pub fn delete_preset(&mut self, name: String) {
        let Some(path) = self.document.preset_path() else {
            return;
        };
        let mut next = self.document.presets.clone();
        next.presets.remove(&name);
        match serde_json::to_string_pretty(&next)
            .map_err(|error| error.to_string())
            .and_then(|json| self.client.write_text_file(&path, json))
        {
            Ok(()) => {
                self.document.selected_preset = None;
                self.document.set_presets(next);
                self.refresh_presets();
                self.viewer
                    .push_log(LogLevel::Info, format!("已删除预设 {name}"));
            }
            Err(error) => self.viewer.push_log(LogLevel::Error, error),
        }
    }

    pub fn export_current_model(&mut self, config: &AppConfig, slicer_name: Option<String>) {
        let Some(source_path) = self.document.current_source().map(PathBuf::from) else {
            return;
        };
        let Some(output_path) = io::export_output_path(self, &source_path, slicer_name.as_deref())
        else {
            return;
        };
        match self.client.export_model(
            config,
            &source_path,
            &self.document.current_defines(),
            output_path.clone(),
            self.document.export_format,
            slicer_name,
        ) {
            Ok(output_path) => {
                self.viewer.push_log(
                    LogLevel::Info,
                    format!("模型已导出到 {}", output_path.display()),
                );
            }
            Err(error) => self.viewer.push_log(LogLevel::Error, error),
        }
    }

    pub fn request_render(&mut self) {
        self.start_render();
    }

    fn refresh_slicers_if_needed(&mut self, config: &AppConfig) {
        let snapshot = io::configured_slicers(config);
        if snapshot == self.slicer_config_snapshot {
            return;
        }
        self.slicer_config_snapshot = snapshot;
        match self.client.list_slicers(&config.slicers) {
            Ok(slicers) => self.slicers = slicers,
            Err(error) => self.viewer.push_log(LogLevel::Warning, error),
        }
    }
}

impl WorkTab for ViewerTab {
    fn id(&self) -> TabId {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn is_closable(&self) -> bool {
        true
    }

    fn show(&mut self, ui: &mut egui::Ui, _ctx: &mut TabContext<'_>) {
        ui.label(
            egui::RichText::new("模型预览由主窗口在每帧传入 AppConfig 时绘制；不应通过 TabManager::show_active_content 单独调用。")
                .color(theme::palette::TEXT_SECONDARY)
                .size(12.0),
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
