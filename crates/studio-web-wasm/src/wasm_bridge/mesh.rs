//! Mesh 解码桥接。
//!
//! 解码纯逻辑位于 crate 顶层的 `mesh_decode` 模块（host + wasm32 共享），
//! 本文件只把结果包成 `MeshHandle` 并导出 wasm_bindgen 入口。

use scad_scene::MeshData;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::mesh_decode::decode_mesh_bytes;

#[wasm_bindgen]
pub struct MeshHandle {
    #[allow(dead_code)]
    pub(crate) data: MeshData,
}

#[wasm_bindgen]
pub fn mesh_decode(bytes: &[u8]) -> Result<MeshHandle, JsValue> {
    let data = decode_mesh_bytes(bytes)
        .map_err(|err| JsValue::from_str(&format!("mesh decode failed: {err}")))?;
    Ok(MeshHandle { data })
}

#[wasm_bindgen]
pub fn mesh_destroy(_handle: MeshHandle) {
    // `MeshHandle` 由值传入，函数退出即释放内部 `MeshData`。
}
