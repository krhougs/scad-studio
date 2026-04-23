use crate::ParameterValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PresetFile {
    #[serde(default)]
    pub presets: BTreeMap<String, BTreeMap<String, ParameterValue>>,
}
