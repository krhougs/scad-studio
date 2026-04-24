//! Customizer 参数与预设的 wasm 桥接。
//!
//! Web 侧只负责呈现控件；参数解析、define 格式化和共享预设 JSON
//! 序列化复用 `studio-common` / `app-server-protocol`。

use app_server_protocol::{ParameterEntry, PresetFile};
use serde::Serialize;
use studio_common::{
    ParameterStore, parameter_entries_to_cli_defines, parse_parameters,
};
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Serialize)]
struct ParsedParameterEntries {
    entries: Vec<ParameterEntry>,
    warnings: Vec<String>,
}

#[wasm_bindgen]
pub fn parameters_parse_source(source: &str) -> Result<JsValue, JsValue> {
    let parsed = parse_parameters(source);
    let store = ParameterStore::from_parsed(parsed.clone());
    to_json_value(&ParsedParameterEntries {
        entries: store.entries().to_vec(),
        warnings: parsed.warnings,
    })
    .map_err(|err| JsValue::from_str(&format!("parameters serialize: {err}")))
}

#[wasm_bindgen]
pub fn parameters_format_defines(entries: JsValue) -> Result<JsValue, JsValue> {
    let parsed: Vec<ParameterEntry> = serde_wasm_bindgen::from_value(entries)
        .map_err(|err| JsValue::from_str(&format!("invalid parameter entries: {err}")))?;
    to_json_value(&parameter_entries_to_cli_defines(&parsed))
        .map_err(|err| JsValue::from_str(&format!("defines serialize: {err}")))
}

#[wasm_bindgen]
pub fn presets_parse_shared_file(text: &str) -> Result<JsValue, JsValue> {
    let parsed: PresetFile = serde_json::from_str(text)
        .map_err(|err| JsValue::from_str(&format!("preset parse failed: {err}")))?;
    to_json_value(&parsed)
        .map_err(|err| JsValue::from_str(&format!("preset serialize: {err}")))
}

#[wasm_bindgen]
pub fn presets_stringify_shared_file(file: JsValue) -> Result<String, JsValue> {
    let parsed: PresetFile = serde_wasm_bindgen::from_value(file)
        .map_err(|err| JsValue::from_str(&format!("invalid preset file: {err}")))?;
    serde_json::to_string_pretty(&parsed)
        .map(|text| format!("{text}\n"))
        .map_err(|err| JsValue::from_str(&format!("preset stringify failed: {err}")))
}

fn to_json_value(value: &impl Serialize) -> Result<JsValue, serde_wasm_bindgen::Error> {
    value.serialize(&serde_wasm_bindgen::Serializer::json_compatible())
}
