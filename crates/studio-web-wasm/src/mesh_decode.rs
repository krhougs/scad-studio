//! Mesh bytes 解码的纯逻辑实现。
//!
//! 与 wasm_bindgen 无关；host 和 wasm32 target 都可编译、测试。
//! 逻辑：判断头部 `PK\x03\x04` 走 3mf；否则走 STL。

use std::io::Cursor;

use scad_scene::{MeshData, mesh::load_stl_from_reader, three_mf::load_3mf_from_reader};

pub fn decode_mesh_bytes(bytes: &[u8]) -> Result<MeshData, String> {
    if is_three_mf(bytes) {
        let mut cursor = Cursor::new(bytes);
        load_3mf_from_reader(&mut cursor).map_err(|err| format!("3mf: {err}"))
    } else {
        let mut cursor = Cursor::new(bytes);
        load_stl_from_reader(&mut cursor).map_err(|err| format!("stl: {err}"))
    }
}

pub fn is_three_mf(bytes: &[u8]) -> bool {
    bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04"
}
