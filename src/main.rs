mod app;
mod camera;
mod mesh;
mod openscad;
mod platform_menu;
mod renderer;
mod system_fonts;
mod watcher;

use std::{path::PathBuf, sync::Arc};

use app::StudioApp;
use camera::{CameraInteraction, OrbitalCamera};
use egui::ViewportId;
use openscad::{OpenScadMessage, OpenScadRunner, RenderedArtifact};
use platform_menu::{MenuCommand, PlatformMenu, APP_NAME};
use renderer::{EguiPaintData, Renderer};
use watcher::{FileWatcher, WatchMessage};
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
    window::{Window, WindowId},
};

#[derive(Debug, Clone)]
enum UserEvent {
    OpenScad(OpenScadMessage),
    SourceChanged(PathBuf),
    WatchError(String),
    Menu(String),
}

struct DesktopApp {
    proxy: EventLoopProxy<UserEvent>,
    platform_menu: Option<PlatformMenu>,
    state: Option<RuntimeState>,
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
    redraw_queued: bool,
}

fn main() {
    env_logger::init();
    let platform_menu = PlatformMenu::new();
    let mut event_loop_builder = EventLoop::<UserEvent>::with_user_event();
    if let Some(menu) = platform_menu.as_ref() {
        menu.configure_event_loop(&mut event_loop_builder);
    }
    let event_loop = event_loop_builder
        .build()
        .expect("创建事件循环失败");
    event_loop.set_control_flow(ControlFlow::Wait);
    let proxy = event_loop.create_proxy();
    if let Some(menu) = platform_menu.as_ref() {
        menu.attach_event_handler(proxy.clone());
    }
    let mut app = DesktopApp {
        proxy,
        platform_menu,
        state: None,
    };
    event_loop.run_app(&mut app).expect("运行应用失败");
}

impl ApplicationHandler<UserEvent> for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match self.build_runtime(event_loop) {
            Ok(state) => self.state = Some(state),
            Err(error) => {
                log::error!("{error}");
                event_loop.exit();
            }
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::OpenScad(message) => self.handle_openscad_message(message),
            UserEvent::SourceChanged(path) => self.handle_source_change(path),
            UserEvent::WatchError(message) => self.handle_watch_error(message),
            UserEvent::Menu(id) => self.handle_menu_event(event_loop, id),
        }
        if let Some(state) = self.state.as_mut() {
            schedule_redraw(state);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        if window_id != state.window.id() {
            return;
        }
        let egui_response = state.egui_state.on_window_event(&state.window, &event);
        if egui_response.repaint {
            schedule_redraw(state);
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => resize_runtime(state, size),
            WindowEvent::ScaleFactorChanged { .. } => {
                let size = state.window.inner_size();
                resize_runtime(state, size);
            }
            WindowEvent::RedrawRequested => self.redraw(),
            other if !egui_response.consumed => {
                if state
                    .camera_interaction
                    .handle_event(&mut state.camera, &other)
                {
                    schedule_redraw(state);
                }
            }
            _ => {}
        }
    }
}

impl DesktopApp {
    fn build_runtime(&self, event_loop: &ActiveEventLoop) -> Result<RuntimeState, String> {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title(APP_NAME)
                        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 800.0)),
                )
                .map_err(|error| format!("创建窗口失败: {error}"))?,
        );
        let renderer = pollster::block_on(Renderer::new(window.clone()))
            .map_err(|error| format!("初始化渲染器失败: {error}"))?;
        let egui_context = egui::Context::default();
        match system_fonts::configure_egui_fonts(&egui_context) {
            Ok(paths) if !paths.is_empty() => {
                log::info!("已加载 {} 个系统字体回退项", paths.len());
            }
            Ok(_) => {
                log::warn!("未获取到系统字体回退项，继续使用 egui 默认字体");
            }
            Err(error) => {
                log::warn!("加载系统字体回退链失败: {error}");
            }
        }
        let egui_state = egui_winit::State::new(
            egui_context.clone(),
            ViewportId::ROOT,
            window.as_ref(),
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(renderer.max_texture_side()),
        );
        let camera = OrbitalCamera::new(renderer.aspect_ratio());
        let openscad = OpenScadRunner::new(self.build_openscad_notifier());
        let watcher = FileWatcher::new(self.build_source_notifier());
        if let Some(menu) = self.platform_menu.as_ref() {
            menu.install(window.as_ref())?;
        }
        let runtime = RuntimeState {
            window,
            renderer,
            egui_context,
            egui_state,
            studio: StudioApp::default(),
            camera,
            camera_interaction: CameraInteraction::default(),
            openscad,
            watcher,
            redraw_queued: false,
        };
        let mut runtime = runtime;
        schedule_redraw(&mut runtime);
        Ok(runtime)
    }

    fn build_openscad_notifier(&self) -> impl Fn(OpenScadMessage) + Send + 'static {
        let proxy = self.proxy.clone();
        move |message| {
            let _ = proxy.send_event(UserEvent::OpenScad(message));
        }
    }

    fn build_source_notifier(&self) -> impl Fn(WatchMessage) + Send + 'static {
        let proxy = self.proxy.clone();
        move |message| match message {
            WatchMessage::Changed(path) => {
                let _ = proxy.send_event(UserEvent::SourceChanged(path));
            }
            WatchMessage::Error(message) => {
                let _ = proxy.send_event(UserEvent::WatchError(message));
            }
        }
    }

    fn redraw(&mut self) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        state.redraw_queued = false;
        let raw_input = state.egui_state.take_egui_input(&state.window);
        let mut ui_actions = app::UiActions::default();
        let show_embedded_menu = self.platform_menu.is_none();
        let full_output = state.egui_context.run(raw_input, |ctx| {
            ui_actions = state.studio.ui(ctx, show_embedded_menu);
        });
        state
            .egui_state
            .handle_platform_output(&state.window, full_output.platform_output);
        let pixels_per_point = state.window.scale_factor() as f32;
        let clipped_primitives = state
            .egui_context
            .tessellate(full_output.shapes, pixels_per_point);
        let paint_data = EguiPaintData {
            clipped_primitives,
            textures_delta: full_output.textures_delta,
            pixels_per_point,
        };
        if let Err(error) = state.renderer.render(&state.camera, paint_data) {
            state.studio.set_error(format!("渲染失败: {error}"));
        }
        if ui_actions.open_file {
            if let Some(path) = select_scad_file() {
                self.open_source_file(path);
            }
        }
    }

    fn open_source_file(&mut self, source_path: PathBuf) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        state.studio.set_current_file(source_path.clone());
        state.studio.set_rendering("正在调用 OpenSCAD 生成 STL");
        state.watcher.watch(source_path.clone());
        state.openscad.render(source_path);
    }

    fn handle_openscad_message(&mut self, message: OpenScadMessage) {
        match message {
            OpenScadMessage::Started(path) => self.handle_render_started(path),
            OpenScadMessage::Finished(result) => self.handle_render_finished(result),
        }
    }

    fn handle_render_started(&mut self, path: PathBuf) {
        if let Some(state) = self.state.as_mut() {
            state.studio.set_current_file(path);
            state.studio.set_rendering("OpenSCAD 正在渲染模型");
        }
    }

    fn handle_render_finished(
        &mut self,
        result: Result<RenderedArtifact, openscad::OpenScadError>,
    ) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match result {
            Ok(artifact) => {
                state.studio.set_current_file(artifact.source_path.clone());
                state.camera.fit_bounds(artifact.mesh.bounds);
                state.renderer.set_mesh(artifact.mesh);
                state.studio.set_ready("预览已更新");
            }
            Err(error) => {
                state.renderer.clear_mesh();
                state.studio.set_error(error.to_string());
            }
        }
    }

    fn handle_source_change(&mut self, path: PathBuf) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        let is_current = state.studio.current_file() == Some(path.as_path());
        if !is_current {
            return;
        }
        state.studio.set_rendering("检测到文件变更，正在重新渲染");
        state.openscad.render(path);
    }

    fn handle_watch_error(&mut self, message: String) {
        if let Some(state) = self.state.as_mut() {
            state.studio.set_error(message);
        }
    }

    fn handle_menu_event(&mut self, event_loop: &ActiveEventLoop, id: String) {
        let Some(menu) = self.platform_menu.as_ref() else {
            return;
        };
        match menu.command_for_event(&id) {
            Some(MenuCommand::OpenFile) => {
                if let Some(path) = select_scad_file() {
                    self.open_source_file(path);
                }
            }
            Some(MenuCommand::ShowAbout) => {
                let parent = self.state.as_ref().map(|state| state.window.as_ref());
                show_about_dialog(parent);
            }
            Some(MenuCommand::QuitApp) => event_loop.exit(),
            None => {}
        }
    }
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
            "{APP_NAME}\n版本 {}\n\n用于打开 .scad 文件并预览 OpenSCAD 生成的三维模型。",
            env!("CARGO_PKG_VERSION")
        ))
        .set_level(rfd::MessageLevel::Info)
        .set_buttons(rfd::MessageButtons::Ok);
    if let Some(window) = parent {
        dialog = dialog.set_parent(window);
    }
    let _ = dialog.show();
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
