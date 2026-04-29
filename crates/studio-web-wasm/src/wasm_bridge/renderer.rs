//! Renderer 桥接（Phase 2b 桩实现）。
//!
//! `scad_scene::Renderer::new_for_canvas` 依赖 wgpu adapter / device 异步请求，
//! 在 wasm 目标下需要借道 `wasm-bindgen-futures` 等待 JS Promise；而桥接契约
//! §10 明确禁止 wasm 内等待 JS 异步结果。Phase 2b 保留签名并返回 Err；真实
//! 实现推到 Phase 3（由 TS 侧管理异步 create，或在 `scad_scene` 补充同步
//! instance API 后再落地）。

use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use super::mesh::MeshHandle;

#[wasm_bindgen]
pub struct RendererHandle {
    _private: (),
}

#[wasm_bindgen]
pub fn renderer_create(_canvas_id: &str) -> Result<RendererHandle, JsValue> {
    Err(JsValue::from_str(
        "renderer not implemented on web yet (Phase 2b stub; async wgpu adapter init must be wired in Phase 3)",
    ))
}

#[wasm_bindgen]
pub fn renderer_resize(
    _handle: &mut RendererHandle,
    _width: u32,
    _height: u32,
    _device_pixel_ratio: f32,
) {
    // 桩实现：handle 无法被创建，此函数实际不可达；保留签名对齐契约 §4。
}

#[wasm_bindgen]
pub fn renderer_render(
    _handle: &mut RendererHandle,
    _mesh: &MeshHandle,
    _camera: JsValue,
) -> Result<(), JsValue> {
    Err(JsValue::from_str(
        "renderer not implemented on web yet (Phase 2b stub)",
    ))
}

#[wasm_bindgen]
pub fn renderer_destroy(_handle: RendererHandle) {
    // 消费 handle，由 drop 清理。实际上 handle 创建会先失败，destroy 幂等成立。
}
