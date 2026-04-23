use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlicerConfig {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub openscad_path: Option<PathBuf>,
    #[serde(default)]
    pub slicers: Vec<SlicerConfig>,
    #[serde(default)]
    pub recent_workspaces: Vec<PathBuf>,
    #[serde(default = "default_overlay_opacity", alias = "camera_overlay_opacity")]
    pub floating_panel_opacity: f32,
    #[serde(default)]
    pub camera_overlay_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub camera_overlay_size: Option<[f32; 2]>,
    #[serde(default)]
    pub param_panel_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub param_panel_size: Option<[f32; 2]>,
    #[serde(default)]
    pub log_panel_pos: Option<[f32; 2]>,
    #[serde(default)]
    pub log_panel_size: Option<[f32; 2]>,
}

fn default_overlay_opacity() -> f32 {
    0.85
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            openscad_path: None,
            slicers: Vec::new(),
            recent_workspaces: Vec::new(),
            floating_panel_opacity: default_overlay_opacity(),
            camera_overlay_pos: None,
            camera_overlay_size: None,
            param_panel_pos: None,
            param_panel_size: None,
            log_panel_pos: None,
            log_panel_size: None,
        }
    }
}

#[derive(Debug)]
pub struct ConfigError(String);

impl AppConfig {
    pub fn to_json(&self) -> Result<String, ConfigError> {
        serde_json::to_string_pretty(self)
            .map_err(|error| ConfigError(format!("序列化配置失败: {error}")))
    }

    pub fn from_json(json: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(json).map_err(|error| ConfigError(format!("解析配置失败: {error}")))
    }
}

impl std::error::Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
