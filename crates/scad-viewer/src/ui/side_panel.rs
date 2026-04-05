use crate::app::{UiActions, UiCommand, ViewerState};
use scad_data::{AppConfig, DocumentState, ExportFormat, SlicerInstall};
use scad_ui::theme::{self, palette};
use scad_ui::widgets::section_header;

const PANEL_WIDTH: f32 = 280.0;

pub struct SidePanelFrame<'a> {
    pub document: &'a mut DocumentState,
    pub slicers: &'a [SlicerInstall],
    pub config: &'a mut AppConfig,
}

pub fn show(
    ctx: &egui::Context,
    viewer_state: &mut ViewerState,
    has_current_file: bool,
    is_rendering: bool,
    actions: &mut UiActions,
    frame: SidePanelFrame<'_>,
) {
    let SidePanelFrame {
        document,
        slicers,
        config,
    } = frame;
    if !viewer_state.side_panel_open {
        return;
    }

    let opacity = config.floating_panel_opacity.clamp(0.1, 1.0);

    let screen = ctx.content_rect();
    let default_pos = egui::pos2(screen.max.x - PANEL_WIDTH - 12.0, screen.min.y + 52.0);
    let pos = config
        .param_panel_pos
        .map(|p| egui::pos2(p[0], p[1]))
        .unwrap_or(default_pos);

    let default_size = config
        .param_panel_size
        .map(|s| egui::vec2(s[0], s[1]))
        .unwrap_or(egui::vec2(PANEL_WIDTH, 400.0));

    let response = egui::Window::new("param_panel")
        .title_bar(false)
        .collapsible(false)
        .resizable(true)
        .movable(true)
        .constrain(true)
        .default_size(default_size)
        .default_pos(pos)
        .frame(theme::floating_frame(opacity))
        .show(ctx, |ui| {
            ui.set_min_width(PANEL_WIDTH);

            // 标题栏
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("参数")
                        .color(palette::TEXT_PRIMARY)
                        .strong()
                        .size(13.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::close_button(ui, "关闭面板").clicked() {
                        viewer_state.side_panel_open = false;
                    }
                });
            });
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(4.0);

            egui::ScrollArea::vertical().show(ui, |ui| {
                if !has_current_file {
                    ui.label(
                        egui::RichText::new("请先加载模型")
                            .color(palette::TEXT_SECONDARY)
                            .italics()
                            .size(12.0),
                    );
                    return;
                }

                if is_rendering {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            egui::RichText::new("正在渲染...")
                                .color(palette::TEXT_ACCENT)
                                .size(12.0),
                        );
                    });
                    ui.add_space(6.0);
                }

                section_header(ui, "参数编辑器");
                ui.add_space(2.0);
                crate::ui::param_editor::show(ui, document);

                ui.add_space(8.0);
                section_header(ui, "预设");
                ui.add_space(2.0);
                preset_section(ui, document, actions);

                ui.add_space(8.0);
                section_header(ui, "导出");
                ui.add_space(2.0);
                export_section(ui, document, slicers, actions);
            });
        });

    // 持久化拖动后的位置和尺寸
    if let Some(inner) = response {
        if inner.response.dragged() || inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.param_panel_pos = Some([rect.min.x, rect.min.y]);
        }
        if inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.param_panel_size = Some([rect.width(), rect.height()]);
            actions.commands.push(UiCommand::SaveSettings);
        }
    }
}

fn preset_section(ui: &mut egui::Ui, document: &mut DocumentState, actions: &mut UiActions) {
    if document.preset_names().is_empty() {
        ui.label(
            egui::RichText::new("当前没有可用预设。")
                .color(palette::TEXT_SECONDARY)
                .italics()
                .size(12.0),
        );
    } else {
        for preset in document.preset_names() {
            let selected = document.selected_preset.as_deref() == Some(preset.as_str());
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(&preset)
                            .color(if selected {
                                palette::TEXT_ACCENT
                            } else {
                                palette::TEXT_PRIMARY
                            })
                            .size(12.0),
                    )
                    .fill(if selected {
                        palette::BG_SELECTION
                    } else {
                        egui::Color32::TRANSPARENT
                    })
                    .corner_radius(egui::CornerRadius::same(4)),
                )
                .clicked()
            {
                document.selected_preset = Some(preset.clone());
                let _ = document.apply_preset(&preset);
            }
        }
    }

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("保存当前参数为预设")
            .color(palette::TEXT_SECONDARY)
            .size(11.0),
    );
    ui.add_space(2.0);
    ui.text_edit_singleline(&mut document.preset_name_input);

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("保存")
                        .color(palette::TEXT_PRIMARY)
                        .size(12.0),
                )
                .fill(palette::BG_WIDGET)
                .corner_radius(egui::CornerRadius::same(4)),
            )
            .clicked()
            && !document.preset_name_input.trim().is_empty()
        {
            actions.commands.push(UiCommand::SavePreset(
                document.preset_name_input.trim().to_string(),
            ));
        }
        let can_delete = document.selected_preset.is_some();
        if ui
            .add_enabled(
                can_delete,
                egui::Button::new(
                    egui::RichText::new("删除")
                        .color(if can_delete {
                            palette::LOG_ERROR
                        } else {
                            palette::TEXT_SECONDARY
                        })
                        .size(12.0),
                )
                .fill(egui::Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(4)),
            )
            .clicked()
        {
            actions.commands.push(UiCommand::DeletePreset(
                document.selected_preset.clone().unwrap_or_default(),
            ));
        }
    });
}

fn export_section(
    ui: &mut egui::Ui,
    document: &mut DocumentState,
    slicers: &[SlicerInstall],
    actions: &mut UiActions,
) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("格式")
                .color(palette::TEXT_SECONDARY)
                .size(12.0),
        );
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("STL")
                        .color(if document.export_format == ExportFormat::Stl {
                            palette::TEXT_BRIGHT
                        } else {
                            palette::TEXT_SECONDARY
                        })
                        .size(12.0),
                )
                .fill(if document.export_format == ExportFormat::Stl {
                    palette::BG_WIDGET_ACTIVE
                } else {
                    egui::Color32::TRANSPARENT
                })
                .corner_radius(egui::CornerRadius::same(4)),
            )
            .clicked()
        {
            document.export_format = ExportFormat::Stl;
        }
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("3MF")
                        .color(if document.export_format == ExportFormat::ThreeMf {
                            palette::TEXT_BRIGHT
                        } else {
                            palette::TEXT_SECONDARY
                        })
                        .size(12.0),
                )
                .fill(if document.export_format == ExportFormat::ThreeMf {
                    palette::BG_WIDGET_ACTIVE
                } else {
                    egui::Color32::TRANSPARENT
                })
                .corner_radius(egui::CornerRadius::same(4)),
            )
            .clicked()
        {
            document.export_format = ExportFormat::ThreeMf;
        }
    });

    ui.add_space(4.0);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("导出模型")
                    .color(palette::TEXT_PRIMARY)
                    .size(12.0),
            )
            .fill(palette::ACCENT)
            .corner_radius(egui::CornerRadius::same(4)),
        )
        .clicked()
    {
        actions.commands.push(UiCommand::ExportModel);
    }

    if slicers.is_empty() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new("未检测到切片软件，可在设置中手动填写路径。")
                .color(palette::TEXT_SECONDARY)
                .italics()
                .size(11.0),
        );
    }
    for slicer in slicers {
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(format!("发送到 {}", slicer.name))
                        .color(palette::TEXT_PRIMARY)
                        .size(12.0),
                )
                .fill(palette::BG_WIDGET)
                .corner_radius(egui::CornerRadius::same(4)),
            )
            .clicked()
        {
            actions
                .commands
                .push(UiCommand::SendToSlicer(slicer.name.clone()));
        }
    }
}
