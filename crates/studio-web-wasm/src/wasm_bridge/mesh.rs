//! Mesh 解码桥接。
//!
//! 解码纯逻辑位于 crate 顶层的 `mesh_decode` 模块（host + wasm32 共享），
//! 本文件只把结果包成 `MeshHandle` 并导出 wasm_bindgen 入口。
//!
//! MeshHandle 对 JS 暴露四个扁平数组 getter（positions / normals / colors /
//! indices），TS 侧可以直接转成 typed array 喂给 Three.js BufferGeometry，
//! 无需重新解码一遍 .3mf / .stl 字节。

use scad_scene::MeshData;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::mesh_decode::decode_mesh_bytes;

#[wasm_bindgen]
pub struct MeshHandle {
    pub(crate) data: MeshData,
}

#[wasm_bindgen]
impl MeshHandle {
    /// 顶点数量（positions.len() / 3 与 vertex count 相等）。
    #[wasm_bindgen(getter)]
    pub fn vertex_count(&self) -> u32 {
        self.data.vertices.len() as u32
    }

    /// 索引数量。
    #[wasm_bindgen(getter)]
    pub fn index_count(&self) -> u32 {
        self.data.indices.len() as u32
    }

    /// 扁平 positions: `[x0, y0, z0, x1, y1, z1, ...]`。
    pub fn positions(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.data.vertices.len() * 3);
        for v in &self.data.vertices {
            out.extend_from_slice(&v.position);
        }
        out
    }

    /// 扁平 normals: `[nx0, ny0, nz0, ...]`。
    pub fn normals(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.data.vertices.len() * 3);
        for v in &self.data.vertices {
            out.extend_from_slice(&v.normal);
        }
        out
    }

    /// 扁平 vertex colors: `[r0, g0, b0, a0, ...]`。若所有顶点都是纯白
    /// 默认色（alpha = 1），返回空 `Vec<f32>` 让 TS 侧走无色路径。
    pub fn colors(&self) -> Vec<f32> {
        let has_color = self
            .data
            .vertices
            .iter()
            .any(|v| v.color[3] >= 0.0 && v.color != [1.0, 1.0, 1.0, 1.0]);
        if !has_color {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(self.data.vertices.len() * 4);
        for v in &self.data.vertices {
            if v.color[3] < 0.0 {
                out.extend_from_slice(&[1.0, 1.0, 1.0, 1.0]);
            } else {
                out.extend_from_slice(&v.color);
            }
        }
        out
    }

    /// 扁平 indices。
    pub fn indices(&self) -> Vec<u32> {
        self.data.indices.clone()
    }
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
