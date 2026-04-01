use std::path::PathBuf;

use crate::config::{AppConfig, SlicerConfig};

const KNOWN_SLICERS: [&str; 3] = ["PrusaSlicer", "Bambu Studio", "Cura"];

pub fn show(ctx: &egui::Context, open: &mut bool, config: &mut AppConfig) -> bool {
    let mut save_requested = false;
    egui::Window::new("设置")
        .open(open)
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("OpenSCAD");
            edit_optional_path(ui, "OpenSCAD 路径", &mut config.openscad_path);
            ui.separator();
            ui.heading("切片软件");
            for name in KNOWN_SLICERS {
                edit_slicer_path(ui, config, name);
            }
            ui.separator();
            if ui.button("保存配置").clicked() {
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
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.text_edit_singleline(&mut text).changed() {
            *path = (!text.trim().is_empty()).then(|| PathBuf::from(text.trim()));
        }
    });
}

fn edit_slicer_path(ui: &mut egui::Ui, config: &mut AppConfig, name: &str) {
    let mut text = config
        .slicers
        .iter()
        .find(|slicer| slicer.name == name)
        .map(|slicer| slicer.path.display().to_string())
        .unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(name);
        if ui.text_edit_singleline(&mut text).changed() {
            update_slicer_path(config, name, text.trim());
        }
    });
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
