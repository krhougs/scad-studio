use crate::app::{CameraAction, UiActions};
use scad_data::AppConfig;
use scad_scene::OrbitalCamera;
use scad_ui::theme::{self, palette};
use scad_ui::widgets::{filled_small_button, icon_button};

pub fn show(
    ctx: &egui::Context,
    camera: &OrbitalCamera,
    actions: &mut UiActions,
    config: &mut AppConfig,
    overlay_open: bool,
) {
    if !overlay_open {
        return;
    }

    let eye = camera.eye();
    let target = camera.target();
    let dist = camera.distance();
    let az = camera.azimuth_degrees();
    let el = camera.elevation_degrees();

    let opacity = config.floating_panel_opacity.clamp(0.1, 1.0);

    // 计算默认位置：右上角
    let screen = ctx.content_rect();
    let default_pos = egui::pos2(screen.max.x - 220.0, screen.min.y + 52.0);

    let pos = config
        .camera_overlay_pos
        .map(|p| egui::pos2(p[0], p[1]))
        .unwrap_or(default_pos);

    let default_size = config
        .camera_overlay_size
        .map(|s| egui::vec2(s[0], s[1]))
        .unwrap_or(egui::vec2(220.0, 300.0));

    let response = egui::Window::new("camera_overlay")
        .title_bar(false)
        .collapsible(false)
        .resizable(true)
        .movable(true)
        .constrain(true)
        .default_size(default_size)
        .default_pos(pos)
        .frame(theme::floating_frame(opacity))
        .show(ctx, |ui| {
            // 标题行
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("相机")
                        .color(palette::TEXT_PRIMARY)
                        .strong()
                        .size(13.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, "R", "重置视角").clicked() {
                        actions.camera_action = Some(CameraAction::ResetView);
                    }
                });
            });

            ui.add_space(4.0);

            // 眼点位置（只读）
            label_row(
                ui,
                "眼点",
                format!("{:.2}  {:.2}  {:.2}", eye.x, eye.y, eye.z),
            );

            ui.add_space(6.0);

            // 目标 XYZ
            drag_row(ui, "目标 X", target.x, |v| {
                actions.camera_action = Some(CameraAction::SetTargetX(v))
            });
            drag_row(ui, "目标 Y", target.y, |v| {
                actions.camera_action = Some(CameraAction::SetTargetY(v))
            });
            drag_row(ui, "目标 Z", target.z, |v| {
                actions.camera_action = Some(CameraAction::SetTargetZ(v))
            });

            ui.add_space(4.0);

            // 距离
            drag_row(ui, "距离", dist, |v| {
                actions.camera_action = Some(CameraAction::SetDistance(v))
            });

            ui.add_space(4.0);

            // 方位角 / 仰角
            drag_row(ui, "方位角", az, |v| {
                actions.camera_action = Some(CameraAction::SetAzimuth(v))
            });
            drag_row(ui, "仰角", el, |v| {
                actions.camera_action = Some(CameraAction::SetElevation(v))
            });

            ui.add_space(8.0);

            // 预设视角按钮
            ui.label(
                egui::RichText::new("预设视角")
                    .color(palette::TEXT_SECONDARY)
                    .size(10.0),
            );
            ui.add_space(2.0);
            view_buttons(ui, actions);
        });

    // 持久化拖动后的位置和尺寸
    if let Some(inner) = response {
        if inner.response.dragged() || inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.camera_overlay_pos = Some([rect.min.x, rect.min.y]);
        }
        if inner.response.drag_stopped() {
            let rect = inner.response.rect;
            config.camera_overlay_size = Some([rect.width(), rect.height()]);
            actions.commands.push(crate::app::UiCommand::SaveSettings);
        }
    }
}

fn view_buttons(ui: &mut egui::Ui, actions: &mut UiActions) {
    let views: &[(&str, CameraAction)] = &[
        ("前", CameraAction::ViewFront),
        ("后", CameraAction::ViewBack),
        ("左", CameraAction::ViewLeft),
        ("右", CameraAction::ViewRight),
        ("顶", CameraAction::ViewTop),
        ("底", CameraAction::ViewBottom),
    ];
    ui.horizontal_wrapped(|ui| {
        for (label, action) in views {
            if filled_small_button(ui, label).clicked() {
                actions.camera_action = Some(action.clone());
            }
        }
    });
}

fn drag_row(ui: &mut egui::Ui, label: &str, value: f32, on_change: impl FnOnce(f32)) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(palette::TEXT_SECONDARY)
                .size(11.0),
        );
        ui.add_space(2.0);
        let mut v = value;
        let resp = ui.add(
            egui::DragValue::new(&mut v)
                .speed(0.01)
                .range(f64::NEG_INFINITY..=f64::INFINITY)
                .max_decimals(3)
                .min_decimals(2),
        );
        if resp.changed() {
            on_change(v);
        }
    });
}

fn label_row(ui: &mut egui::Ui, label: &str, text: String) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .color(palette::TEXT_SECONDARY)
                .size(11.0),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(text)
                .color(palette::TEXT_PRIMARY)
                .monospace()
                .size(11.0),
        );
    });
}
