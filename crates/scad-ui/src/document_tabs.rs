use egui::{Align2, Color32, CornerRadius, RichText, Stroke, TextStyle, TextWrapMode, WidgetText};

use crate::{rail_style, theme::palette};

const RAIL_PADDING_X: i8 = 6;
const RAIL_PADDING_TOP: i8 = 4;
const RAIL_PADDING_BOTTOM: i8 = 4;
/// 标签条内区在标签高度上下的留白，避免圆角与 ScrollArea 裁剪把顶部切掉
const TAB_STRIP_INNER_VERT_GAP: f32 = 2.0;
const TAB_BAR_ITEM_SPACING: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentTabKind {
    Viewer,
    Markdown,
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentTabState {
    Normal,
    Dirty,
    Busy,
    Error,
}

#[derive(Debug, Clone, Copy)]
pub struct DocumentTabItem<'a> {
    pub title: &'a str,
    pub kind: DocumentTabKind,
    pub active: bool,
    pub state: DocumentTabState,
    pub closable: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DocumentTabsResponse {
    pub activate: Option<usize>,
    pub close: Option<usize>,
}

pub fn rail_inner_content_height() -> f32 {
    tab_height() + 2.0 * TAB_STRIP_INNER_VERT_GAP
}

pub fn rail_height() -> f32 {
    f32::from(RAIL_PADDING_TOP) + rail_inner_content_height() + f32::from(RAIL_PADDING_BOTTOM)
}

/// 首行标签条（`rail_height()` 高）内，药丸标签在 **flipped、顶边为 y=0** 的坐标系里相对条带顶边的垂直中心 Y。
/// 与 `allocate_filled_strip_ui(..., rail_margin(), ...)` 内 `Align::Center` 的药丸对齐。
pub fn tab_rail_pills_center_y_from_strip_top() -> f32 {
    f32::from(RAIL_PADDING_TOP) + rail_inner_content_height() * 0.5
}

pub fn rail_vertical_padding() -> i8 {
    RAIL_PADDING_TOP
}

pub fn rail_bottom_padding() -> i8 {
    RAIL_PADDING_BOTTOM
}

pub fn tab_height() -> f32 {
    rail_style::metrics().item_height
}

pub fn active_tab_extension_height() -> i8 {
    0
}

pub fn rail_show_separator_line() -> bool {
    false
}

pub fn rail_fill_color() -> egui::Color32 {
    crate::theme::palette::BG_PANEL
}

pub fn rail_margin() -> egui::Margin {
    egui::Margin {
        left: RAIL_PADDING_X,
        right: RAIL_PADDING_X,
        top: RAIL_PADDING_TOP,
        bottom: RAIL_PADDING_BOTTOM,
    }
}

pub fn rail_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(rail_fill_color())
        .inner_margin(rail_margin())
        .stroke(Stroke::NONE)
}

pub fn show_document_tabs(
    ui: &mut egui::Ui,
    items: &[DocumentTabItem<'_>],
) -> DocumentTabsResponse {
    let mut response = DocumentTabsResponse::default();

    egui::ScrollArea::horizontal()
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let rail_bottom = ui.max_rect().bottom();
            let mut active_rect = None;
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = TAB_BAR_ITEM_SPACING;
                for (index, item) in items.iter().enumerate() {
                    let tab_response = show_single_tab(ui, item);
                    if item.active {
                        active_rect = Some(tab_response.rect);
                    }
                    if tab_response.activate {
                        response.activate = Some(index);
                    }
                    if tab_response.close {
                        response.close = Some(index);
                    }
                }
            });
            if let Some(rect) = active_rect {
                paint_active_tab_extension(ui, rect, rail_bottom);
            }
        });

    response
}

#[derive(Debug, Clone, Copy)]
struct SingleTabResponse {
    activate: bool,
    close: bool,
    rect: egui::Rect,
}

impl Default for SingleTabResponse {
    fn default() -> Self {
        Self {
            activate: false,
            close: false,
            rect: egui::Rect::NOTHING,
        }
    }
}

fn measure_title_width(ui: &egui::Ui, title: &str) -> f32 {
    let galley = WidgetText::from(RichText::new(title).size(12.0).color(palette::TEXT_PRIMARY))
        .into_galley(
            ui,
            Some(TextWrapMode::Truncate),
            f32::INFINITY,
            TextStyle::Button,
        );
    galley.size().x
}

fn tab_width_for_item(ui: &egui::Ui, item: &DocumentTabItem<'_>) -> f32 {
    let m = rail_style::metrics();
    let title_w = measure_title_width(ui, item.title);
    let trail = m.status_dot_slot_width
        + if item.closable {
            m.content_gap + m.close_button_slot_width
        } else {
            0.0
        };
    let body = m.kind_chip_width + m.content_gap + title_w + m.content_gap + trail;
    let pad = 2.0 * f32::from(m.item_padding_x);
    (body + pad).clamp(m.tab_min_width, m.tab_max_width)
}

fn show_single_tab(ui: &mut egui::Ui, item: &DocumentTabItem<'_>) -> SingleTabResponse {
    let metrics = rail_style::metrics();
    let content_height = rail_style::content_height();
    let tab_width = tab_width_for_item(ui, item);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(tab_width, metrics.item_height),
        egui::Sense::click(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let state =
        rail_style::resolve_item_state(item.active, response.hovered(), response.has_focus());
    let visuals = rail_style::document_tab_visuals(state);
    rail_style::paint_rail_item_surface(
        ui,
        rect,
        visuals,
        rail_style::document_tab_corner_radius(),
    );

    let inner_rect = rect.shrink2(egui::vec2(
        f32::from(metrics.item_padding_x),
        f32::from(metrics.item_padding_y),
    ));
    let hover_pos = ui.ctx().input(|i| i.pointer.hover_pos());
    let close_rect = item.closable.then(|| {
        egui::Rect::from_min_size(
            egui::pos2(
                inner_rect.right() - metrics.close_button_slot_width,
                inner_rect.center().y - content_height * 0.5,
            ),
            egui::vec2(metrics.close_button_slot_width, content_height),
        )
    });
    let status_rect = egui::Rect::from_min_size(
        egui::pos2(
            inner_rect.right()
                - metrics.status_dot_slot_width
                - if item.closable {
                    metrics.close_button_slot_width + metrics.content_gap
                } else {
                    0.0
                },
            inner_rect.center().y - content_height * 0.5,
        ),
        egui::vec2(metrics.status_dot_slot_width, content_height),
    );
    let chip_rect = paint_kind_chip(ui, inner_rect, item.kind, visuals);
    let title_right = (status_rect.left() - metrics.content_gap).max(chip_rect.right());
    let title_rect = egui::Rect::from_min_max(
        egui::pos2(chip_rect.right() + metrics.content_gap, inner_rect.top()),
        egui::pos2(title_right, inner_rect.bottom()),
    );

    paint_title(ui, &response, title_rect, item.title, visuals.text);
    paint_status_dot(ui, status_rect, item.state);

    let close_clicked = close_rect.is_some_and(|close_rect| {
        let hovered = hover_pos.is_some_and(|pos| close_rect.contains(pos));
        paint_close_button(ui, close_rect, item.active || response.hovered(), hovered);
        response
            .interact_pointer_pos()
            .is_some_and(|pos| response.clicked() && close_rect.contains(pos))
    });

    SingleTabResponse {
        activate: response.clicked() && !close_clicked,
        close: close_clicked,
        rect,
    }
}

/// 侧栏文件树等：与标签内部相同的 chip + 标题绘制；默认行高为 `content_height()`。
pub fn show_document_tab_inner_row(
    ui: &mut egui::Ui,
    title: &str,
    active: bool,
    kind: Option<DocumentTabKind>,
) -> egui::Response {
    show_document_tab_inner_row_sized(ui, title, active, kind, rail_style::content_height())
}

/// 与 [`show_document_tab_inner_row`] 相同，可指定行高（文件树与展开箭头列对齐时使用）。
pub fn show_document_tab_inner_row_sized(
    ui: &mut egui::Ui,
    title: &str,
    active: bool,
    kind: Option<DocumentTabKind>,
    row_height: f32,
) -> egui::Response {
    const COMPACT_PAD_X: f32 = 3.0;
    const COMPACT_PAD_Y: f32 = 0.0;
    const COMPACT_CHIP_TITLE_GAP: f32 = 4.0;

    let width = ui.available_width();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(width, row_height), egui::Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    let state = rail_style::resolve_item_state(active, response.hovered(), response.has_focus());
    let row_visuals = rail_style::document_tab_visuals(state);
    let chip_visuals = file_tree_kind_chip_visuals(state);

    let inner_rect = rect.shrink2(egui::vec2(COMPACT_PAD_X, COMPACT_PAD_Y));
    paint_document_tab_inner_content(
        ui,
        &response,
        inner_rect,
        title,
        kind,
        row_visuals,
        chip_visuals,
        COMPACT_CHIP_TITLE_GAP,
    );
    response
}

fn file_tree_kind_chip_visuals(state: rail_style::RailItemState) -> rail_style::RailItemVisuals {
    let mut v = rail_style::document_tab_visuals(state);
    match state {
        rail_style::RailItemState::Idle => {
            v.chip_fill = Color32::from_rgb(38, 38, 38);
            v.chip_text = palette::TEXT_PRIMARY;
        }
        rail_style::RailItemState::Hovered => {
            v.chip_fill = Color32::from_rgb(48, 48, 48);
            v.chip_text = palette::TEXT_BRIGHT;
        }
        rail_style::RailItemState::Active | rail_style::RailItemState::Focused => {
            v.chip_fill = Color32::from_rgb(56, 56, 58);
        }
    }
    v
}

fn paint_document_tab_inner_content(
    ui: &mut egui::Ui,
    response: &egui::Response,
    inner_rect: egui::Rect,
    title: &str,
    kind: Option<DocumentTabKind>,
    row_visuals: rail_style::RailItemVisuals,
    chip_visuals: rail_style::RailItemVisuals,
    chip_title_gap: f32,
) {
    let title_left = if let Some(k) = kind {
        let chip_rect = paint_kind_chip(ui, inner_rect, k, chip_visuals);
        chip_rect.right() + chip_title_gap
    } else {
        inner_rect.left()
    };
    let title_rect = egui::Rect::from_min_max(
        egui::pos2(title_left, inner_rect.top()),
        egui::pos2(inner_rect.right(), inner_rect.bottom()),
    );
    paint_title(ui, response, title_rect, title, row_visuals.text);
}

fn paint_active_tab_extension(ui: &egui::Ui, rect: egui::Rect, rail_bottom: f32) {
    if active_tab_extension_height() <= 0 {
        return;
    }
    let extension_top = (rect.bottom() - 1.0).min(rail_bottom);
    let extension_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), extension_top),
        egui::pos2(rect.right(), rail_bottom),
    );
    ui.painter().rect_filled(
        extension_rect,
        CornerRadius::ZERO,
        rail_style::item_visuals(rail_style::RailItemState::Active).fill,
    );
    let stroke = rail_style::item_visuals(rail_style::RailItemState::Active).stroke;
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), extension_top),
            egui::pos2(rect.left(), rail_bottom),
        ],
        stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(rect.right(), extension_top),
            egui::pos2(rect.right(), rail_bottom),
        ],
        stroke,
    );
}

fn paint_kind_chip(
    ui: &mut egui::Ui,
    inner_rect: egui::Rect,
    kind: DocumentTabKind,
    visuals: rail_style::RailItemVisuals,
) -> egui::Rect {
    let metrics = rail_style::metrics();
    let label = match kind {
        DocumentTabKind::Viewer => "3D",
        DocumentTabKind::Markdown => "MD",
        DocumentTabKind::Image => "IMG",
    };
    let chip_rect = egui::Rect::from_min_size(
        egui::pos2(
            inner_rect.left(),
            inner_rect.center().y - rail_style::content_height() * 0.5,
        ),
        egui::vec2(metrics.kind_chip_width, rail_style::content_height()),
    );
    ui.painter()
        .rect_filled(chip_rect, CornerRadius::same(6), visuals.chip_fill);
    let galley = WidgetText::from(
        RichText::new(label)
            .size(9.5)
            .strong()
            .color(visuals.chip_text),
    )
    .into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        chip_rect.width() - 8.0,
        TextStyle::Button,
    );
    let text_pos = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), chip_rect)
        .min
        - galley.rect.min.to_vec2();
    ui.painter().galley(text_pos, galley, visuals.chip_text);
    chip_rect
}

fn paint_title(
    ui: &mut egui::Ui,
    response: &egui::Response,
    title_rect: egui::Rect,
    title: &str,
    text_color: egui::Color32,
) {
    let galley = WidgetText::from(RichText::new(title).size(12.0).color(text_color)).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        title_rect.width(),
        TextStyle::Button,
    );
    let elided = galley.elided;
    let text_pos = Align2::LEFT_CENTER
        .align_size_within_rect(galley.size(), title_rect)
        .min
        - galley.rect.min.to_vec2();
    ui.painter().galley(text_pos, galley, text_color);
    if elided && response.hovered() {
        response.show_tooltip_text(title);
    }
}

fn paint_status_dot(ui: &mut egui::Ui, rect: egui::Rect, state: DocumentTabState) {
    let color = match state {
        DocumentTabState::Normal => return,
        DocumentTabState::Dirty => rail_style::accent_color(),
        DocumentTabState::Busy => palette::TEXT_PRIMARY,
        DocumentTabState::Error => palette::LOG_ERROR,
    };
    ui.painter().circle_filled(rect.center(), 2.6, color);
}

fn paint_close_button(ui: &mut egui::Ui, rect: egui::Rect, emphasized: bool, hovered: bool) {
    let icon_rect = egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(
            rail_style::metrics().close_button_size,
            rail_style::metrics().close_button_size,
        ),
    );
    if hovered {
        ui.painter()
            .rect_filled(icon_rect, CornerRadius::same(6), palette::BG_WIDGET);
    }
    let color = rail_style::close_button_color(emphasized || hovered);
    let galley = WidgetText::from(RichText::new("\u{00D7}").size(12.0).color(color)).into_galley(
        ui,
        Some(TextWrapMode::Truncate),
        icon_rect.width(),
        TextStyle::Button,
    );
    let text_pos = Align2::CENTER_CENTER
        .align_size_within_rect(galley.size(), icon_rect)
        .min
        - galley.rect.min.to_vec2();
    ui.painter().galley(text_pos, galley, color);
}
