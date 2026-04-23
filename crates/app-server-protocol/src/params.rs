use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ParameterKind {
    Number {
        min: Option<f64>,
        step: Option<f64>,
        max: Option<f64>,
    },
    Bool,
    Choice {
        options: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParameterValue {
    Number(f64),
    Bool(bool),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterDefinition {
    pub name: String,
    pub group: Option<String>,
    pub hidden: bool,
    pub kind: ParameterKind,
    pub default_value: ParameterValue,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParsedParameters {
    pub items: Vec<ParameterDefinition>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParameterEntry {
    pub definition: ParameterDefinition,
    pub value: ParameterValue,
}
