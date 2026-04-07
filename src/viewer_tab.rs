use std::{
    any::Any,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use glam::{Vec2, Vec4Swizzles};
use scad_data::{
    AppConfig, DocumentState, FileWatcher, LogLevel, OpenScadError, OpenScadMessage,
    OpenScadRunner, RenderedArtifact, SlicerConfig, WatchMessage, build_export_filename,
    delete_preset, detect_slicer_paths, export_model, load_presets, save_preset, send_to_slicer,
};
use scad_scene::{
    Bounds, CameraInteraction, ClipPlane, EditMode, MeshData, OrbitalCamera, RenderSettings,
    three_mf,
};
use scad_ui::tab_system::{TabContext, TabId, WorkTab};
use scad_ui::theme;
use scad_viewer::app::{CameraAction, StudioApp as ViewerStudioApp, UiActions, UiCommand, UiFrame};
use scad_viewer::ui::{show_viewer_overlays, status_bar, toolbar};
use winit::{
    event::{ElementState, WindowEvent},
    event_loop::EventLoopProxy,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowId,
};

use crate::UserEvent;
use crate::macos_fused_titlebar;
use crate::viewer_camera;
use crate::viewer_viewport;

pub struct ViewerTab {
    id: TabId,
    path: PathBuf,
    title: String,
    kind: ViewerSourceKind,
    viewer: ViewerStudioApp,
    document: DocumentState,
    camera: OrbitalCamera,
    camera_interaction: CameraInteraction,
    openscad: Option<OpenScadRunner>,
    watcher: FileWatcher,
    settings_open: bool,
    clip_plane: ClipPlane,
    clip_edit_mode: EditMode,
    clip_drag_active: bool,
    cursor_position: Option<Vec2>,
    ctrl_pressed: bool,
    current_bounds: Option<Bounds>,
    mesh: Option<MeshData>,
    mesh_revision: u64,
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
        path: PathBuf,
        aspect_ratio: f32,
        proxy: EventLoopProxy<UserEvent>,
        window_id: WindowId,
    ) -> Result<Self, String> {
        let id = tab_id_for_path("viewer", &path);
        let title = file_label(&path);
        let kind = detect_viewer_kind(&path)?;
        let watcher = FileWatcher::new(build_source_notifier(proxy.clone(), window_id, id));
        let openscad = matches!(kind, ViewerSourceKind::Scad)
            .then(|| OpenScadRunner::new(build_openscad_notifier(proxy, window_id, id)));
        let mut tab = Self {
            id,
            path: path.clone(),
            title,
            kind,
            viewer: ViewerStudioApp::default(),
            document: DocumentState::default(),
            camera: OrbitalCamera::new(aspect_ratio),
            camera_interaction: CameraInteraction::default(),
            openscad,
            watcher,
            settings_open: false,
            clip_plane: ClipPlane::default(),
            clip_edit_mode: EditMode::Translate,
            clip_drag_active: false,
            cursor_position: None,
            ctrl_pressed: false,
            current_bounds: None,
            mesh: None,
            mesh_revision: 0,
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

    /// 在标签页 `Ui` 内绘制工具栏、状态栏与中间透明视口，并在 `ctx` 上绘制浮层（参数面板、gizmo 等）。
    pub fn run_model_tab_frame(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        config: &mut AppConfig,
    ) -> ViewerUiOutcome {
        let previous_state = self.viewer.viewer_state().clone();
        let pending_render = self.document.has_pending_render();
        let slicers = detect_slicer_paths(config);
        let mut actions = UiActions::default();

        let viewport_rect = ui
            .vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                // 与条带分配使用同一 `Ui` 的可用宽度；水平边距 8+8=16；垂直边距与 `toolbar_strip_outer_height` 中 `STRIP_VERT_MARGIN` 一致。
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
        let viewport_rect_physical = physical_viewport_rect(viewport_rect, ctx.pixels_per_point());

        show_viewer_overlays(
            ctx,
            &mut self.viewer,
            camera_matrices,
            &self.camera,
            UiFrame {
                document: &mut self.document,
                config,
                settings_open: &mut self.settings_open,
                slicers: &slicers,
            },
            viewport_rect,
            &mut actions,
        );

        if let Some(action) = actions.camera_action.take() {
            apply_camera_action(&mut self.camera, action, self.current_bounds);
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
        handle_cross_section_event(self, event, viewport_rect)
            || handle_camera_event(self, event, viewport_rect)
    }

    pub fn captures_pointer(&self) -> bool {
        self.clip_drag_active || self.camera_interaction.is_dragging()
    }

    pub fn handle_openscad_message(&mut self, message: OpenScadMessage) {
        match message {
            OpenScadMessage::Started(path) => {
                self.viewer.set_current_file(path);
                self.viewer.set_rendering("OpenSCAD 正在生成 3MF 预览");
            }
            OpenScadMessage::Log(entry) => self.viewer.push_log(entry.level, entry.message),
            OpenScadMessage::Finished(result) => self.handle_render_result(result),
        }
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
        match save_preset(&path, &name, parameters) {
            Ok(()) => {
                self.document.preset_name_input.clear();
                self.document.selected_preset = Some(name.clone());
                self.refresh_presets();
                self.viewer
                    .push_log(LogLevel::Info, format!("已保存预设 {name}"));
            }
            Err(error) => self.viewer.push_log(LogLevel::Error, error.to_string()),
        }
    }

    pub fn delete_preset(&mut self, name: String) {
        let Some(path) = self.document.preset_path() else {
            return;
        };
        match delete_preset(&path, &name) {
            Ok(()) => {
                self.document.selected_preset = None;
                self.refresh_presets();
                self.viewer
                    .push_log(LogLevel::Info, format!("已删除预设 {name}"));
            }
            Err(error) => self.viewer.push_log(LogLevel::Error, error.to_string()),
        }
    }

    pub fn export_current_model(&mut self, config: &AppConfig, slicer_name: Option<String>) {
        let Some(source_path) = self.document.current_source().map(PathBuf::from) else {
            return;
        };
        let Some(output_path) = export_output_path(self, &source_path, slicer_name.as_deref())
        else {
            return;
        };
        match export_model(
            config,
            &source_path,
            &self.document.current_defines(),
            &output_path,
            self.document.export_format,
        ) {
            Ok(()) => {
                if let Some(name) = slicer_name
                    && let Some(slicer) = detect_slicer_paths(config)
                        .into_iter()
                        .find(|slicer| slicer.name == name)
                    && let Err(error) = send_to_slicer(
                        &SlicerConfig {
                            name: slicer.name,
                            path: slicer.path,
                        },
                        &output_path,
                    )
                {
                    self.viewer.push_log(LogLevel::Error, error);
                    return;
                }
                self.viewer.push_log(
                    LogLevel::Info,
                    format!("模型已导出到 {}", output_path.display()),
                );
            }
            Err(error) => self.viewer.push_log(LogLevel::Error, error.to_string()),
        }
    }

    pub fn request_render(&mut self) {
        self.start_render();
    }

    fn load_initial_state(&mut self) -> Result<(), String> {
        self.viewer.set_current_file(self.path.clone());
        match self.kind {
            ViewerSourceKind::Scad => self.load_scad_document(),
            ViewerSourceKind::Stl | ViewerSourceKind::ThreeMf => self.load_direct_mesh(),
        }
    }

    fn load_scad_document(&mut self) -> Result<(), String> {
        let source_text = std::fs::read_to_string(&self.path)
            .map_err(|error| format!("读取源文件失败: {error}"))?;
        self.document.load_source(self.path.clone(), &source_text);
        self.refresh_presets();
        self.flush_document_warnings();
        self.watcher.watch_files(self.document.watch_paths());
        self.start_render();
        Ok(())
    }

    fn load_direct_mesh(&mut self) -> Result<(), String> {
        let mesh = match self.kind {
            ViewerSourceKind::Stl => {
                scad_scene::mesh::load_stl(&self.path).map_err(|error| error.to_string())?
            }
            ViewerSourceKind::ThreeMf => {
                three_mf::load_3mf(&self.path).map_err(|error| error.to_string())?
            }
            ViewerSourceKind::Scad => return Ok(()),
        };
        self.watcher.watch_files(vec![self.path.clone()]);
        self.set_mesh(mesh);
        self.viewer.set_ready("预览已更新");
        self.viewer
            .push_log(LogLevel::Info, format!("已载入 {}", self.path.display()));
        Ok(())
    }

    fn reload_source_document(&mut self, path: &Path) -> Result<(), String> {
        let source_text =
            std::fs::read_to_string(path).map_err(|error| format!("读取源文件失败: {error}"))?;
        self.document.reload_source(&source_text);
        self.flush_document_warnings();
        Ok(())
    }

    fn refresh_presets(&mut self) {
        let Some(path) = self.document.preset_path() else {
            return;
        };
        match load_presets(&path) {
            Ok(presets) => self.document.set_presets(presets),
            Err(error) => self.viewer.push_log(LogLevel::Warning, error.to_string()),
        }
    }

    fn flush_document_warnings(&mut self) {
        for warning in self.document.take_warnings() {
            self.viewer.push_log(LogLevel::Warning, warning);
        }
    }

    fn start_render(&mut self) {
        let Some(openscad) = self.openscad.as_ref() else {
            return;
        };
        self.viewer.set_rendering("正在调用 OpenSCAD 生成 3MF");
        self.viewer
            .push_log(LogLevel::Info, format!("开始渲染 {}", self.path.display()));
        openscad.render_with_defines(self.path.clone(), self.document.current_defines());
    }

    fn handle_render_result(&mut self, result: Result<RenderedArtifact, OpenScadError>) {
        match result {
            Ok(artifact) => {
                self.viewer.set_current_file(artifact.source_path.clone());
                self.viewer.set_ready("预览已更新");
                self.viewer
                    .push_log(LogLevel::Info, "OpenSCAD 3MF 预览完成");
                self.set_mesh(artifact.mesh);
            }
            Err(error) => {
                self.mesh = None;
                self.mesh_revision = self.mesh_revision.wrapping_add(1);
                self.viewer.set_error(error.to_string());
                self.viewer.push_log(LogLevel::Error, error.to_string());
            }
        }
    }

    fn set_mesh(&mut self, mesh: MeshData) {
        self.clip_plane.visible_extent = mesh.bounds.radius().max(64.0);
        self.current_bounds = Some(mesh.bounds);
        self.camera.fit_bounds(mesh.bounds);
        self.mesh = Some(mesh);
        self.mesh_revision = self.mesh_revision.wrapping_add(1);
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

fn build_openscad_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(OpenScadMessage) + Send + 'static {
    move |message| {
        let _ = proxy.send_event(UserEvent::OpenScad(window_id, tab_id, message));
    }
}

fn build_source_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(WatchMessage) + Send + 'static {
    move |message| match message {
        WatchMessage::Changed(path) => {
            let _ = proxy.send_event(UserEvent::SourceChanged(window_id, tab_id, path));
        }
        WatchMessage::Error(message) => {
            let _ = proxy.send_event(UserEvent::WatchError(window_id, tab_id, message));
        }
    }
}

fn tab_id_for_path(kind: &str, path: &Path) -> TabId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish()
}

fn detect_viewer_kind(path: &Path) -> Result<ViewerSourceKind, String> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("scad") => Ok(ViewerSourceKind::Scad),
        Some("stl") => Ok(ViewerSourceKind::Stl),
        Some("3mf") => Ok(ViewerSourceKind::ThreeMf),
        _ => Err(format!("不支持的模型类型: {}", path.display())),
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("模型")
        .to_owned()
}

fn export_output_path(
    tab: &ViewerTab,
    source_path: &Path,
    slicer_name: Option<&str>,
) -> Option<PathBuf> {
    if slicer_name.is_some() {
        return Some(std::env::temp_dir().join(build_export_filename(
            source_path,
            tab.document.export_format,
        )));
    }
    let file_name = build_export_filename(source_path, tab.document.export_format);
    let extension = tab.document.export_format.extension();
    rfd::FileDialog::new()
        .set_file_name(file_name)
        .add_filter(extension.to_uppercase(), &[extension])
        .save_file()
}

fn handle_cross_section_event(
    tab: &mut ViewerTab,
    event: &WindowEvent,
    viewport_rect: egui::Rect,
) -> bool {
    match event {
        WindowEvent::ModifiersChanged(modifiers) => {
            tab.ctrl_pressed = modifiers.state().control_key();
            false
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed || !tab.viewer.viewer_state().clip_plane_enabled
            {
                return false;
            }
            match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyW) => {
                    tab.clip_edit_mode = EditMode::Translate;
                    true
                }
                PhysicalKey::Code(KeyCode::KeyE) => {
                    tab.clip_edit_mode = EditMode::Rotate;
                    true
                }
                _ => false,
            }
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: winit::event::MouseButton::Left,
            ..
        } => begin_clip_drag(
            tab,
            Vec2::new(viewport_rect.width(), viewport_rect.height()),
        ),
        WindowEvent::MouseInput {
            state: ElementState::Released,
            button: winit::event::MouseButton::Left,
            ..
        } => {
            tab.clip_drag_active = false;
            false
        }
        WindowEvent::CursorMoved { position, .. } => {
            let cursor = viewport_local_cursor(*position, viewport_rect);
            if update_clip_drag(tab, cursor) {
                return true;
            }
            tab.cursor_position = Some(cursor);
            false
        }
        _ => false,
    }
}

fn handle_camera_event(
    tab: &mut ViewerTab,
    event: &WindowEvent,
    viewport_rect: egui::Rect,
) -> bool {
    match event {
        WindowEvent::MouseInput { state, button, .. } => tab
            .camera_interaction
            .handle_mouse_input_event(*state, *button),
        WindowEvent::CursorMoved { position, .. } => tab.camera_interaction.handle_cursor_position(
            &mut tab.camera,
            viewport_local_cursor(*position, viewport_rect),
        ),
        WindowEvent::MouseWheel { delta, .. } => tab
            .camera_interaction
            .handle_wheel_delta(&mut tab.camera, delta),
        _ => false,
    }
}

fn begin_clip_drag(tab: &mut ViewerTab, viewport_size: Vec2) -> bool {
    if !tab.viewer.viewer_state().clip_plane_enabled {
        tab.clip_plane.selected = false;
        return false;
    }
    let Some(cursor) = tab.cursor_position else {
        return false;
    };
    let inverse = tab
        .camera
        .matrices_for_bounds(tab.current_bounds)
        .view_proj
        .inverse();
    let Some(ray) = scad_scene::cross_section::screen_ray(cursor, viewport_size, inverse) else {
        return false;
    };
    let Some(distance) = tab.clip_plane.ray_intersection(ray.origin, ray.direction) else {
        tab.clip_plane.selected = false;
        return false;
    };
    let hit_point = ray.origin + ray.direction * distance;
    if !tab.clip_plane.contains_point(hit_point) {
        tab.clip_plane.selected = false;
        return false;
    }
    tab.clip_plane.selected = true;
    tab.clip_drag_active = true;
    true
}

fn update_clip_drag(tab: &mut ViewerTab, cursor: Vec2) -> bool {
    if !tab.viewer.viewer_state().clip_plane_enabled || !tab.clip_drag_active {
        return false;
    }
    let previous = tab.cursor_position.unwrap_or(cursor);
    let delta = cursor - previous;
    let distance_scale = tab
        .camera
        .matrices_for_bounds(tab.current_bounds)
        .eye
        .distance(tab.clip_plane.center())
        * 0.0025;
    match tab.clip_edit_mode {
        EditMode::Translate => {
            let amount = (delta.x - delta.y) * distance_scale;
            tab.clip_plane
                .translate_along_normal(amount, tab.ctrl_pressed);
        }
        EditMode::Rotate => {
            let inverse_view = tab
                .camera
                .matrices_for_bounds(tab.current_bounds)
                .view
                .inverse();
            let right = inverse_view.x_axis.xyz().normalize_or_zero();
            let up = inverse_view.y_axis.xyz().normalize_or_zero();
            let axis = if delta.x.abs() >= delta.y.abs() {
                up
            } else {
                right
            };
            tab.clip_plane
                .rotate((delta.x - delta.y) * 0.01, axis, tab.ctrl_pressed);
        }
    }
    tab.cursor_position = Some(cursor);
    true
}

fn apply_camera_action(camera: &mut OrbitalCamera, action: CameraAction, bounds: Option<Bounds>) {
    match action {
        CameraAction::SetTargetX(v) => camera.set_target_x(v),
        CameraAction::SetTargetY(v) => camera.set_target_y(v),
        CameraAction::SetTargetZ(v) => camera.set_target_z(v),
        CameraAction::SetDistance(v) => camera.set_distance(v),
        CameraAction::SetAzimuth(v) => camera.set_azimuth_degrees(v),
        CameraAction::SetElevation(v) => camera.set_elevation_degrees(v),
        CameraAction::ResetView => camera.reset_view(bounds),
        CameraAction::ViewTop => camera.view_top(),
        CameraAction::ViewBottom => camera.view_bottom(),
        CameraAction::ViewFront => camera.view_front(),
        CameraAction::ViewBack => camera.view_back(),
        CameraAction::ViewLeft => camera.view_left(),
        CameraAction::ViewRight => camera.view_right(),
    }
}

fn viewport_local_cursor(
    position: winit::dpi::PhysicalPosition<f64>,
    viewport_rect: egui::Rect,
) -> Vec2 {
    Vec2::new(
        position.x as f32 - viewport_rect.min.x,
        position.y as f32 - viewport_rect.min.y,
    )
}

fn physical_viewport_rect(viewport_rect: egui::Rect, pixels_per_point: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            viewport_rect.min.x * pixels_per_point,
            viewport_rect.min.y * pixels_per_point,
        ),
        egui::pos2(
            viewport_rect.max.x * pixels_per_point,
            viewport_rect.max.y * pixels_per_point,
        ),
    )
}
