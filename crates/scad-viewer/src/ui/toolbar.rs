use crate::app::{ColorMode, ProjectionMode, RenderMode, StudioApp, UiActions, ViewerState};
use crate::wrap_line_pack;
use scad_ui::theme;
use scad_ui::widgets::{selectable_button, toggle_button, toolbar_label};

/// 功能块估算宽度（与 `embedded_height`、分行算法共用）。略小于真实上限以在常见宽度下多放几块到同一行；若出现行末裁切再小幅上调对应块。
const W_FILE_BLOCK: f32 = 100.0;
const W_RENDER_BLOCK: f32 = 200.0;
const W_COLOR_BLOCK: f32 = 128.0;
const W_PROJECTION_BLOCK: f32 = 118.0;
const W_TOGGLE_BLOCK: f32 = 330.0;
const W_PANEL_BLOCK: f32 = 198.0;
/// 与 `Separator::default().spacing(...).vertical()` 一致，供换行估算与行宽求和共用。
const ROW_BLOCK_GAP: f32 = 12.0;
const ROW_VERTICAL_GAP: f32 = 1.0;
/// 每一行水平布局的固定高度。垂直 Separator 在 horizontal 内高度取该行可用高度；若不限制行高，会吃满整条预览工具条剩余高度并把第一行撑裂。
const TOOLBAR_ROW_HEIGHT: f32 = 26.0;

pub fn show(
    ctx: &egui::Context,
    studio: &mut StudioApp,
    actions: &mut UiActions,
    settings_open: &mut bool,
) {
    show_toolbar(ctx, studio, actions, Some(settings_open));
}

pub fn show_embedded(ctx: &egui::Context, studio: &mut StudioApp, actions: &mut UiActions) {
    show_toolbar(ctx, studio, actions, None);
}

fn show_toolbar(
    ctx: &egui::Context,
    studio: &mut StudioApp,
    actions: &mut UiActions,
    settings_open: Option<&mut bool>,
) {
    egui::TopBottomPanel::top("toolbar")
        .frame(theme::panel_bar_frame(8, 4))
        .show(ctx, |ui| {
            match settings_open {
                Some(so) => paint_toolbar_row(ui, studio, actions, true, so),
                None => {
                    let mut sink = false;
                    paint_toolbar_row(ui, studio, actions, false, &mut sink);
                }
            }
        });
}

/// 在已有 `Ui` 内绘制工具栏（用于 SCAD Studio 标签页内嵌；独立 viewer 仍使用 [`show`] / [`show_embedded`]）。
/// `draw_file_strip` 为 true 时绘制「打开 / 设置」块，并使用 `settings_open`；内嵌预览传 `false` 与占位 `&mut bool` 即可。
pub fn paint_toolbar_row(
    ui: &mut egui::Ui,
    studio: &mut StudioApp,
    actions: &mut UiActions,
    draw_file_strip: bool,
    settings_open: &mut bool,
) {
    let has_current_file = studio.has_current_file();
    let wrap_max_w = ui.max_rect().width().max(1.0);
    ui.set_max_width(wrap_max_w);

    let include_file = draw_file_strip;
    let widths = toolbar_block_widths_vec(include_file);
    let line_ix = wrap_line_pack::line_ranges(&widths, wrap_max_w, ROW_BLOCK_GAP);

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for (row_i, line) in line_ix.into_iter().enumerate() {
            if row_i > 0 {
                ui.add_space(ROW_VERTICAL_GAP);
            }
            ui.push_id(row_i, |ui| {
                ui.set_max_height(TOOLBAR_ROW_HEIGHT);
                ui.with_layout(
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        for (k, block_idx) in (line.start..line.end).enumerate() {
                            if k > 0 {
                                ui.add(
                                    egui::Separator::default().spacing(ROW_BLOCK_GAP).vertical(),
                                );
                            }
                            if include_file && block_idx == 0 {
                                file_group(ui, actions, settings_open);
                            } else {
                                paint_toolbar_block(
                                    ui,
                                    block_idx,
                                    include_file,
                                    studio,
                                    has_current_file,
                                );
                            }
                        }
                    },
                );
            });
        }
    });
}

/// 内嵌条带高度：按功能块宽度与可用内宽做贪心换行，须与 [`paint_toolbar_row`] 使用同一套块划分。
/// `include_file_group` 与 [`paint_toolbar_row`] 的 `draw_file_strip` 一致。
#[allow(dead_code)]
pub fn embedded_height(available_width: f32, include_file_group: bool) -> f32 {
    let lines = toolbar_line_count(available_width, include_file_group);
    toolbar_strip_outer_height(lines)
}

fn toolbar_block_widths_vec(include_file_group: bool) -> Vec<f32> {
    let mut blocks = Vec::with_capacity(6);
    if include_file_group {
        blocks.push(W_FILE_BLOCK);
    }
    blocks.extend_from_slice(&[
        W_RENDER_BLOCK,
        W_COLOR_BLOCK,
        W_PROJECTION_BLOCK,
        W_TOGGLE_BLOCK,
        W_PANEL_BLOCK,
    ]);
    blocks
}

fn toolbar_line_count(available_width: f32, include_file_group: bool) -> usize {
    let blocks = toolbar_block_widths_vec(include_file_group);
    wrap_line_pack::line_count(&blocks, available_width, ROW_BLOCK_GAP)
}

fn toolbar_strip_outer_height(lines: usize) -> f32 {
    // 与 Studio 内嵌条带 `Margin::symmetric(8,1)` 上下边距之和一致。
    const STRIP_VERT_MARGIN: f32 = 2.0;
    let n = lines.max(1);
    STRIP_VERT_MARGIN + n as f32 * TOOLBAR_ROW_HEIGHT + (n - 1) as f32 * ROW_VERTICAL_GAP
}

fn paint_toolbar_block(
    ui: &mut egui::Ui,
    block_index: usize,
    include_file: bool,
    studio: &mut StudioApp,
    has_current_file: bool,
) {
    let group_index = if include_file {
        block_index - 1
    } else {
        block_index
    };
    match group_index {
        0 => render_mode_group(ui, studio.viewer_state_mut()),
        1 => color_mode_group(ui, studio.viewer_state_mut()),
        2 => projection_group(ui, studio.viewer_state_mut()),
        3 => toggle_group(ui, studio.viewer_state_mut()),
        4 => panel_group(ui, studio.viewer_state_mut(), has_current_file),
        _ => {}
    }
}

fn file_group(ui: &mut egui::Ui, actions: &mut UiActions, settings_open: &mut bool) {
    if ui
        .add(
            egui::Button::new(egui::RichText::new("\u{1f4c2} 打开").size(13.0))
                .corner_radius(egui::CornerRadius::same(4)),
        )
        .clicked()
    {
        actions.open_file = true;
    }
    if ui
        .add(
            egui::Button::new(egui::RichText::new("\u{2699}").size(14.0))
                .corner_radius(egui::CornerRadius::same(4)),
        )
        .on_hover_text("设置")
        .clicked()
    {
        *settings_open = true;
    }
}

fn render_mode_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    toolbar_label(ui, "渲染");
    if selectable_button(ui, vs.render_mode == RenderMode::Solid, "Solid").clicked() {
        vs.render_mode = RenderMode::Solid;
    }
    let wire_btn = selectable_button(ui, vs.render_mode == RenderMode::Wireframe, "Wire");
    if !vs.wireframe_supported {
        // disabled state — just show dimmed
    } else if wire_btn.clicked() {
        vs.render_mode = RenderMode::Wireframe;
    }
    if selectable_button(ui, vs.render_mode == RenderMode::XRay, "X-Ray").clicked() {
        vs.render_mode = RenderMode::XRay;
    }
}

fn color_mode_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    toolbar_label(ui, "颜色");
    if selectable_button(ui, vs.color_mode == ColorMode::Mono, "Mono").clicked() {
        vs.color_mode = ColorMode::Mono;
    }
    if selectable_button(ui, vs.color_mode == ColorMode::Color, "Color").clicked() {
        vs.color_mode = ColorMode::Color;
    }
}

fn projection_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    toolbar_label(ui, "投影");
    if selectable_button(
        ui,
        vs.projection_mode == ProjectionMode::Perspective,
        "透视",
    )
    .clicked()
    {
        vs.projection_mode = ProjectionMode::Perspective;
    }
    if selectable_button(
        ui,
        vs.projection_mode == ProjectionMode::Orthographic,
        "正交",
    )
    .clicked()
    {
        vs.projection_mode = ProjectionMode::Orthographic;
    }
}

fn toggle_group(ui: &mut egui::Ui, vs: &mut ViewerState) {
    if toggle_button(ui, vs.show_grid, "网格").clicked() {
        vs.show_grid = !vs.show_grid;
    }
    if toggle_button(ui, vs.show_build_plate, "底板").clicked() {
        vs.show_build_plate = !vs.show_build_plate;
    }
    if toggle_button(ui, vs.show_axis_gizmo, "坐标轴").clicked() {
        vs.show_axis_gizmo = !vs.show_axis_gizmo;
    }
    if toggle_button(ui, vs.shadows_enabled, "阴影").clicked() {
        vs.shadows_enabled = !vs.shadows_enabled;
    }
    if toggle_button(ui, vs.fog_enabled, "雾效").clicked() {
        vs.fog_enabled = !vs.fog_enabled;
    }
    if toggle_button(ui, vs.clip_plane_enabled, "剖切").clicked() {
        vs.clip_plane_enabled = !vs.clip_plane_enabled;
    }
}

fn panel_group(ui: &mut egui::Ui, vs: &mut ViewerState, _has_current_file: bool) {
    if toggle_button(ui, vs.side_panel_open, "参数面板")
        .on_hover_text(if vs.side_panel_open {
            "隐藏参数面板"
        } else {
            "显示参数面板"
        })
        .clicked()
    {
        vs.toggle_side_panel();
    }
    if toggle_button(ui, vs.camera_overlay_open, "相机面板").clicked() {
        vs.camera_overlay_open = !vs.camera_overlay_open;
    }
    if toggle_button(ui, vs.log_panel_open, "日志").clicked() {
        vs.toggle_log_panel();
    }
}
