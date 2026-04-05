use egui::{Color32, CornerRadius, Margin, Stroke, Visuals};

/// 调色板常量，供各 UI 模块引用
pub mod palette {
    use egui::Color32;

    pub const BG_PANEL: Color32 = Color32::from_rgb(14, 14, 14);
    pub const BG_WINDOW: Color32 = Color32::from_rgb(20, 20, 20);
    pub const BG_WIDGET: Color32 = Color32::from_rgb(30, 30, 30);
    pub const BG_WIDGET_HOVER: Color32 = Color32::from_rgb(44, 44, 44);
    pub const BG_WIDGET_ACTIVE: Color32 = Color32::from_rgb(56, 56, 56);
    pub const BG_SELECTION: Color32 = Color32::from_rgb(40, 55, 80);

    pub const STROKE_DIM: Color32 = Color32::from_rgb(40, 40, 40);
    pub const STROKE_MED: Color32 = Color32::from_rgb(56, 56, 56);
    pub const STROKE_BRIGHT: Color32 = Color32::from_rgb(72, 72, 72);

    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(210, 210, 210);
    pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(140, 140, 140);
    pub const TEXT_BRIGHT: Color32 = Color32::WHITE;
    pub const TEXT_ACCENT: Color32 = Color32::from_rgb(110, 160, 230);

    pub const ACCENT: Color32 = Color32::from_rgb(55, 100, 160);

    pub const LOG_INFO: Color32 = Color32::from_rgb(120, 180, 255);
    pub const LOG_WARN: Color32 = Color32::from_rgb(255, 196, 93);
    pub const LOG_ERROR: Color32 = Color32::from_rgb(255, 110, 110);

    pub const CORNER_RADIUS: u8 = 6;
}

/// 共享浮动面板 frame，支持透明度
pub fn floating_frame(opacity: f32) -> egui::Frame {
    let bg = palette::BG_PANEL;
    let bg_alpha = egui::Color32::from_rgba_premultiplied(
        bg.r(),
        bg.g(),
        bg.b(),
        (bg.a() as f32 * opacity) as u8,
    );
    let s = palette::STROKE_DIM;
    let stroke_alpha =
        egui::Color32::from_rgba_premultiplied(s.r(), s.g(), s.b(), (s.a() as f32 * opacity) as u8);
    egui::Frame::default()
        .fill(bg_alpha)
        .corner_radius(egui::CornerRadius::same(8))
        .stroke(egui::Stroke::new(1.0, stroke_alpha))
        .inner_margin(egui::Margin::same(10))
        .shadow(egui::epaint::Shadow {
            offset: [0, 0],
            blur: 16,
            spread: 4,
            color: egui::Color32::from_black_alpha(48),
        })
}

pub fn panel_bar_frame(horizontal: i8, vertical: i8) -> egui::Frame {
    egui::Frame::default()
        .fill(palette::BG_PANEL)
        .inner_margin(egui::Margin::symmetric(horizontal, vertical))
        .stroke(egui::Stroke::new(1.0, palette::STROKE_DIM))
}

/// 共享关闭按钮，使用 U+00D7 乘号作为关闭图标
pub fn close_button(ui: &mut egui::Ui, tooltip: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new("\u{00D7}")
                .color(palette::TEXT_SECONDARY)
                .size(14.0),
        )
        .fill(egui::Color32::TRANSPARENT)
        .corner_radius(egui::CornerRadius::same(3)),
    )
    .on_hover_text(tooltip)
}

pub fn apply(ctx: &egui::Context) {
    use palette::*;

    let mut visuals = Visuals::dark();

    visuals.panel_fill = BG_PANEL;
    visuals.window_fill = BG_WINDOW;
    visuals.extreme_bg_color = Color32::from_rgb(10, 10, 10);
    visuals.faint_bg_color = Color32::from_rgb(22, 22, 22);

    // 控件
    let widget_states = [
        (
            &mut visuals.widgets.noninteractive,
            BG_WIDGET,
            STROKE_DIM,
            TEXT_SECONDARY,
        ),
        (
            &mut visuals.widgets.inactive,
            BG_WIDGET,
            STROKE_MED,
            TEXT_PRIMARY,
        ),
        (
            &mut visuals.widgets.hovered,
            BG_WIDGET_HOVER,
            STROKE_BRIGHT,
            TEXT_BRIGHT,
        ),
        (
            &mut visuals.widgets.active,
            BG_WIDGET_ACTIVE,
            STROKE_BRIGHT,
            TEXT_BRIGHT,
        ),
        (
            &mut visuals.widgets.open,
            Color32::from_rgb(36, 36, 36),
            STROKE_MED,
            TEXT_BRIGHT,
        ),
    ];
    for (w, bg, stroke, fg) in widget_states {
        w.bg_fill = bg;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.fg_stroke = Stroke::new(1.0, fg);
        w.corner_radius = CornerRadius::same(CORNER_RADIUS);
        w.expansion = 0.0;
    }

    visuals.selection.bg_fill = BG_SELECTION;
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);

    let radius = CornerRadius::same(CORNER_RADIUS);
    visuals.window_corner_radius = radius;
    visuals.menu_corner_radius = radius;

    // 无阴影
    visuals.window_shadow = egui::epaint::Shadow::NONE;
    visuals.popup_shadow = egui::epaint::Shadow::NONE;
    visuals.window_stroke = Stroke::NONE;

    visuals.hyperlink_color = TEXT_ACCENT;
    visuals.warn_fg_color = LOG_WARN;
    visuals.error_fg_color = LOG_ERROR;

    let mut style = (*ctx.style()).clone();
    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.window_margin = Margin::same(10);
    style.spacing.menu_margin = Margin::same(6);
    style.spacing.button_padding = egui::vec2(10.0, 3.0);
    style.spacing.slider_rail_height = 3.0;
    style.spacing.combo_width = 120.0;
    ctx.set_style(style);
}
