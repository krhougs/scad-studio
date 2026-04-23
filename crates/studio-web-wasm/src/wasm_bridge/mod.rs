//! wasm_bindgen bridge for studio-web-wasm.
//!
//! 这一层只做薄包装：把 `studio-common::ManagedClient`、`scad_scene` 的 mesh
//! 解码和 renderer 通过 `#[wasm_bindgen]` 暴露给 TypeScript。所有业务状态机
//! 归 `studio-common`；本模块不持有任何协议状态或 WebSocket 实例。

pub mod client;
pub mod mesh;
pub mod renderer;
pub(crate) mod transport;
