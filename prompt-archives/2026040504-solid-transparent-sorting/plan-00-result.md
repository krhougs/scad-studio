## 执行结果

### Phase 1

- 已完成。
- 在 `tests/mesh_tests.rs` 新增 `mesh_data_sorts_transparent_triangles_back_to_front_for_eye_position`。
- 运行 `cargo test mesh_data_sorts_transparent_triangles_back_to_front_for_eye_position --test mesh_tests` 后按预期失败，失败原因是 `MeshData` 不存在透明三角面排序 API。

### Phase 2

- 已完成。
- 在 `src/mesh.rs` 中新增 `sorted_transparent_triangle_indices(eye_position)`，按三角面质心到相机距离从远到近排序透明三角面索引。
- 在 `src/renderer.rs` 中，`Solid` 透明 pass 每帧基于当前 `camera.eye()` 更新透明索引缓冲，再执行透明绘制。
- 这一轮没有改动不透明 pass，也没有改动 `X-Ray` 路径。

### Phase 3

- 已完成。
- 运行 `cargo fmt --all`。
- 运行 `git diff --check`，未发现补丁格式问题。
- 运行：
  - `cargo test --test mesh_tests`，7/7 通过
  - `cargo test --test pipeline_tests`，14/14 通过
  - `cargo test --test three_mf_tests`，9/9 通过

## 遗留风险

- 目前透明三角面排序使用三角面质心距离，而不是更精细的 per-fragment OIT；对于极端交错的透明几何，仍可能存在排序误差。
- 本轮仍缺少 GUI 级截图验证，需要用户直接用 `mini_itx_pc_003.scad` 在应用里再确认一次。
