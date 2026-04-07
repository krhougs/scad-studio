mod app;
mod document_session;
mod document_workspace;
mod layout;
mod left_panel;
mod log_panel;
mod markdown_tab;
mod platform_menu;
mod studio_document;
mod viewer_event_routing;
mod viewer_tab;
mod viewer_camera;
mod viewer_viewport;
mod welcome;
mod macos_fused_titlebar;
mod work_area;
mod work_area_frame;
mod workspace;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use app::StudioApp;
use document_session::{DocumentDescriptor, DocumentKind};
use egui::ViewportId;
use layout::LayoutAction;
use markdown_tab::MarkdownTab;
use platform_menu::{APP_NAME, MenuCommand, PlatformMenu};
use scad_data::{AppConfig, FileWatcher, OpenScadMessage, WatchMessage, load_config, save_config};
use scad_scene::{ClipPlane, EguiPaintData, MeshData, OrbitalCamera, RenderSettings, Renderer};
use scad_ui::{document_tabs, font_setup, theme, tab_system::TabId};
use studio_document::StudioDocumentSession;
use viewer_tab::{ViewerTab, ViewerUiOutcome};
use viewer_event_routing::ViewerEventKind;
use workspace::sanitize_recent_workspaces;
use winit::{
    application::ApplicationHandler,
    event::{Modifiers, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    dpi::PhysicalPosition,
    window::{Window, WindowId},
};

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(String),
    OpenScad(WindowId, TabId, OpenScadMessage),
    SourceChanged(WindowId, TabId, PathBuf),
    WatchError(WindowId, TabId, String),
}

struct StudioRuntime {
    window: Arc<Window>,
    renderer: Renderer,
    egui_context: egui::Context,
    egui_state: egui_winit::State,
    app: StudioApp,
    workspace_watcher: FileWatcher,
    redraw_queued: bool,
    modifiers: ModifiersState,
    active_viewer_binding: Option<(TabId, u64)>,
    last_viewport_rect: Option<egui::Rect>,
    last_cursor_position: Option<PhysicalPosition<f64>>,
}

const WORKSPACE_TREE_WATCH_ID: TabId = 0;

struct StudioDesktopApp {
    proxy: EventLoopProxy<UserEvent>,
    platform_menu: Option<PlatformMenu>,
    config: AppConfig,
    windows: HashMap<WindowId, StudioRuntime>,
    last_active_window: Option<WindowId>,
}

struct RedrawResult {
    layout_action: Option<LayoutAction>,
    viewer_outcome: Option<ViewerUiOutcome>,
}

struct ViewerSceneSnapshot {
    binding: Option<(TabId, u64)>,
    mesh: Option<MeshData>,
    camera: OrbitalCamera,
    settings: RenderSettings,
    clip_plane: Option<ClipPlane>,
    viewport_rect: egui::Rect,
}

fn main() {
    env_logger::init();
    let config = load_app_config();
    let platform_menu = PlatformMenu::new(&config.recent_workspaces);
    let mut event_loop_builder = EventLoop::<UserEvent>::with_user_event();
    if let Some(menu) = platform_menu.as_ref() {
        menu.configure_event_loop(&mut event_loop_builder);
    }
    let event_loop = event_loop_builder.build().expect("创建事件循环失败");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    attach_menu_handler(&platform_menu, proxy.clone());
    let mut desktop = StudioDesktopApp {
        proxy,
        platform_menu,
        config,
        windows: HashMap::new(),
        last_active_window: None,
    };
    event_loop.run_app(&mut desktop).expect("运行应用失败");
}

impl ApplicationHandler<UserEvent> for StudioDesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.windows.is_empty() && let Err(error) = self.create_window(event_loop) {
            log::error!("{error}");
            event_loop.exit();
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(id) => self.handle_menu_event(event_loop, &id),
            UserEvent::OpenScad(window_id, tab_id, message) => {
                self.handle_openscad_message(window_id, tab_id, message)
            }
            UserEvent::SourceChanged(window_id, tab_id, path) => {
                self.handle_source_change(window_id, tab_id, path)
            }
            UserEvent::WatchError(window_id, tab_id, message) => {
                self.handle_watch_error(window_id, tab_id, message)
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::Focused(true) | WindowEvent::RedrawRequested) {
            self.last_active_window = Some(window_id);
        }
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
            if let WindowEvent::CursorMoved { position, .. } = event {
                state.last_cursor_position = Some(position);
            }
            match event {
                WindowEvent::CloseRequested => close_window = true,
                WindowEvent::Resized(size) => {
                    resize_runtime(state, size);
                    #[cfg(target_os = "macos")]
                    sync_macos_traffic_lights_with_tab_rail(state.window.as_ref());
                }
                WindowEvent::ScaleFactorChanged { .. } => {
                    let size = state.window.inner_size();
                    resize_runtime(state, size);
                    #[cfg(target_os = "macos")]
                    sync_macos_traffic_lights_with_tab_rail(state.window.as_ref());
                }
                WindowEvent::Focused(true) => {
                    #[cfg(target_os = "macos")]
                    sync_macos_traffic_lights_with_tab_rail(state.window.as_ref());
                }
                WindowEvent::CursorLeft { .. } => {
                    state.last_cursor_position = None;
                }
                WindowEvent::RedrawRequested => {
                    redraw_result = Some(redraw_window(state, &mut self.config));
                }
                other => {
                    let event_kind = viewer_event_kind(&other);
                    let dispatch = viewer_event_routing::dispatch_effects(
                        event_kind,
                        egui_response.consumed,
                    );
                    if dispatch.update_modifiers
                        && let WindowEvent::ModifiersChanged(modifiers) = &other
                    {
                        state.modifiers = modifiers.state();
                    }
                    if dispatch.evaluate_shortcuts
                        && let WindowEvent::KeyboardInput { event, .. } = &other
                    {
                        shortcut_action = shortcut_action_for(event, state.modifiers);
                    }
                    let route_viewer_event = state
                        .app
                        .active_viewer()
                        .is_some_and(|tab| {
                            should_route_viewer_event(
                                state,
                                tab,
                                &other,
                                egui_response.consumed,
                            )
                        });
                    if route_viewer_event
                        && let Some(tab) = state.app.active_viewer_mut()
                        && tab.handle_window_event(
                            &other,
                            state.last_viewport_rect.unwrap_or_else(default_viewport_rect),
                        )
                    {
                        schedule_redraw(state);
                    }
                }
            }
        }
        if let Some(action) = shortcut_action {
            self.apply_shortcut(window_id, event_loop, action);
        }
        if let Some(result) = redraw_result {
            if let Some(action) = result.layout_action {
                self.handle_layout_action(event_loop, window_id, action);
            }
            if let Some(outcome) = result.viewer_outcome {
                self.handle_viewer_outcome(window_id, outcome);
            }
        }
        if close_window {
            self.close_window(event_loop, window_id);
        }
    }
}

impl StudioDesktopApp {
    fn create_window(&mut self, event_loop: &ActiveEventLoop) -> Result<WindowId, String> {
        let runtime = create_runtime(
            event_loop,
            &self.platform_menu,
            &self.config,
            self.proxy.clone(),
        )?;
        let window_id = runtime.window.id();
        self.last_active_window = Some(window_id);
        self.windows.insert(window_id, runtime);
        if let Some(state) = self.windows.get_mut(&window_id) {
            schedule_redraw(state);
        }
        Ok(window_id)
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
                    log::error!("创建 Studio 窗口失败: {error}");
                }
            }
            MenuCommand::OpenFolder => {
                if let Some(path) = select_workspace_folder() {
                    self.open_workspace(path);
                }
            }
            MenuCommand::OpenRecent(path) => self.open_workspace(path),
            MenuCommand::CloseWindow => {
                if let Some(window_id) = self.active_window_id() {
                    self.close_window(event_loop, window_id);
                }
            }
            MenuCommand::ToggleLeftPanel => self.toggle_left_panel(),
            MenuCommand::ToggleLogPanel => self.toggle_log_panel(),
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

    fn handle_layout_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        action: LayoutAction,
    ) {
        match action {
            LayoutAction::OpenFolder => {
                if let Some(path) = select_workspace_folder() {
                    self.open_workspace_in_window(window_id, path);
                }
            }
            LayoutAction::OpenRecent(path) => self.open_workspace_in_window(window_id, path),
            LayoutAction::OpenFile(path) => self.open_workspace_file(window_id, path),
            LayoutAction::SentChat(message) => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state
                        .app
                        .push_log(scad_data::LogLevel::Info, format!("Chat 输入: {message}"));
                    schedule_redraw(state);
                }
            }
        }
        if self.windows.is_empty() {
            event_loop.exit();
        }
    }

    fn handle_viewer_outcome(&mut self, window_id: WindowId, outcome: ViewerUiOutcome) {
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.last_viewport_rect = Some(outcome.viewport_rect);
            let mut needs_save = outcome.save_settings;
            for command in outcome.commands {
                match command {
                    scad_viewer::app::UiCommand::SavePreset(name) => {
                        if let Some(tab) = state.app.active_viewer_mut() {
                            tab.save_preset(name);
                        }
                    }
                    scad_viewer::app::UiCommand::DeletePreset(name) => {
                        if let Some(tab) = state.app.active_viewer_mut() {
                            tab.delete_preset(name);
                        }
                    }
                    scad_viewer::app::UiCommand::ExportModel => {
                        if let Some(tab) = state.app.active_viewer_mut() {
                            tab.export_current_model(&self.config, None);
                        }
                    }
                    scad_viewer::app::UiCommand::SendToSlicer(name) => {
                        if let Some(tab) = state.app.active_viewer_mut() {
                            tab.export_current_model(&self.config, Some(name));
                        }
                    }
                    scad_viewer::app::UiCommand::SaveSettings => needs_save = true,
                }
            }
            if outcome.render_requested
                && let Some(tab) = state.app.active_viewer_mut()
            {
                tab.request_render();
            }
            if outcome.pending_render {
                schedule_redraw(state);
            }
            if needs_save {
                self.save_app_config(window_id);
            }
        }
    }

    fn handle_openscad_message(
        &mut self,
        window_id: WindowId,
        tab_id: TabId,
        message: OpenScadMessage,
    ) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if let Some(session) = state.app.document_by_legacy_tab_id_mut(tab_id)
            && let Some(tab) = session.as_viewer_mut()
        {
            tab.handle_openscad_message(message);
            schedule_redraw(state);
        }
    }

    fn handle_source_change(&mut self, window_id: WindowId, tab_id: TabId, path: PathBuf) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if tab_id == WORKSPACE_TREE_WATCH_ID {
            if let Some(tree) = state.app.file_tree_mut() {
                tree.invalidate(&path);
            }
            schedule_redraw(state);
            return;
        }
        if let Some(session) = state.app.document_by_legacy_tab_id_mut(tab_id) {
            if let Some(tab) = session.as_viewer_mut() {
                tab.handle_source_change(&path);
            } else if let Some(tab) = session.as_markdown_mut()
                && tab.path() == path.as_path()
                && let Err(error) = tab.reload()
            {
                state.app.push_log(scad_data::LogLevel::Error, error);
            }
            schedule_redraw(state);
        }
    }

    fn handle_watch_error(&mut self, window_id: WindowId, tab_id: TabId, message: String) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if tab_id == WORKSPACE_TREE_WATCH_ID {
            state.app.push_log(scad_data::LogLevel::Error, message);
            schedule_redraw(state);
            return;
        }
        if let Some(session) = state.app.document_by_legacy_tab_id_mut(tab_id) {
            if let Some(tab) = session.as_viewer_mut() {
                tab.handle_watch_error(message);
            } else {
                state.app.push_log(scad_data::LogLevel::Error, message);
            }
            schedule_redraw(state);
        }
    }

    fn apply_shortcut(
        &mut self,
        window_id: WindowId,
        event_loop: &ActiveEventLoop,
        action: ShortcutAction,
    ) {
        match action {
            ShortcutAction::NewWindow => {
                let _ = self.create_window(event_loop);
            }
            ShortcutAction::OpenFolder => {
                if let Some(path) = select_workspace_folder() {
                    self.open_workspace_in_window(window_id, path);
                }
            }
            ShortcutAction::CloseWindow => {
                self.close_window(event_loop, window_id);
            }
            ShortcutAction::ToggleLeftPanel => {
                self.toggle_left_panel();
            }
            ShortcutAction::ToggleLogPanel => {
                self.toggle_log_panel();
            }
            ShortcutAction::Quit => {
                event_loop.exit();
            }
        }
    }

    fn active_window_id(&self) -> Option<WindowId> {
        self.last_active_window
            .filter(|window_id| self.windows.contains_key(window_id))
            .or_else(|| self.windows.keys().next().copied())
    }

    fn open_workspace(&mut self, path: PathBuf) {
        let Some(window_id) = self.active_window_id() else {
            return;
        };
        self.open_workspace_in_window(window_id, path);
    }

    fn open_workspace_in_window(&mut self, window_id: WindowId, path: PathBuf) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if !path.is_dir() {
            state.app.push_log(
                scad_data::LogLevel::Error,
                format!("Workspace 不存在: {}", path.display()),
            );
            schedule_redraw(state);
            return;
        }
        state.app.set_workspace_path(path);
        if let Some(workspace_path) = state.app.workspace_path().map(PathBuf::from) {
            state.workspace_watcher.watch_files(vec![workspace_path]);
        }
        state.window.set_title(&state.app.window_title());
        self.config.recent_workspaces = state.app.recent_workspaces().to_vec();
        if let Err(error) = save_config(&self.config) {
            state.app.push_log(
                scad_data::LogLevel::Warning,
                format!("保存最近工作区失败: {error}"),
            );
        }
        schedule_redraw(state);
        self.refresh_platform_menu();
    }

    fn open_workspace_file(&mut self, window_id: WindowId, path: PathBuf) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if !path.is_file() {
            state.app.push_log(
                scad_data::LogLevel::Warning,
                format!("文件不存在: {}", path.display()),
            );
            schedule_redraw(state);
            return;
        }
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let Some(kind) = document_kind_for_extension(&extension) else {
            state.app.push_log(
                scad_data::LogLevel::Error,
                format!("暂不支持的文件类型: {}", path.display()),
            );
            schedule_redraw(state);
            return;
        };
        let descriptor = DocumentDescriptor::new(kind, path.clone());
        if state.app.contains_document(&descriptor.key)
        {
            state.app.set_active_document(descriptor.key);
            schedule_redraw(state);
            return;
        }
        let open_result = match kind {
            DocumentKind::Viewer => ViewerTab::open(
                path.clone(),
                state.renderer.aspect_ratio(),
                self.proxy.clone(),
                window_id,
            )
            .map(StudioDocumentSession::Viewer),
            DocumentKind::Markdown => {
                MarkdownTab::open(path.clone(), self.proxy.clone(), window_id)
                    .map(StudioDocumentSession::Markdown)
            }
        };
        match open_result {
            Ok(document) => {
                let _ = state.app.open_document(document);
            }
            Err(error) => state.app.push_log(scad_data::LogLevel::Error, error),
        }
        schedule_redraw(state);
    }

    fn close_window(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId) {
        self.windows.remove(&window_id);
        if self.last_active_window == Some(window_id) {
            self.last_active_window = self.windows.keys().next().copied();
        }
        if self.windows.is_empty() {
            event_loop.exit();
        }
    }

    fn toggle_left_panel(&mut self) {
        let Some(window_id) = self.active_window_id() else {
            return;
        };
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        state.app.toggle_left_panel();
        schedule_redraw(state);
    }

    fn toggle_log_panel(&mut self) {
        let Some(window_id) = self.active_window_id() else {
            return;
        };
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        state.app.toggle_log_panel();
        schedule_redraw(state);
    }

    fn save_app_config(&mut self, window_id: WindowId) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        match save_config(&self.config) {
            Ok(()) => state.app.push_log(scad_data::LogLevel::Info, "配置已保存"),
            Err(error) => state.app.push_log(scad_data::LogLevel::Error, error.to_string()),
        }
    }

    fn refresh_platform_menu(&mut self) {
        let menu = PlatformMenu::new(&self.config.recent_workspaces);
        attach_menu_handler(&menu, self.proxy.clone());
        for state in self.windows.values() {
            if let Some(menu_ref) = menu.as_ref()
                && let Err(error) = menu_ref.install(state.window.as_ref())
            {
                log::warn!("刷新 Studio 菜单失败: {error}");
            }
        }
        self.platform_menu = menu;
    }
}

fn attach_menu_handler(menu: &Option<PlatformMenu>, proxy: EventLoopProxy<UserEvent>) {
    if let Some(menu) = menu.as_ref() {
        menu.attach_event_handler(proxy);
    }
}

fn create_runtime(
    event_loop: &ActiveEventLoop,
    platform_menu: &Option<PlatformMenu>,
    config: &AppConfig,
    proxy: EventLoopProxy<UserEvent>,
) -> Result<StudioRuntime, String> {
    let app = StudioApp::new(config.recent_workspaces.clone());
    let window_attrs = macos_fused_titlebar::apply_macos_fused_titlebar_attributes(
        Window::default_attributes()
            .with_title(app.window_title())
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0)),
    );
    let window = Arc::new(
        event_loop
            .create_window(window_attrs)
            .map_err(|error| format!("创建 Studio 窗口失败: {error}"))?,
    );
    let renderer = pollster::block_on(Renderer::new(window.clone()))
        .map_err(|error| format!("初始化 Studio 渲染器失败: {error}"))?;
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
    if let Some(menu) = platform_menu.as_ref() {
        menu.install(window.as_ref())?;
    }
    let workspace_watcher = FileWatcher::new(build_workspace_notifier(proxy, window.id()));
    Ok(StudioRuntime {
        window,
        renderer,
        egui_context,
        egui_state,
        app,
        workspace_watcher,
        redraw_queued: false,
        modifiers: Modifiers::default().state(),
        active_viewer_binding: None,
        last_viewport_rect: None,
        last_cursor_position: None,
    })
}

#[cfg(target_os = "macos")]
fn sync_macos_traffic_lights_with_tab_rail(window: &Window) {
    macos_fused_titlebar::sync_traffic_lights_with_tab_rail(
        window,
        document_tabs::tab_rail_pills_center_y_from_strip_top(),
        document_tabs::tab_height(),
    );
}

fn redraw_window(state: &mut StudioRuntime, config: &mut AppConfig) -> RedrawResult {
    state.redraw_queued = false;
    #[cfg(target_os = "macos")]
    sync_macos_traffic_lights_with_tab_rail(state.window.as_ref());
    let raw_input = state.egui_state.take_egui_input(&state.window);
    let mut layout_action = None;
    let mut viewer_outcome = None;
    let full_output = state.egui_context.run(raw_input, |ctx| {
        theme::apply(ctx);
        let (layout, outcome) = layout::show(ctx, &mut state.app, config);
        layout_action = layout;
        viewer_outcome = outcome;
    });
    state
        .egui_state
        .handle_platform_output(&state.window, full_output.platform_output);
    state.last_viewport_rect = viewer_outcome.as_ref().map(|outcome| outcome.viewport_rect);
    let paint_data = build_paint_data(state, full_output.shapes, full_output.textures_delta);
    if let Err(error) = render_ui(state, paint_data) {
        log::error!("渲染 Studio 界面失败: {error}");
    }
    RedrawResult {
        layout_action,
        viewer_outcome,
    }
}

fn build_paint_data(
    state: &StudioRuntime,
    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
) -> EguiPaintData {
    let pixels_per_point = state.window.scale_factor() as f32;
    EguiPaintData {
        clipped_primitives: state.egui_context.tessellate(shapes, pixels_per_point),
        textures_delta,
        pixels_per_point,
    }
}

fn render_ui(state: &mut StudioRuntime, paint_data: EguiPaintData) -> Result<(), String> {
    let snapshot = active_viewer_snapshot(&state.app, state.last_viewport_rect);
    sync_active_viewer_mesh(&mut state.renderer, &mut state.active_viewer_binding, snapshot.as_ref());
    if let Some(snapshot) = snapshot.as_ref() {
        return state
            .renderer
            .render(
                &snapshot.camera,
                &snapshot.settings,
                snapshot.clip_plane.as_ref(),
                Some(rect_to_viewport(snapshot.viewport_rect)),
                paint_data,
            )
            .map_err(|error| error.to_string());
    }
    state
        .renderer
        .render_egui_only(paint_data)
        .map_err(|error| error.to_string())
}

fn active_viewer_snapshot(
    app: &StudioApp,
    viewport_rect: Option<egui::Rect>,
) -> Option<ViewerSceneSnapshot> {
    let tab = app.active_viewer()?;
    let viewport_rect = viewport_rect?;
    let mut camera = *tab.camera();
    camera.set_aspect_ratio(viewport_aspect_ratio(viewport_rect));
    Some(ViewerSceneSnapshot {
        binding: tab.mesh_signature(),
        mesh: tab.mesh().cloned(),
        camera,
        settings: tab.render_settings(),
        clip_plane: tab.clip_plane().copied(),
        viewport_rect,
    })
}

fn sync_active_viewer_mesh(
    renderer: &mut Renderer,
    active_binding: &mut Option<(TabId, u64)>,
    snapshot: Option<&ViewerSceneSnapshot>,
) {
    let Some(snapshot) = snapshot else {
        if active_binding.take().is_some() {
            renderer.clear_mesh();
        }
        return;
    };
    if *active_binding == snapshot.binding {
        return;
    }
    if let Some(mesh) = snapshot.mesh.clone() {
        renderer.set_mesh(mesh);
    } else {
        renderer.clear_mesh();
    }
    *active_binding = snapshot.binding;
}

fn resize_runtime(state: &mut StudioRuntime, size: winit::dpi::PhysicalSize<u32>) {
    state.renderer.resize(size);
    schedule_redraw(state);
}

fn schedule_redraw(state: &mut StudioRuntime) {
    if state.redraw_queued {
        return;
    }
    state.redraw_queued = true;
    state.window.request_redraw();
}

fn load_app_config() -> AppConfig {
    match load_config() {
        Ok(mut config) => {
            config.recent_workspaces = sanitize_recent_workspaces(&config.recent_workspaces);
            config
        }
        Err(error) => {
            log::warn!("读取 Studio 配置失败，已使用默认值: {error}");
            AppConfig::default()
        }
    }
}

fn select_workspace_folder() -> Option<PathBuf> {
    rfd::FileDialog::new().pick_folder()
}

fn should_route_viewer_event(
    state: &StudioRuntime,
    tab: &ViewerTab,
    event: &WindowEvent,
    egui_consumed: bool,
) -> bool {
    let event_kind = viewer_event_kind(event);
    if matches!(event_kind, ViewerEventKind::KeyboardInput) {
        return !egui_consumed;
    }
    viewer_event_routing::should_route_event(
        event_kind,
        point_in_viewport_from_cursor(state),
        current_pointer_layer_order(state),
        tab.captures_pointer(),
    )
}

fn viewer_event_kind(event: &WindowEvent) -> ViewerEventKind {
    match event {
        WindowEvent::CursorMoved { .. } => ViewerEventKind::CursorMoved,
        WindowEvent::MouseWheel { .. } => ViewerEventKind::MouseWheel,
        WindowEvent::MouseInput {
            state: winit::event::ElementState::Pressed,
            ..
        } => ViewerEventKind::MousePressed,
        WindowEvent::MouseInput {
            state: winit::event::ElementState::Released,
            ..
        } => ViewerEventKind::MouseReleased,
        WindowEvent::KeyboardInput { .. } => ViewerEventKind::KeyboardInput,
        WindowEvent::ModifiersChanged { .. } => ViewerEventKind::ModifiersChanged,
        _ => ViewerEventKind::Other,
    }
}

fn point_in_viewport_from_cursor(state: &StudioRuntime) -> bool {
    let Some(position) = current_pointer_position(state) else {
        return false;
    };
    point_in_viewport(state.last_viewport_rect, position)
}

fn current_pointer_layer_order(state: &StudioRuntime) -> Option<egui::Order> {
    current_pointer_position_in_points(state)
        .and_then(|position| state.egui_context.layer_id_at(position))
        .map(|layer_id| layer_id.order)
}

fn current_pointer_position(state: &StudioRuntime) -> Option<PhysicalPosition<f64>> {
    state.last_cursor_position.or_else(|| {
        current_pointer_position_in_points(state).map(|position| {
            let pixels_per_point = state.egui_context.pixels_per_point() as f64;
            PhysicalPosition::new(
                position.x as f64 * pixels_per_point,
                position.y as f64 * pixels_per_point,
            )
        })
    })
}

fn current_pointer_position_in_points(state: &StudioRuntime) -> Option<egui::Pos2> {
    state
        .egui_context
        .input(|input| input.pointer.latest_pos())
}

fn point_in_viewport(
    viewport_rect: Option<egui::Rect>,
    position: PhysicalPosition<f64>,
) -> bool {
    viewport_rect.is_some_and(|rect| {
        rect.contains(egui::pos2(position.x as f32, position.y as f32))
    })
}

fn default_viewport_rect() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1.0, 1.0))
}

fn rect_to_viewport(rect: egui::Rect) -> [f32; 4] {
    [rect.min.x, rect.min.y, rect.width(), rect.height()]
}

fn viewport_aspect_ratio(viewport_rect: egui::Rect) -> f32 {
    (viewport_rect.width() / viewport_rect.height().max(1.0)).max(0.1)
}

fn document_kind_for_extension(extension: &str) -> Option<DocumentKind> {
    match extension {
        "scad" | "stl" | "3mf" => Some(DocumentKind::Viewer),
        "md" | "markdown" => Some(DocumentKind::Markdown),
        _ => None,
    }
}

fn build_workspace_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
) -> impl Fn(WatchMessage) + Send + 'static {
    move |message| match message {
        WatchMessage::Changed(path) => {
            let _ = proxy.send_event(UserEvent::SourceChanged(
                window_id,
                WORKSPACE_TREE_WATCH_ID,
                path,
            ));
        }
        WatchMessage::Error(message) => {
            let _ = proxy.send_event(UserEvent::WatchError(
                window_id,
                WORKSPACE_TREE_WATCH_ID,
                message,
            ));
        }
    }
}

#[derive(Clone, Copy)]
enum ShortcutAction {
    NewWindow,
    OpenFolder,
    CloseWindow,
    ToggleLeftPanel,
    ToggleLogPanel,
    Quit,
}

fn shortcut_action_for(
    event: &winit::event::KeyEvent,
    modifiers: ModifiersState,
) -> Option<ShortcutAction> {
    let primary = modifiers.super_key() || modifiers.control_key();
    if !primary || event.state != winit::event::ElementState::Pressed || event.repeat {
        return None;
    }
    match event.physical_key {
        PhysicalKey::Code(KeyCode::KeyN) => Some(ShortcutAction::NewWindow),
        PhysicalKey::Code(KeyCode::KeyO) => Some(ShortcutAction::OpenFolder),
        PhysicalKey::Code(KeyCode::KeyW) => Some(ShortcutAction::CloseWindow),
        PhysicalKey::Code(KeyCode::KeyB) => Some(ShortcutAction::ToggleLeftPanel),
        PhysicalKey::Code(KeyCode::KeyJ) => Some(ShortcutAction::ToggleLogPanel),
        PhysicalKey::Code(KeyCode::KeyQ) => Some(ShortcutAction::Quit),
        _ => None,
    }
}

fn show_about_dialog(parent: Option<&Window>) {
    let mut dialog = rfd::MessageDialog::new()
        .set_title(format!("关于 {APP_NAME}"))
        .set_description("SCAD Studio\n\n支持多窗口，每个窗口对应独立 Workspace。")
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::Ok);
    if let Some(window) = parent {
        dialog = dialog.set_parent(window);
    }
    let _ = dialog.show();
}
