use std::{fmt, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

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
    /// 浮动面板透明度 0.0~1.0
    #[serde(default = "default_overlay_opacity", alias = "camera_overlay_opacity")]
    pub floating_panel_opacity: f32,
    /// 相机面板位置（像素），None 表示使用默认位置
    #[serde(default)]
    pub camera_overlay_pos: Option<[f32; 2]>,
    /// 相机面板尺寸（像素），None 表示使用默认尺寸
    #[serde(default)]
    pub camera_overlay_size: Option<[f32; 2]>,
    /// 参数面板位置（像素），None 表示使用默认位置
    #[serde(default)]
    pub param_panel_pos: Option<[f32; 2]>,
    /// 参数面板尺寸（像素），None 表示使用默认尺寸
    #[serde(default)]
    pub param_panel_size: Option<[f32; 2]>,
    /// 日志面板位置（像素），None 表示使用默认位置
    #[serde(default)]
    pub log_panel_pos: Option<[f32; 2]>,
    /// 日志面板尺寸（像素），None 表示使用默认尺寸
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

pub fn config_file_path() -> Result<PathBuf, ConfigError> {
    dirs::config_dir()
        .map(|dir| dir.join("scad-studio").join("config.json"))
        .ok_or_else(|| ConfigError("无法确定配置目录".into()))
}

pub fn load_config() -> Result<AppConfig, ConfigError> {
    let path = config_file_path()?;
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let json = fs::read_to_string(&path)
        .map_err(|error| ConfigError(format!("读取配置失败: {error}")))?;
    AppConfig::from_json(&json)
}

pub fn save_config(config: &AppConfig) -> Result<(), ConfigError> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ConfigError(format!("创建配置目录失败: {error}")))?;
    }
    fs::write(path, config.to_json()?)
        .map_err(|error| ConfigError(format!("写入配置失败: {error}")))
}

impl std::error::Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
