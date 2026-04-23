use crate::{
    ExportFormat, ParameterEntry, ParameterStore, ParameterValue, PresetFile, parse_parameters,
    preset_path_for_source,
};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const PARAM_RENDER_DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone)]
pub struct DocumentState {
    source_path: Option<PathBuf>,
    source_text: String,
    pub parameters: Option<ParameterStore>,
    pub presets: PresetFile,
    pub export_format: ExportFormat,
    pub preset_name_input: String,
    pub selected_preset: Option<String>,
    warnings: Vec<String>,
    pending_render_at: Option<Instant>,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            source_path: None,
            source_text: String::new(),
            parameters: None,
            presets: PresetFile::default(),
            export_format: ExportFormat::Stl,
            preset_name_input: String::new(),
            selected_preset: None,
            warnings: Vec::new(),
            pending_render_at: None,
        }
    }
}

impl DocumentState {
    pub fn load_source(&mut self, source_path: PathBuf, source_text: &str) {
        let parsed = parse_parameters(source_text);
        self.source_path = Some(source_path);
        self.source_text = source_text.to_string();
        self.parameters = Some(ParameterStore::from_parsed(parsed.clone()));
        self.export_format = ExportFormat::Stl;
        self.presets = PresetFile::default();
        self.preset_name_input.clear();
        self.selected_preset = None;
        self.warnings = parsed.warnings;
        self.pending_render_at = None;
    }

    pub fn reload_source(&mut self, source_text: &str) {
        let reparsed = parse_parameters(source_text);
        self.source_text = source_text.to_string();
        match self.parameters.as_mut() {
            Some(store) => store.merge_reparsed(reparsed.clone()),
            None => self.parameters = Some(ParameterStore::from_parsed(reparsed.clone())),
        }
        self.warnings = reparsed.warnings;
    }

    pub fn current_source(&self) -> Option<&Path> {
        self.source_path.as_deref()
    }

    pub fn watch_paths(&self) -> Vec<PathBuf> {
        let Some(source_path) = self.source_path.clone() else {
            return Vec::new();
        };
        vec![source_path.clone(), preset_path_for_source(&source_path)]
    }

    pub fn current_defines(&self) -> Vec<String> {
        self.parameters
            .as_ref()
            .map(ParameterStore::cli_defines)
            .unwrap_or_default()
    }

    pub fn parameter_entries(&self) -> Vec<ParameterEntry> {
        self.parameters
            .as_ref()
            .map(|store| store.entries().to_vec())
            .unwrap_or_default()
    }

    pub fn parameter_value(&self, name: &str) -> Option<&ParameterValue> {
        self.parameters.as_ref()?.value(name)
    }

    pub fn parameters(&self) -> Option<&ParameterStore> {
        self.parameters.as_ref()
    }

    pub fn set_parameter(&mut self, name: &str, value: ParameterValue) -> Result<(), String> {
        self.parameters
            .as_mut()
            .ok_or_else(|| "当前没有可编辑参数".to_string())?
            .set_value(name, value)?;
        self.pending_render_at = Some(Instant::now() + PARAM_RENDER_DEBOUNCE);
        Ok(())
    }

    pub fn restore_parameter(&mut self, name: &str) -> Result<(), String> {
        self.parameters
            .as_mut()
            .ok_or_else(|| "当前没有可编辑参数".to_string())?
            .restore_default(name)?;
        self.pending_render_at = Some(Instant::now() + PARAM_RENDER_DEBOUNCE);
        Ok(())
    }

    pub fn apply_preset(&mut self, name: &str) -> Result<(), String> {
        let values = self
            .presets
            .presets
            .get(name)
            .cloned()
            .ok_or_else(|| format!("未找到预设 {name}"))?;
        let store = self
            .parameters
            .as_mut()
            .ok_or_else(|| "当前没有可编辑参数".to_string())?;
        for (param_name, value) in values {
            let _ = store.set_value(&param_name, value);
        }
        self.pending_render_at = Some(Instant::now() + PARAM_RENDER_DEBOUNCE);
        Ok(())
    }

    pub fn set_presets(&mut self, presets: PresetFile) {
        self.presets = presets;
        if let Some(selected) = self.selected_preset.clone()
            && !self.presets.presets.contains_key(&selected)
        {
            self.selected_preset = None;
        }
    }

    pub fn take_pending_render(&mut self) -> bool {
        let Some(deadline) = self.pending_render_at else {
            return false;
        };
        if Instant::now() < deadline {
            return false;
        }
        self.pending_render_at = None;
        true
    }

    pub fn has_pending_render(&self) -> bool {
        self.pending_render_at.is_some()
    }

    pub fn preset_path(&self) -> Option<PathBuf> {
        self.source_path.as_deref().map(preset_path_for_source)
    }

    pub fn preset_names(&self) -> Vec<String> {
        self.presets.presets.keys().cloned().collect()
    }

    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }

    pub fn source_text(&self) -> &str {
        &self.source_text
    }
}
