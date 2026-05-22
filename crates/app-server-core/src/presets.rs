use app_server_protocol::{ParameterValue, PresetFile};
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};
use tokio::fs;

#[derive(Debug)]
pub struct PresetError(String);

pub fn preset_path_for_source(source_path: &Path) -> PathBuf {
    source_path.with_extension("scad.json")
}

pub async fn load_presets(path: &Path) -> Result<PresetFile, PresetError> {
    if !fs::try_exists(path)
        .await
        .map_err(|error| PresetError(format!("读取预设文件失败: {error}")))?
    {
        return Ok(PresetFile::default());
    }
    let contents = fs::read_to_string(path)
        .await
        .map_err(|error| PresetError(format!("读取预设文件失败: {error}")))?;
    serde_json::from_str(&contents)
        .map_err(|error| PresetError(format!("解析预设文件失败: {error}")))
}

pub async fn save_preset(
    path: &Path,
    name: &str,
    values: &BTreeMap<String, ParameterValue>,
) -> Result<(), PresetError> {
    let mut file = load_presets(path).await?;
    file.presets.insert(name.to_string(), values.clone());
    write_presets(path, &file).await
}

pub async fn delete_preset(path: &Path, name: &str) -> Result<(), PresetError> {
    let mut file = load_presets(path).await?;
    file.presets.remove(name);
    write_presets(path, &file).await
}

async fn write_presets(path: &Path, file: &PresetFile) -> Result<(), PresetError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|error| PresetError(format!("创建预设目录失败: {error}")))?;
    }
    let json = serde_json::to_string_pretty(file)
        .map_err(|error| PresetError(format!("序列化预设文件失败: {error}")))?;
    fs::write(path, json)
        .await
        .map_err(|error| PresetError(format!("写入预设文件失败: {error}")))
}

impl std::error::Error for PresetError {}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
