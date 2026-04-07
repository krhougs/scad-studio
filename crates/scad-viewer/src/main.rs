mod app;
mod platform_menu;
mod ui;
mod wrap_line_pack;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use app::{LogEntry, LogLevel, StudioApp};
use egui::ViewportId;
use glam::{Vec2, Vec4Swizzles};
use platform_menu::{APP_NAME, MenuCommand, PlatformMenu};
use scad_data::{
    AppConfig, DocumentState, FileWatcher, OpenScadError, OpenScadMessage, OpenScadRunner,
    RenderedArtifact, SlicerConfig, WatchMessage, build_export_filename, delete_preset,
    detect_slicer_paths, export_model, load_config, load_presets, save_config, save_preset,
    send_to_slicer,
};
use scad_scene::{
    Bounds, CameraInteraction, ClipPlane, EditMode, EguiPaintData, OrbitalCamera, Renderer,
};
use scad_ui::font_setup;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Window, WindowId},
};

#[derive(Debug, Clone)]
enum UserEvent {
    OpenScad(WindowId, OpenScadMessage),
    SourceChanged(WindowId, PathBuf),
    WatchError(WindowId, String),
    Menu(String),
}

struct RuntimeState {
    window: Arc<Window>,
    renderer: Renderer,
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    studio: StudioApp,
    camera: OrbitalCamera,
    camera_interaction: CameraInteraction,
    openscad: OpenScadRunner,
    watcher: FileWatcher,
    document: DocumentState,
    config: AppConfig,
    settings_open: bool,
    clip_plane: ClipPlane,
    clip_edit_mode: EditMode,
    clip_drag_active: bool,
    cursor_position: Option<Vec2>,
    ctrl_pressed: bool,
    modifiers: ModifiersState,
    redraw_queued: bool,
    current_bounds: Option<Bounds>,
}

struct DesktopApp {
    proxy: EventLoopProxy<UserEvent>,
    platform_menu: Option<PlatformMenu>,
    default_config: AppConfig,
    startup_warning: Option<String>,
    windows: HashMap<WindowId, RuntimeState>,
    focused_window: Option<WindowId>,
    last_active_window: Option<WindowId>,
}

fn main() {
    env_logger::init();
    let (config, startup_warning) = load_runtime_config();
    apply_openscad_path_override(&config);
    let platform_menu = PlatformMenu::new();
    let mut event_loop_builder = EventLoop::<UserEvent>::with_user_event();
    if let Some(menu) = platform_menu.as_ref() {
        menu.configure_event_loop(&mut event_loop_builder);
    }
    let event_loop = event_loop_builder.build().expect("创建事件循环失败");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    if let Some(menu) = platform_menu.as_ref() {
        menu.attach_event_handler(proxy.clone());
    }
    let mut app = DesktopApp {
        proxy,
        platform_menu,
        default_config: config,
        startup_warning,
        windows: HashMap::new(),
        focused_window: None,
        last_active_window: None,
    };
    event_loop.run_app(&mut app).expect("运行应用失败");
}

impl ApplicationHandler<UserEvent> for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty()
            && let Err(error) = self.create_window(event_loop)
        {
            log::error!("{error}");
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::OpenScad(window_id, message) => {
                self.handle_openscad_message(window_id, message)
            }
            UserEvent::SourceChanged(window_id, path) => self.handle_source_change(window_id, path),
            UserEvent::WatchError(window_id, message) => {
                self.handle_watch_error(window_id, message)
            }
            UserEvent::Menu(id) => self.handle_menu_event(event_loop, &id),
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::Focused(focused) = &event {
            self.update_window_focus(window_id, *focused);
        }
        let mut dropped_file = None;
        let mut close_window = false;
        let mut redraw_result = None;
        let mut shortcut_action = None;
        {
            let Some(state) = self.windows.get_mut(&window_id) else {
                return;
            };
            let egui_response = state.egui_state.on_window_event(&state.window, &event);
            if egui_response.repaint {
                schedule_redraw(state);
            }
            match event {
                WindowEvent::CloseRequested => close_window = true,
                WindowEvent::Resized(size) => resize_runtime(state, size),
                WindowEvent::DroppedFile(path) => dropped_file = Some(path),
                WindowEvent::ScaleFactorChanged { .. } => {
                    let size = state.window.inner_size();
                    resize_runtime(state, size);
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    state.modifiers = modifiers.state();
                    state.ctrl_pressed = modifiers.state().control_key();
                }
                WindowEvent::RedrawRequested => {
                    redraw_result = redraw_window(state, self.platform_menu.is_none())
                }
                WindowEvent::KeyboardInput { ref event, .. } if !egui_response.consumed => {
                    shortcut_action = shortcut_action_for(event, state.modifiers);
                }
                other if !egui_response.consumed => {
                    let handled = handle_cross_section_event(state, &other)
                        || state
                            .camera_interaction
                            .handle_event(&mut state.camera, &other);
                    if handled {
                        schedule_redraw(state);
                    }
                }
                _ => {}
            }
        }
        if let Some(action) = shortcut_action {
            self.apply_shortcut(window_id, event_loop, action);
        }
        if let Some(path) = dropped_file.filter(|path| is_scad_file(path)) {
            self.open_source_file(window_id, path);
        }
        if let Some((ui_actions, render_due, pending_render)) = redraw_result {
            self.handle_post_redraw(window_id, ui_actions, render_due, pending_render);
        }
        if close_window {
            self.close_window(event_loop, window_id);
        }
    }
}

impl DesktopApp {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<WindowId, String> {
        let runtime = create_runtime(
            event_loop,
            &self.platform_menu,
            self.proxy.clone(),
            self.default_config.clone(),
            self.startup_warning.take(),
        )?;
        let window_id = runtime.window.id();
        self.last_active_window = Some(window_id);
        self.windows.insert(window_id, runtime);
        if let Some(state) = self.windows.get_mut(&window_id) {
            schedule_redraw(state);
        }
        Ok(window_id)
    }

    fn apply_shortcut(
        &mut self,
        window_id: WindowId,
        event_loop: &ActiveEventLoop,
        action: ShortcutAction,
    ) {
        match action {
            ShortcutAction::NewWindow => {
                if let Err(error) = self.create_window(event_loop) {
                    log::error!("创建 Viewer 窗口失败: {error}");
                }
            }
            ShortcutAction::OpenFile => {
                if let Some(path) = select_scad_file() {
                    self.open_source_file(window_id, path);
                }
            }
            ShortcutAction::CloseWindow => self.close_window(event_loop, window_id),
            ShortcutAction::OpenSettings => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state.settings_open = true;
                    schedule_redraw(state);
                }
            }
            ShortcutAction::Quit => event_loop.exit(),
        }
    }

    fn update_window_focus(&mut self, window_id: WindowId, focused: bool) {
        if focused {
            self.focused_window = Some(window_id);
            self.last_active_window = Some(window_id);
            return;
        }
        if self.focused_window == Some(window_id) {
            self.focused_window = None;
        }
    }

    fn active_window_id(&self) -> Option<WindowId> {
        self.focused_window
            .filter(|window_id| self.windows.contains_key(window_id))
            .or_else(|| {
                self.last_active_window
                    .filter(|window_id| self.windows.contains_key(window_id))
            })
            .or_else(|| self.windows.keys().next().copied())
    }

    fn close_window(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) {
        self.windows.remove(&window_id);
        if self.focused_window == Some(window_id) {
            self.focused_window = None;
        }
        if self.last_active_window == Some(window_id) {
            self.last_active_window = self.windows.keys().next().copied();
        }
        if self.windows.is_empty() {
            event_loop.exit();
        }
    }

    fn handle_menu_event(&mut self, event_loop: &ActiveEventLoop, id: &str) {
        let Some(menu) = self.platform_menu.as_ref() else {
            return;
        };
        let Some(command) = menu.command_for_event(id) else {
            return;
        };
        match command {
            MenuCommand::NewWindow => {
                if let Err(error) = self.create_window(event_loop) {
                    log::error!("创建 Viewer 窗口失败: {error}");
                }
            }
            MenuCommand::OpenFile => {
                if let Some(window_id) = self.active_window_id()
                    && let Some(path) = select_scad_file()
                {
                    self.open_source_file(window_id, path);
                }
            }
            MenuCommand::CloseWindow => {
                if let Some(window_id) = self.active_window_id() {
                    self.close_window(event_loop, window_id);
                }
            }
            MenuCommand::OpenSettings => {
                if let Some(window_id) = self.active_window_id()
                    && let Some(state) = self.windows.get_mut(&window_id)
                {
                    state.settings_open = true;
                    schedule_redraw(state);
                }
            }
            MenuCommand::ShowAbout => {
                let parent = self
                    .active_window_id()
                    .and_then(|window_id| self.windows.get(&window_id))
                    .map(|state| state.window.as_ref());
                show_about_dialog(parent);
            }
            MenuCommand::QuitApp => event_loop.exit(),
        }
    }

    fn open_source_file(&mut self, window_id: WindowId, source_path: PathBuf) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        match load_document(state, source_path.clone()) {
            Ok(()) => start_render(state, source_path),
            Err(message) => {
                state.studio.set_error(message.clone());
                state.studio.push_log(LogLevel::Error, message);
                schedule_redraw(state);
            }
        }
    }

    fn handle_post_redraw(
        &mut self,
        window_id: WindowId,
        ui_actions: app::UiActions,
        render_due: bool,
        pending_render: bool,
    ) {
        if ui_actions.open_file
            && let Some(path) = select_scad_file()
        {
            self.open_source_file(window_id, path);
        }
        self.handle_ui_commands(window_id, ui_actions.commands);
        if render_due {
            self.render_current_document(window_id);
        }
        if (ui_actions.viewer_state_changed || pending_render)
            && let Some(state) = self.windows.get_mut(&window_id)
        {
            schedule_redraw(state);
        }
    }

    fn handle_ui_commands(&mut self, window_id: WindowId, commands: Vec<app::UiCommand>) {
        for command in commands {
            match command {
                app::UiCommand::SavePreset(name) => self.save_preset(window_id, name),
                app::UiCommand::DeletePreset(name) => self.delete_preset(window_id, name),
                app::UiCommand::ExportModel => self.export_current_model(window_id, None),
                app::UiCommand::SendToSlicer(name) => {
                    self.export_current_model(window_id, Some(name));
                }
                app::UiCommand::SaveSettings => self.save_settings(window_id),
            }
        }
    }

    fn render_current_document(&mut self, window_id: WindowId) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        let Some(source_path) = state.document.current_source().map(PathBuf::from) else {
            return;
        };
        start_render(state, source_path);
    }

    fn handle_openscad_message(&mut self, window_id: WindowId, message: OpenScadMessage) {
        match message {
            OpenScadMessage::Started(path) => self.handle_render_started(window_id, path),
            OpenScadMessage::Log(entry) => self.handle_openscad_log(window_id, entry),
            OpenScadMessage::Finished(result) => self.handle_render_finished(window_id, result),
        }
    }

    fn handle_render_started(&mut self, window_id: WindowId, path: PathBuf) {
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.studio.set_current_file(path);
            state.studio.set_rendering("OpenSCAD 正在生成 3MF 预览");
            schedule_redraw(state);
        }
    }

    fn handle_openscad_log(&mut self, window_id: WindowId, entry: LogEntry) {
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.studio.push_log(entry.level, entry.message);
            schedule_redraw(state);
        }
    }

    fn handle_render_finished(
        &mut self,
        window_id: WindowId,
        result: Result<RenderedArtifact, OpenScadError>,
    ) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        match result {
            Ok(artifact) => {
                state.clip_plane.visible_extent = artifact.mesh.bounds.radius().max(64.0);
                state.studio.set_current_file(artifact.source_path.clone());
                state.current_bounds = Some(artifact.mesh.bounds);
                state.camera.fit_bounds(artifact.mesh.bounds);
                state.renderer.set_mesh(artifact.mesh);
                state.studio.set_ready("预览已更新");
                state
                    .studio
                    .push_log(LogLevel::Info, "OpenSCAD 3MF 预览完成");
            }
            Err(error) => {
                state.renderer.clear_mesh();
                state.studio.set_error(error.to_string());
                state.studio.push_log(LogLevel::Error, error.to_string());
            }
        }
        schedule_redraw(state);
    }

    fn handle_source_change(&mut self, window_id: WindowId, path: PathBuf) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if state.document.current_source() == Some(path.as_path()) {
            if let Err(message) = reload_source_document(state, &path) {
                state.studio.set_error(message.clone());
                state.studio.push_log(LogLevel::Error, message);
                schedule_redraw(state);
                return;
            }
            start_render(state, path);
            schedule_redraw(state);
            return;
        }
        if state.document.preset_path().as_deref() == Some(path.as_path()) {
            refresh_presets(state);
            schedule_redraw(state);
        }
    }

    fn handle_watch_error(&mut self, window_id: WindowId, message: String) {
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.studio.set_error(message.clone());
            state.studio.push_log(LogLevel::Error, message);
            schedule_redraw(state);
        }
    }

    fn save_preset(&mut self, window_id: WindowId, name: String) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        let Some(path) = state.document.preset_path() else {
            return;
        };
        let Some(parameters) = state.document.parameters() else {
            return;
        };
        match save_preset(&path, &name, parameters) {
            Ok(()) => {
                state.document.preset_name_input.clear();
                state.document.selected_preset = Some(name.clone());
                refresh_presets(state);
                state
                    .studio
                    .push_log(LogLevel::Info, format!("已保存预设 {name}"));
            }
            Err(error) => state.studio.push_log(LogLevel::Error, error.to_string()),
        }
    }

    fn delete_preset(&mut self, window_id: WindowId, name: String) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        let Some(path) = state.document.preset_path() else {
            return;
        };
        match delete_preset(&path, &name) {
            Ok(()) => {
                state.document.selected_preset = None;
                refresh_presets(state);
                state
                    .studio
                    .push_log(LogLevel::Info, format!("已删除预设 {name}"));
            }
            Err(error) => state.studio.push_log(LogLevel::Error, error.to_string()),
        }
    }

    fn export_current_model(&mut self, window_id: WindowId, slicer_name: Option<String>) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        let Some(source_path) = state.document.current_source().map(PathBuf::from) else {
            return;
        };
        let output_path = match export_output_path(state, &source_path, slicer_name.as_deref()) {
            Some(path) => path,
            None => return,
        };
        let result = export_model(
            &state.config,
            &source_path,
            &state.document.current_defines(),
            &output_path,
            state.document.export_format,
        );
        match result {
            Ok(()) => {
                if let Some(name) = slicer_name
                    && let Some(slicer) = detect_slicer_paths(&state.config)
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
                    state.studio.push_log(LogLevel::Error, error);
                    return;
                }
                state.studio.push_log(
                    LogLevel::Info,
                    format!("模型已导出到 {}", output_path.display()),
                );
            }
            Err(error) => state.studio.push_log(LogLevel::Error, error.to_string()),
        }
    }

    fn save_settings(&mut self, window_id: WindowId) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        match save_config(&state.config) {
            Ok(()) => {
                self.default_config = state.config.clone();
                apply_openscad_path_override(&self.default_config);
                state.studio.push_log(LogLevel::Info, "配置已保存");
            }
            Err(error) => state.studio.push_log(LogLevel::Error, error.to_string()),
        }
    }
}

fn create_runtime(
    event_loop: &ActiveEventLoop,
    platform_menu: &Option<PlatformMenu>,
    proxy: EventLoopProxy<UserEvent>,
    config: AppConfig,
    startup_warning: Option<String>,
) -> Result<RuntimeState, String> {
    let window = Arc::new(
        event_loop
            .create_window(
                Window::default_attributes()
                    .with_title(APP_NAME)
                    .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0)),
            )
            .map_err(|error| format!("创建窗口失败: {error}"))?,
    );
    let window_id = window.id();
    let renderer = pollster::block_on(Renderer::new(window.clone()))
        .map_err(|error| format!("初始化渲染器失败: {error}"))?;
    let egui_context = egui::Context::default();
    font_setup::configure_egui_fonts(&egui_context);
    let egui_state = egui_winit::State::new(
        egui_context.clone(),
        ViewportId::ROOT,
        window.as_ref(),
        Some(window.scale_factor() as f32),
        window.theme(),
        Some(renderer.max_texture_side()),
    );
    let camera = OrbitalCamera::new(renderer.aspect_ratio());
    let openscad = OpenScadRunner::new(build_openscad_notifier(proxy.clone(), window_id));
    let watcher = FileWatcher::new(build_source_notifier(proxy, window_id));
    if let Some(menu) = platform_menu.as_ref() {
        menu.install(window.as_ref())?;
    }
    let mut studio = StudioApp::default();
    if let Some(message) = startup_warning {
        studio.push_log(LogLevel::Warning, message);
    }
    let mut runtime = RuntimeState {
        window,
        renderer,
        egui_context,
        egui_state,
        studio,
        camera,
        camera_interaction: CameraInteraction::default(),
        openscad,
        watcher,
        document: DocumentState::default(),
        config,
        settings_open: false,
        clip_plane: ClipPlane::default(),
        clip_edit_mode: EditMode::Translate,
        clip_drag_active: false,
        cursor_position: None,
        ctrl_pressed: false,
        modifiers: Modifiers::default().state(),
        redraw_queued: false,
        current_bounds: None,
    };
    runtime.studio.viewer_state_mut().wireframe_supported = runtime.renderer.wireframe_supported();
    Ok(runtime)
}

fn build_openscad_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
) -> impl Fn(OpenScadMessage) + Send + 'static {
    move |message| {
        let _ = proxy.send_event(UserEvent::OpenScad(window_id, message));
    }
}

fn build_source_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
) -> impl Fn(WatchMessage) + Send + 'static {
    move |message| match message {
        WatchMessage::Changed(path) => {
            let _ = proxy.send_event(UserEvent::SourceChanged(window_id, path));
        }
        WatchMessage::Error(message) => {
            let _ = proxy.send_event(UserEvent::WatchError(window_id, message));
        }
    }
}

fn redraw_window(
    state: &mut RuntimeState,
    show_embedded_menu: bool,
) -> Option<(app::UiActions, bool, bool)> {
    state.redraw_queued = false;
    let raw_input = state.egui_state.take_egui_input(&state.window);
    let mut ui_actions = app::UiActions::default();
    let camera_matrices = state.camera.matrices_for_bounds(state.current_bounds);
    let slicers = detect_slicer_paths(&state.config);
    let full_output = state.egui_context.run(raw_input, |ctx| {
        ui_actions = state.studio.ui(
            ctx,
            show_embedded_menu,
            camera_matrices,
            &state.camera,
            app::UiFrame {
                document: &mut state.document,
                config: &mut state.config,
                settings_open: &mut state.settings_open,
                slicers: &slicers,
            },
        );
    });
    state
        .camera
        .set_projection_mode(state.studio.viewer_state().projection_mode);
    if let Some(action) = ui_actions.camera_action.take() {
        if matches!(action, app::CameraAction::ResetView) {
            state.camera.reset_view(state.current_bounds);
        } else {
            apply_camera_action(&mut state.camera, action);
        }
    }
    state
        .egui_state
        .handle_platform_output(&state.window, full_output.platform_output);
    let paint_data = build_paint_data(state, full_output.shapes, full_output.textures_delta);
    let render_settings = state.studio.viewer_state().render_settings();
    if let Err(error) = state.renderer.render(
        &state.camera,
        &render_settings,
        state
            .studio
            .viewer_state()
            .clip_plane_enabled
            .then_some(&state.clip_plane),
        None,
        paint_data,
    ) {
        state.studio.set_error(format!("渲染失败: {error}"));
        state
            .studio
            .push_log(LogLevel::Error, format!("渲染失败: {error}"));
    }
    let render_due = state.document.take_pending_render();
    let pending_render = state.document.has_pending_render();
    Some((ui_actions, render_due, pending_render))
}

fn select_scad_file() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .add_filter("OpenSCAD", &["scad"])
        .pick_file()
}

fn show_about_dialog(parent: Option<&Window>) {
    let mut dialog = rfd::MessageDialog::new()
        .set_title(format!("关于 {APP_NAME}"))
        .set_description(format!(
            "{APP_NAME}\n版本 {}\n\n支持多窗口，每个窗口对应独立模型。",
            env!("CARGO_PKG_VERSION")
        ))
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::Ok);
    if let Some(window) = parent {
        dialog = dialog.set_parent(window);
    }
    let _ = dialog.show();
}

fn build_paint_data(
    state: &RuntimeState,
    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
) -> EguiPaintData {
    let pixels_per_point = state.window.scale_factor() as f32;
    let clipped_primitives = state.egui_context.tessellate(shapes, pixels_per_point);
    EguiPaintData {
        clipped_primitives,
        textures_delta,
        pixels_per_point,
    }
}

fn load_runtime_config() -> (AppConfig, Option<String>) {
    match load_config() {
        Ok(config) => (config, None),
        Err(error) => (
            AppConfig::default(),
            Some(format!("读取配置失败，已使用默认配置: {error}")),
        ),
    }
}

fn apply_openscad_path_override(config: &AppConfig) {
    if let Some(path) = &config.openscad_path {
        unsafe { std::env::set_var("OPENSCAD_PATH", path) };
    }
}

fn load_document(state: &mut RuntimeState, source_path: PathBuf) -> Result<(), String> {
    let source_text = std::fs::read_to_string(&source_path)
        .map_err(|error| format!("读取源文件失败: {error}"))?;
    state
        .document
        .load_source(source_path.clone(), &source_text);
    refresh_presets(state);
    flush_document_warnings(state);
    state.studio.set_current_file(source_path);
    state.watcher.watch_files(state.document.watch_paths());
    Ok(())
}

fn reload_source_document(state: &mut RuntimeState, source_path: &PathBuf) -> Result<(), String> {
    let source_text =
        std::fs::read_to_string(source_path).map_err(|error| format!("读取源文件失败: {error}"))?;
    state.document.reload_source(&source_text);
    flush_document_warnings(state);
    Ok(())
}

fn refresh_presets(state: &mut RuntimeState) {
    let Some(path) = state.document.preset_path() else {
        return;
    };
    match load_presets(&path) {
        Ok(presets) => state.document.set_presets(presets),
        Err(error) => state.studio.push_log(LogLevel::Warning, error.to_string()),
    }
}

fn flush_document_warnings(state: &mut RuntimeState) {
    for warning in state.document.take_warnings() {
        state.studio.push_log(LogLevel::Warning, warning);
    }
}

fn start_render(state: &mut RuntimeState, source_path: PathBuf) {
    state.studio.set_rendering("正在调用 OpenSCAD 生成 STL");
    state.studio.push_log(
        LogLevel::Info,
        format!("开始渲染 {}", source_path.display()),
    );
    state
        .openscad
        .render_with_defines(source_path, state.document.current_defines());
}

fn export_output_path(
    state: &RuntimeState,
    source_path: &std::path::Path,
    slicer_name: Option<&str>,
) -> Option<PathBuf> {
    if slicer_name.is_some() {
        return Some(std::env::temp_dir().join(build_export_filename(
            source_path,
            state.document.export_format,
        )));
    }
    let file_name = build_export_filename(source_path, state.document.export_format);
    let extension = state.document.export_format.extension();
    rfd::FileDialog::new()
        .set_file_name(file_name)
        .add_filter(extension.to_uppercase(), &[extension])
        .save_file()
}

fn is_scad_file(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("scad"))
}

fn handle_cross_section_event(state: &mut RuntimeState, event: &WindowEvent) -> bool {
    match event {
        WindowEvent::ModifiersChanged(modifiers) => {
            state.ctrl_pressed = modifiers.state().control_key();
            false
        }
        WindowEvent::KeyboardInput { event, .. } => {
            if event.state != ElementState::Pressed
                || !state.studio.viewer_state().clip_plane_enabled
            {
                return false;
            }
            match event.physical_key {
                PhysicalKey::Code(KeyCode::KeyW) => {
                    state.clip_edit_mode = EditMode::Translate;
                    true
                }
                PhysicalKey::Code(KeyCode::KeyE) => {
                    state.clip_edit_mode = EditMode::Rotate;
                    true
                }
                _ => false,
            }
        }
        WindowEvent::MouseInput {
            state: ElementState::Pressed,
            button: winit::event::MouseButton::Left,
            ..
        } => begin_clip_drag(state),
        WindowEvent::MouseInput {
            state: ElementState::Released,
            button: winit::event::MouseButton::Left,
            ..
        } => {
            state.clip_drag_active = false;
            false
        }
        WindowEvent::CursorMoved { position, .. } => {
            let cursor = Vec2::new(position.x as f32, position.y as f32);
            if update_clip_drag(state, cursor) {
                return true;
            }
            state.cursor_position = Some(cursor);
            false
        }
        _ => false,
    }
}

fn begin_clip_drag(state: &mut RuntimeState) -> bool {
    if !state.studio.viewer_state().clip_plane_enabled {
        state.clip_plane.selected = false;
        return false;
    }
    let Some(cursor) = state.cursor_position else {
        return false;
    };
    let ray = scad_scene::cross_section::screen_ray(
        cursor,
        viewport_size(state),
        state
            .camera
            .matrices_for_bounds(state.current_bounds)
            .view_proj
            .inverse(),
    );
    let Some(ray) = ray else {
        return false;
    };
    let Some(distance) = state.clip_plane.ray_intersection(ray.origin, ray.direction) else {
        state.clip_plane.selected = false;
        return false;
    };
    let hit_point = ray.origin + ray.direction * distance;
    if !state.clip_plane.contains_point(hit_point) {
        state.clip_plane.selected = false;
        return false;
    }
    state.clip_plane.selected = true;
    state.clip_drag_active = true;
    true
}

fn update_clip_drag(state: &mut RuntimeState, cursor: Vec2) -> bool {
    if !state.studio.viewer_state().clip_plane_enabled || !state.clip_drag_active {
        return false;
    }
    let previous = state.cursor_position.unwrap_or(cursor);
    let delta = cursor - previous;
    let distance_scale = state
        .camera
        .matrices_for_bounds(state.current_bounds)
        .eye
        .distance(state.clip_plane.center())
        * 0.0025;
    match state.clip_edit_mode {
        EditMode::Translate => {
            let amount = (delta.x - delta.y) * distance_scale;
            state
                .clip_plane
                .translate_along_normal(amount, state.ctrl_pressed);
        }
        EditMode::Rotate => {
            let camera_matrices = state.camera.matrices_for_bounds(state.current_bounds);
            let inverse_view = camera_matrices.view.inverse();
            let right = inverse_view.x_axis.xyz().normalize_or_zero();
            let up = inverse_view.y_axis.xyz().normalize_or_zero();
            let axis = if delta.x.abs() >= delta.y.abs() {
                up
            } else {
                right
            };
            let angle = (delta.x - delta.y) * 0.01;
            state.clip_plane.rotate(angle, axis, state.ctrl_pressed);
        }
    }
    state.cursor_position = Some(cursor);
    true
}

fn viewport_size(state: &RuntimeState) -> Vec2 {
    let size = state.window.inner_size();
    Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32)
}

fn resize_runtime(state: &mut RuntimeState, size: winit::dpi::PhysicalSize<u32>) {
    state.renderer.resize(size);
    state.camera.set_aspect_ratio(state.renderer.aspect_ratio());
    schedule_redraw(state);
}

fn schedule_redraw(state: &mut RuntimeState) {
    if state.redraw_queued {
        return;
    }
    state.redraw_queued = true;
    state.window.request_redraw();
}

fn apply_camera_action(camera: &mut OrbitalCamera, action: app::CameraAction) {
    match action {
        app::CameraAction::SetTargetX(v) => camera.set_target_x(v),
        app::CameraAction::SetTargetY(v) => camera.set_target_y(v),
        app::CameraAction::SetTargetZ(v) => camera.set_target_z(v),
        app::CameraAction::SetDistance(v) => camera.set_distance(v),
        app::CameraAction::SetAzimuth(v) => camera.set_azimuth_degrees(v),
        app::CameraAction::SetElevation(v) => camera.set_elevation_degrees(v),
        app::CameraAction::ResetView => camera.reset_view(None),
        app::CameraAction::ViewTop => camera.view_top(),
        app::CameraAction::ViewBottom => camera.view_bottom(),
        app::CameraAction::ViewFront => camera.view_front(),
        app::CameraAction::ViewBack => camera.view_back(),
        app::CameraAction::ViewLeft => camera.view_left(),
        app::CameraAction::ViewRight => camera.view_right(),
    }
}

#[derive(Debug, Clone, Copy)]
enum ShortcutAction {
    NewWindow,
    OpenFile,
    CloseWindow,
    OpenSettings,
    Quit,
}

fn shortcut_action_for(
    event: &winit::event::KeyEvent,
    modifiers: ModifiersState,
) -> Option<ShortcutAction> {
    let primary = modifiers.super_key() || modifiers.control_key();
    if !primary || event.state != ElementState::Pressed || event.repeat {
        return None;
    }
    match event.physical_key {
        PhysicalKey::Code(KeyCode::KeyN) => Some(ShortcutAction::NewWindow),
        PhysicalKey::Code(KeyCode::KeyO) => Some(ShortcutAction::OpenFile),
        PhysicalKey::Code(KeyCode::KeyW) => Some(ShortcutAction::CloseWindow),
        PhysicalKey::Code(KeyCode::Comma) => Some(ShortcutAction::OpenSettings),
        PhysicalKey::Code(KeyCode::KeyQ) => Some(ShortcutAction::Quit),
        _ => None,
    }
}
