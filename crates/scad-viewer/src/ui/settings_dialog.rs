use std::path::PathBuf;

use scad_data::{AppConfig, SlicerConfig};
use scad_ui::theme::palette;
use scad_ui::widgets::section_label;

const KNOWN_SLICERS: [&str; 3] = ["PrusaSlicer", "Bambu Studio", "Cura"];

pub fn show(ctx: &egui::Context, open: &mut bool, config: &mut AppConfig) -> bool {
    let mut save_requested = false;
    egui::Window::new("设置")
        .open(open)
        .resizable(true)
        .default_width(400.0)
        .frame(
            egui::Frame::default()
                .fill(palette::BG_WINDOW)
                .inner_margin(egui::Margin::same(16))
                .corner_radius(egui::CornerRadius::same(8))
                .stroke(egui::Stroke::new(1.0, palette::STROKE_MED)),
        )
        .show(ctx, |ui| {
            section_label(ui, "OPENSCAD");
            edit_optional_path(ui, "OpenSCAD 路径", &mut config.openscad_path);

            ui.add_space(12.0);
            section_label(ui, "切片软件");
            for name in KNOWN_SLICERS {
                edit_slicer_path(ui, config, name);
            }

            ui.add_space(12.0);
            section_label(ui, "界面");
            ui.label(
                egui::RichText::new("浮动面板透明度")
                    .color(palette::TEXT_PRIMARY)
                    .size(12.0),
            );
            ui.add_space(2.0);
            let mut opacity = config.floating_panel_opacity;
            if ui
                .add(
                    egui::Slider::new(&mut opacity, 0.1..=1.0)
                        .show_value(true)
                        .fixed_decimals(2),
                )
                .changed()
            {
                config.floating_panel_opacity = opacity;
            }

            ui.add_space(16.0);
            ui.separator();
            ui.add_space(8.0);

            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("保存配置")
                            .color(palette::TEXT_PRIMARY)
                            .size(13.0),
                    )
                    .fill(palette::ACCENT)
                    .corner_radius(egui::CornerRadius::same(4))
                    .min_size(egui::vec2(100.0, 0.0)),
                )
                .clicked()
            {
                save_requested = true;
            }
        });
    if save_requested {
        *open = false;
    }
    save_requested
}

fn edit_optional_path(ui: &mut egui::Ui, label: &str, path: &mut Option<PathBuf>) {
    let mut text = path
        .as_ref()
        .map(|value| value.display().to_string())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new(label)
            .color(palette::TEXT_PRIMARY)
            .size(12.0),
    );
    ui.add_space(2.0);
    if ui
        .add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 4)),
        )
        .changed()
    {
        *path = (!text.trim().is_empty()).then(|| PathBuf::from(text.trim()));
    }
    ui.add_space(4.0);
}

fn edit_slicer_path(ui: &mut egui::Ui, config: &mut AppConfig, name: &str) {
    let mut text = config
        .slicers
        .iter()
        .find(|slicer| slicer.name == name)
        .map(|slicer| slicer.path.display().to_string())
        .unwrap_or_default();
    ui.label(
        egui::RichText::new(name)
            .color(palette::TEXT_PRIMARY)
            .size(12.0),
    );
    ui.add_space(2.0);
    if ui
        .add(
            egui::TextEdit::singleline(&mut text)
                .desired_width(f32::INFINITY)
                .margin(egui::Margin::symmetric(8, 4)),
        )
        .changed()
    {
        update_slicer_path(config, name, text.trim());
    }
    ui.add_space(4.0);
}

fn update_slicer_path(config: &mut AppConfig, name: &str, path: &str) {
    config.slicers.retain(|slicer| slicer.name != name);
    if path.is_empty() {
        return;
    }
    config.slicers.push(SlicerConfig {
        name: name.to_string(),
        path: PathBuf::from(path),
    });
}
