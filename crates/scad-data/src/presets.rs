use std::{
    collections::BTreeMap,
    fmt,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::params::{ParameterStore, ParameterValue};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresetFile {
    #[serde(default)]
    pub presets: BTreeMap<String, BTreeMap<String, ParameterValue>>,
}

#[derive(Debug)]
pub struct PresetError(String);

pub fn preset_path_for_source(source_path: &Path) -> PathBuf {
    source_path.with_extension("scad.json")
}

pub fn load_presets(path: &Path) -> Result<PresetFile, PresetError> {
    if !path.exists() {
        return Ok(PresetFile::default());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| PresetError(format!("读取预设文件失败: {error}")))?;
    serde_json::from_str(&contents)
        .map_err(|error| PresetError(format!("解析预设文件失败: {error}")))
}

pub fn save_preset(path: &Path, name: &str, store: &ParameterStore) -> Result<(), PresetError> {
    let mut file = load_presets(path)?;
    file.presets.insert(name.to_string(), store.current_values());
    write_presets(path, &file)
}

pub fn delete_preset(path: &Path, name: &str) -> Result<(), PresetError> {
    let mut file = load_presets(path)?;
    file.presets.remove(name);
    write_presets(path, &file)
}

fn write_presets(path: &Path, file: &PresetFile) -> Result<(), PresetError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| PresetError(format!("创建预设目录失败: {error}")))?;
    }
    let json = serde_json::to_string_pretty(file)
        .map_err(|error| PresetError(format!("序列化预设文件失败: {error}")))?;
    fs::write(path, json).map_err(|error| PresetError(format!("写入预设文件失败: {error}")))
}

impl std::error::Error for PresetError {}

impl fmt::Display for PresetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
