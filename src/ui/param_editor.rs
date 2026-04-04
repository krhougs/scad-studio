use std::collections::BTreeMap;

use crate::{
    document::DocumentState,
    params::{ParameterEntry, ParameterKind, ParameterValue},
    ui::theme::palette,
};

pub fn show(ui: &mut egui::Ui, document: &mut DocumentState) {
    let entries = document
        .parameter_entries()
        .into_iter()
        .filter(|entry| !entry.definition.hidden)
        .collect::<Vec<_>>();
    if entries.is_empty() {
        ui.label(
            egui::RichText::new("未检测到可编辑的 Customizer 参数。")
                .color(palette::TEXT_SECONDARY)
                .italics()
                .size(12.0),
        );
        return;
    }
    for (group, items) in group_entries(&entries) {
        ui.add_space(4.0);
        ui.collapsing(
            egui::RichText::new(&group)
                .color(palette::TEXT_PRIMARY)
                .size(12.0),
            |ui| {
                for entry in items {
                    show_entry(ui, document, &entry);
                }
            },
        );
    }
}

fn group_entries(entries: &[ParameterEntry]) -> Vec<(String, Vec<ParameterEntry>)> {
    let mut groups = BTreeMap::<String, Vec<ParameterEntry>>::new();
    for entry in entries {
        let group = entry
            .definition
            .group
            .clone()
            .unwrap_or_else(|| "未分组".to_string());
        groups.entry(group).or_default().push(entry.clone());
    }
    groups.into_iter().collect()
}

fn show_entry(ui: &mut egui::Ui, document: &mut DocumentState, entry: &ParameterEntry) {
    let overridden = entry.value != entry.definition.default_value;
    ui.horizontal(|ui| {
        let name_text = if overridden {
            egui::RichText::new(&entry.definition.name)
                .color(palette::TEXT_ACCENT)
                .strong()
                .size(12.0)
        } else {
            egui::RichText::new(&entry.definition.name)
                .color(palette::TEXT_PRIMARY)
                .size(12.0)
        };
        ui.label(name_text);
        ui.add_space(4.0);
        render_control(ui, document, entry);
        if overridden {
            ui.add_space(2.0);
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new("恢复")
                            .color(palette::TEXT_SECONDARY)
                            .size(10.0),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(3)),
                )
                .clicked()
            {
                let _ = document.restore_parameter(&entry.definition.name);
            }
        }
    });
    ui.add_space(2.0);
}

fn render_control(ui: &mut egui::Ui, document: &mut DocumentState, entry: &ParameterEntry) {
    match (&entry.definition.kind, &entry.value) {
        (ParameterKind::Number { min, step, max }, ParameterValue::Number(value)) => {
            show_number_control(ui, document, entry, *value, *min, *step, *max);
        }
        (ParameterKind::Bool, ParameterValue::Bool(value)) => {
            let mut current = *value;
            if ui.checkbox(&mut current, "").changed() {
                let _ =
                    document.set_parameter(&entry.definition.name, ParameterValue::Bool(current));
            }
        }
        (ParameterKind::Choice { options }, ParameterValue::Text(value)) => {
            let mut current = value.clone();
            egui::ComboBox::from_id_salt((&entry.definition.name, "choice"))
                .selected_text(&current)
                .show_ui(ui, |ui| {
                    for option in options {
                        ui.selectable_value(&mut current, option.clone(), option);
                    }
                });
            if current != *value {
                let _ =
                    document.set_parameter(&entry.definition.name, ParameterValue::Text(current));
            }
        }
        _ => {}
    }
}

fn show_number_control(
    ui: &mut egui::Ui,
    document: &mut DocumentState,
    entry: &ParameterEntry,
    value: f64,
    min: Option<f64>,
    step: Option<f64>,
    max: Option<f64>,
) {
    let mut current = value;
    let changed = if let (Some(min), Some(max)) = (min, max) {
        ui.add(
            egui::Slider::new(&mut current, min..=max)
                .step_by(step.unwrap_or(1.0))
                .show_value(true),
        )
        .changed()
    } else {
        ui.add(egui::DragValue::new(&mut current).speed(step.unwrap_or(1.0)))
            .changed()
    };
    if changed {
        let _ = document.set_parameter(&entry.definition.name, ParameterValue::Number(current));
    }
}
