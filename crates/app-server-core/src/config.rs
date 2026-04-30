use std::{fmt, path::PathBuf};

use app_server_protocol::{AppConfigDto, DisplayUnitDto, HostLocalPath, SlicerConfigDto};
use studio_common::{AppConfig, DisplayUnit, SlicerConfig};
use tokio::fs;

#[derive(Debug)]
pub struct ConfigError(String);

pub fn config_file_path() -> Result<PathBuf, ConfigError> {
    dirs::config_dir()
        .map(|dir| dir.join("scad-studio").join("config.json"))
        .ok_or_else(|| ConfigError("无法确定配置目录".into()))
}

pub async fn load_config() -> Result<AppConfig, ConfigError> {
    let path = config_file_path()?;
    if !fs::try_exists(path.clone())
        .await
        .map_err(|error| ConfigError(format!("读取配置失败: {error}")))?
    {
        return Ok(AppConfig::default());
    }
    let json = fs::read_to_string(path)
        .await
        .map_err(|error| ConfigError(format!("读取配置失败: {error}")))?;
    AppConfig::from_json(&json).map_err(|error| ConfigError(error.to_string()))
}

pub async fn save_config(config: &AppConfig) -> Result<(), ConfigError> {
    let path = config_file_path()?;
    if let Some(parent) = path.parent().map(|path| path.to_path_buf()) {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| ConfigError(format!("创建配置目录失败: {error}")))?;
    }
    fs::write(
        path,
        config
            .to_json()
            .map_err(|error| ConfigError(error.to_string()))?,
    )
    .await
    .map_err(|error| ConfigError(format!("写入配置失败: {error}")))
}

pub async fn load_config_dto() -> Result<AppConfigDto, ConfigError> {
    app_config_to_dto(&load_config().await?)
}

pub async fn save_config_dto(config: AppConfigDto) -> Result<(), ConfigError> {
    save_config(&app_config_from_dto(config)?).await
}

pub async fn load_config_json() -> Result<String, ConfigError> {
    load_config()
        .await?
        .to_json()
        .map_err(|error| ConfigError(error.to_string()))
}

pub async fn save_config_json(json: &str) -> Result<(), ConfigError> {
    let config = AppConfig::from_json(json).map_err(|error| ConfigError(error.to_string()))?;
    save_config(&config).await
}

pub fn app_config_to_dto(config: &AppConfig) -> Result<AppConfigDto, ConfigError> {
    Ok(AppConfigDto {
        openscad_path: optional_path_to_host_local(config.openscad_path.as_ref())?,
        slicers: config
            .slicers
            .iter()
            .map(slicer_to_dto)
            .collect::<Result<Vec<_>, _>>()?,
        recent_workspaces: config
            .recent_workspaces
            .iter()
            .map(path_to_host_local)
            .collect::<Result<Vec<_>, _>>()?,
        floating_panel_opacity: config.floating_panel_opacity,
        left_panel_width: config.left_panel_width,
        right_panel_width: config.right_panel_width,
        display_unit: display_unit_to_dto(config.display_unit),
        camera_overlay_pos: config.camera_overlay_pos,
        camera_overlay_size: config.camera_overlay_size,
        param_panel_pos: config.param_panel_pos,
        param_panel_size: config.param_panel_size,
        log_panel_pos: config.log_panel_pos,
        log_panel_size: config.log_panel_size,
    })
}

pub fn app_config_from_dto(config: AppConfigDto) -> Result<AppConfig, ConfigError> {
    Ok(AppConfig {
        openscad_path: config.openscad_path.map(|path| path.to_path_buf()),
        slicers: config
            .slicers
            .into_iter()
            .map(slicer_from_dto)
            .collect::<Vec<_>>(),
        recent_workspaces: config
            .recent_workspaces
            .into_iter()
            .map(|path| path.to_path_buf())
            .collect(),
        floating_panel_opacity: config.floating_panel_opacity,
        left_panel_width: config.left_panel_width,
        right_panel_width: config.right_panel_width,
        display_unit: display_unit_from_dto(config.display_unit),
        camera_overlay_pos: config.camera_overlay_pos,
        camera_overlay_size: config.camera_overlay_size,
        param_panel_pos: config.param_panel_pos,
        param_panel_size: config.param_panel_size,
        log_panel_pos: config.log_panel_pos,
        log_panel_size: config.log_panel_size,
    })
}

fn slicer_to_dto(config: &SlicerConfig) -> Result<SlicerConfigDto, ConfigError> {
    Ok(SlicerConfigDto {
        name: config.name.clone(),
        path: path_to_host_local(&config.path)?,
    })
}

fn slicer_from_dto(config: SlicerConfigDto) -> SlicerConfig {
    SlicerConfig {
        name: config.name,
        path: config.path.to_path_buf(),
    }
}

fn optional_path_to_host_local(
    path: Option<&PathBuf>,
) -> Result<Option<HostLocalPath>, ConfigError> {
    path.map(path_to_host_local).transpose()
}

fn path_to_host_local(path: &PathBuf) -> Result<HostLocalPath, ConfigError> {
    let value = path
        .to_str()
        .ok_or_else(|| ConfigError("host-local path 必须是 UTF-8".into()))?;
    HostLocalPath::new(value).map_err(|error| ConfigError(error.message))
}

fn display_unit_to_dto(unit: DisplayUnit) -> DisplayUnitDto {
    match unit {
        DisplayUnit::Millimeter => DisplayUnitDto::Millimeter,
        DisplayUnit::Centimeter => DisplayUnitDto::Centimeter,
        DisplayUnit::Inch => DisplayUnitDto::Inch,
    }
}

fn display_unit_from_dto(unit: DisplayUnitDto) -> DisplayUnit {
    match unit {
        DisplayUnitDto::Millimeter => DisplayUnit::Millimeter,
        DisplayUnitDto::Centimeter => DisplayUnit::Centimeter,
        DisplayUnitDto::Inch => DisplayUnit::Inch,
    }
}

impl std::error::Error for ConfigError {}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
