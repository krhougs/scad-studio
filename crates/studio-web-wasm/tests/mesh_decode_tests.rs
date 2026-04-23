//! S1a host-side 覆盖 mesh 解码的纯逻辑：magic 判断 + 错误路径。
//! 避免引入新的 dev-dep（例如 `stl_io`），成功路径由 `scad-scene` 的 mesh
//! 测试覆盖；这里只确认 `studio_web_wasm::mesh_decode::decode_mesh_bytes`
//! 的 dispatch 分流与错误消息格式稳定。

use studio_web_wasm::mesh_decode::{decode_mesh_bytes, is_three_mf};

#[test]
fn is_three_mf_matches_zip_magic() {
    assert!(is_three_mf(b"PK\x03\x04more-bytes"));
}

#[test]
fn is_three_mf_rejects_non_zip_bytes() {
    assert!(!is_three_mf(b""));
    assert!(!is_three_mf(b"PK"));
    assert!(!is_three_mf(b"solid ascii"));
    assert!(!is_three_mf(&[0, 0, 0, 0, 0]));
}

#[test]
fn decode_mesh_bytes_rejects_empty_stl_input() {
    let result = decode_mesh_bytes(b"");
    let err = result.expect_err("empty input must fail mesh decode");
    assert!(err.starts_with("stl: "), "unexpected error prefix: {err}");
}

#[test]
fn decode_mesh_bytes_routes_zip_prefix_to_three_mf_branch() {
    // 构造最小 ZIP header bytes；后续解析必然失败（内容非 3mf），
    // 但必须命中 3mf 错误分支而不是 stl 分支。
    let bytes = b"PK\x03\x04\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let err = decode_mesh_bytes(bytes).expect_err("malformed 3mf should fail");
    assert!(err.starts_with("3mf: "), "unexpected error prefix: {err}");
}
