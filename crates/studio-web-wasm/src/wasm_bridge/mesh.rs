//! Mesh 解码桥接。
//!
//! 输入 bytes 先尝试按 3MF 解析（zip 头 `PK\x03\x04`），否则按 STL 解析。

use std::io::Cursor;

use scad_scene::{MeshData, mesh::load_stl_from_reader, three_mf::load_3mf_from_reader};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

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

fn decode_mesh_bytes(bytes: &[u8]) -> Result<MeshData, String> {
    if is_three_mf(bytes) {
        let mut cursor = Cursor::new(bytes);
        load_3mf_from_reader(&mut cursor).map_err(|err| format!("3mf: {err}"))
    } else {
        let mut cursor = Cursor::new(bytes);
        load_stl_from_reader(&mut cursor).map_err(|err| format!("stl: {err}"))
    }
}

fn is_three_mf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04"
}
