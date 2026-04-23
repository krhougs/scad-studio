use std::{
    any::Any,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use egui::{Align, Color32, CornerRadius, Layout, Pos2, Rect, Sense, Vec2};
use scad_ui::{
    image_decode,
    image_zoom_math::pan_after_zoom_to_focal,
    tab_system::{TabContext, TabId, WorkTab},
    theme, viewer_viewport,
};
use winit::{event_loop::EventLoopProxy, window::WindowId};

use crate::{
    UserEvent,
    protocol_client::{DesktopProtocolClient, WatchSubscriptionHandle},
};

const ZOOM_STEP: f32 = 1.15;
const ZOOM_MIN: f32 = 0.05;
const ZOOM_MAX: f32 = 32.0;
const WHEEL_ZOOM_FACTOR: f32 = 0.0012;

#[derive(Clone, Copy, Debug, PartialEq)]
enum ImageZoomState {
    Fit,
    Absolute(f32),
}

pub struct ImageTab {
    id: TabId,
    path: PathBuf,
    title: String,
    client: DesktopProtocolClient,
    texture: Option<egui::TextureHandle>,
    load_error: Option<String>,
    _watch_subscription: WatchSubscriptionHandle,
    zoom: ImageZoomState,
    pan: Vec2,
    pending_toolbar_zoom: Option<bool>,
}

impl ImageTab {
    pub fn open(
        client: DesktopProtocolClient,
        path: PathBuf,
        proxy: EventLoopProxy<UserEvent>,
        window_id: WindowId,
    ) -> Result<Self, String> {
        let id = tab_id_for_path("image", &path);
        let watch_subscription = client.subscribe_path(
            &path,
            build_changed_notifier(proxy.clone(), window_id, id),
            build_error_notifier(proxy, window_id, id),
        )?;
        let tab = Self {
            id,
            path: path.clone(),
            title: file_label(&path),
            client,
            texture: None,
            load_error: None,
            _watch_subscription: watch_subscription,
            zoom: ImageZoomState::Fit,
            pan: Vec2::ZERO,
            pending_toolbar_zoom: None,
        };
        Ok(tab)
    }

    pub fn legacy_tab_id(&self) -> TabId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn invalidate_texture(&mut self) {
        self.texture = None;
        self.load_error = None;
        self.zoom = ImageZoomState::Fit;
        self.pan = Vec2::ZERO;
        self.pending_toolbar_zoom = None;
    }

    pub fn show_document(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.texture.is_none() && self.load_error.is_none() {
            self.try_load_texture(ctx);
        }
        if let Some(err) = &self.load_error {
            ui.colored_label(egui::Color32::RED, err);
            return;
        }
        let Some(texture) = self.texture.clone() else {
            ui.label("加载中…");
            return;
        };
        let tex_size = texture.size_vec2();
        if tex_size.x < 1.0 || tex_size.y < 1.0 {
            return;
        }
        self.paint_loaded_image(ui, &texture, tex_size);
    }

    fn paint_loaded_image(
        &mut self,
        ui: &mut egui::Ui,
        texture: &egui::TextureHandle,
        tex_size: Vec2,
    ) {
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            let toolbar_h = theme::palette::TAB_STRIP_GAP_BELOW.max(8.0) + 28.0;
            let bar_w = ui.available_width();
            let canvas_preview_h = (ui.available_height() - toolbar_h).max(1.0);
            let _ = viewer_viewport::allocate_filled_strip_ui(
                ui,
                Vec2::new(bar_w, toolbar_h),
                egui::Margin::symmetric(8, 4),
                theme::palette::BG_PANEL,
                Layout::left_to_right(Align::Center),
                |ui| {
                    self.paint_zoom_toolbar(ui, tex_size, Vec2::new(bar_w, canvas_preview_h));
                },
            );

            let canvas_h = ui.available_height();
            let (response, painter) = ui.allocate_painter(
                Vec2::new(ui.available_width(), canvas_h.max(1.0)),
                Sense::click_and_drag().union(Sense::hover()),
            );
            let canvas = response.rect;
            let viewport_center = canvas.center();

            if let Some(zoom_in) = self.pending_toolbar_zoom.take() {
                self.apply_zoom_step(viewport_center, tex_size, canvas.size(), zoom_in);
            }

            let fit_s = fit_scale(canvas.size(), tex_size);
            if matches!(self.zoom, ImageZoomState::Absolute(_)) && response.dragged() {
                self.pan += response.drag_delta();
            }

            let eff_before_gesture = effective_scale(self.zoom, fit_s);
            self.apply_gesture_zoom(&response, viewport_center, tex_size, eff_before_gesture);

            let eff = effective_scale(self.zoom, fit_s);
            let display = tex_size * eff;
            let center = viewport_center + self.pan;
            let img_rect = Rect::from_center_size(Pos2::new(center.x, center.y), display);

            let mut clip = painter.clone();
            clip.set_clip_rect(canvas);
            clip.rect_filled(canvas, CornerRadius::ZERO, theme::palette::BG_WINDOW);
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            clip.image(texture.id(), img_rect, uv, Color32::WHITE);
        });
    }

    fn paint_zoom_toolbar(&mut self, ui: &mut egui::Ui, tex_size: Vec2, canvas_preview: Vec2) {
        let preview_fit = fit_scale(canvas_preview, tex_size);
        let eff_label = effective_scale(self.zoom, preview_fit);
        let pct = (eff_label * 100.0).round();

        if ui.button("−").clicked() {
            self.pending_toolbar_zoom = Some(false);
        }
        if ui.button("100%").clicked() {
            self.zoom = ImageZoomState::Absolute(1.0);
            self.pan = Vec2::ZERO;
        }
        if ui.button("+").clicked() {
            self.pending_toolbar_zoom = Some(true);
        }
        ui.label(format!("{pct:.0}%"));
    }

    fn apply_zoom_step(
        &mut self,
        viewport_center: Pos2,
        tex_size: Vec2,
        canvas_size: Vec2,
        zoom_in: bool,
    ) {
        let fit_s = fit_scale(canvas_size, tex_size);
        let old = effective_scale(self.zoom, fit_s);
        let factor = if zoom_in { ZOOM_STEP } else { 1.0 / ZOOM_STEP };
        let new = clamp_abs(old * factor);
        self.zoom = ImageZoomState::Absolute(new);
        if let Some(p) = pan_after_zoom_to_focal(
            viewport_center,
            self.pan,
            tex_size,
            old,
            new,
            viewport_center,
        ) {
            self.pan = p;
        }
    }

    fn apply_gesture_zoom(
        &mut self,
        response: &egui::Response,
        viewport_center: Pos2,
        tex_size: Vec2,
        eff_before: f32,
    ) {
        if !response.hovered() {
            return;
        }

        let focal = response.hover_pos().unwrap_or(viewport_center);
        let ctx = response.ctx.clone();

        let zd = ctx.input(|i| i.zoom_delta());
        if (zd - 1.0).abs() > 1e-4 {
            let new = clamp_abs(eff_before * zd);
            self.zoom = ImageZoomState::Absolute(new);
            if let Some(p) =
                pan_after_zoom_to_focal(viewport_center, self.pan, tex_size, eff_before, new, focal)
            {
                self.pan = p;
            }
            return;
        }

        let scroll = ctx.input(|i| i.raw_scroll_delta.y);
        if scroll == 0.0 {
            return;
        }
        let factor = 1.0 + scroll * WHEEL_ZOOM_FACTOR;
        if factor <= 0.0 {
            return;
        }
        let new = clamp_abs(eff_before * factor);
        self.zoom = ImageZoomState::Absolute(new);
        if let Some(p) =
            pan_after_zoom_to_focal(viewport_center, self.pan, tex_size, eff_before, new, focal)
        {
            self.pan = p;
        }
    }

    fn try_load_texture(&mut self, ctx: &egui::Context) {
        let bytes = match self.client.read_binary_file(&self.path, "图片") {
            Ok(b) => b,
            Err(e) => {
                self.load_error = Some(format!("读取图片失败: {e}"));
                return;
            }
        };
        let (w, h, rgba) = match image_decode::rgba_from_image_bytes(&bytes) {
            Ok(v) => v,
            Err(e) => {
                self.load_error = Some(e);
                return;
            }
        };
        let size = [w as usize, h as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, &rgba);
        self.texture = Some(ctx.load_texture(
            format!("studio_image_{}", self.id),
            color,
            egui::TextureOptions::LINEAR,
        ));
        self.zoom = ImageZoomState::Fit;
        self.pan = Vec2::ZERO;
        self.pending_toolbar_zoom = None;
    }
}

impl WorkTab for ImageTab {
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
        let ctx = ui.ctx().clone();
        self.show_document(&ctx, ui);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn fit_scale(avail: Vec2, tex: Vec2) -> f32 {
    if tex.x < 1e-6 || tex.y < 1e-6 {
        return 1.0;
    }
    (avail.x / tex.x).min(avail.y / tex.y)
}

fn clamp_abs(s: f32) -> f32 {
    s.clamp(ZOOM_MIN, ZOOM_MAX)
}

fn effective_scale(zoom: ImageZoomState, fit_s: f32) -> f32 {
    match zoom {
        ImageZoomState::Fit => fit_s,
        ImageZoomState::Absolute(s) => clamp_abs(s),
    }
}

fn tab_id_for_path(kind: &str, path: &Path) -> TabId {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    kind.hash(&mut hasher);
    path.hash(&mut hasher);
    hasher.finish()
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("图片")
        .to_owned()
}

fn build_changed_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(PathBuf) + Send + 'static {
    move |path| {
        let _ = proxy.send_event(UserEvent::SourceChanged(window_id, tab_id, path));
    }
}

fn build_error_notifier(
    proxy: EventLoopProxy<UserEvent>,
    window_id: WindowId,
    tab_id: TabId,
) -> impl Fn(String) + Send + 'static {
    move |message| {
        let _ = proxy.send_event(UserEvent::WatchError(window_id, tab_id, message));
    }
}
