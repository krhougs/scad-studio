use std::{fmt, fs, path::PathBuf};

use studio_common::AppConfig;

#[derive(Debug)]
pub struct ConfigError(String);

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
    let json =
        fs::read_to_string(&path).map_err(|error| ConfigError(format!("读取配置失败: {error}")))?;
    AppConfig::from_json(&json).map_err(|error| ConfigError(error.to_string()))
}

pub fn save_config(config: &AppConfig) -> Result<(), ConfigError> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| ConfigError(format!("创建配置目录失败: {error}")))?;
    }
    fs::write(
        path,
        config
            .to_json()
            .map_err(|error| ConfigError(error.to_string()))?,
    )
    .map_err(|error| ConfigError(format!("写入配置失败: {error}")))
}

pub fn load_config_json() -> Result<String, ConfigError> {
    load_config()?
        .to_json()
        .map_err(|error| ConfigError(error.to_string()))
}

pub fn save_config_json(json: &str) -> Result<(), ConfigError> {
    let config = AppConfig::from_json(json).map_err(|error| ConfigError(error.to_string()))?;
    save_config(&config)
}

impl std::error::Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
