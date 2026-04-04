## 执行结果

### Phase 1

- 已完成。
- 在 `tests/mesh_tests.rs` 新增 `mesh_data_splits_opaque_and_transparent_triangle_indices`。
- 在 `tests/pipeline_tests.rs` 新增 `solid_transparent_pipeline_uses_alpha_blend_without_depth_write`。
- 两条测试在现状下都先失败：
  - `MeshData` 不存在透明索引分区 API
  - `pipeline` 不存在透明 Solid 管线策略函数

### Phase 2

- 已完成。
- 在 `src/mesh.rs` 中新增 `triangle_index_partitions()`，按三角面 alpha 把索引分成不透明与透明两组。
- 在 `src/renderer.rs` 中：
  - 为 mesh 同时保留完整索引、不透明索引、透明索引
  - `Solid` 模式下改为先绘制不透明三角面，再绘制透明三角面
  - 透明三角面使用独立透明管线，且不参与阴影 pass
- 在 `src/pipeline.rs` 中新增透明 Solid 管线配置：
  - alpha blend
  - 关闭深度写入
- 在 `src/shader.wgsl` 中让彩色模式输出真实 model alpha，供透明 Solid pass 使用。

### 补充证据

- 使用 `/Users/krhougs/LocalCodes/scad-play/mini_itx_pc_003.scad` 实际导出 3MF 后确认：
  - 模型为单对象、单 build item
  - 含有半透明颜色 `#1A1A2EF2`（屏幕玻璃）
  - 通过只读分析脚本发现，玻璃与浅色背板之间存在大量距离很近、法线相反的平行面片
- 这与“`X-Ray` 正常、`Solid` 闪烁”这一现象一致，支持本轮根因判断。

### Phase 3

- 已完成。
- 运行 `cargo fmt --all`。
- 运行 `git diff --check`，未发现补丁格式问题。
- 运行：
  - `cargo test --test mesh_tests`，6/6 通过
  - `cargo test --test pipeline_tests`，14/14 通过
  - `cargo test --test three_mf_tests`，9/9 通过

## 遗留风险

- 当前透明三角面还没有做按相机距离排序；对于更复杂的多层透明结构，后续可能仍需做透明面排序优化。
- 本轮没有 GUI 级截图验证能力，最终仍需要用户在实际应用里用 `mini_itx_pc_003.scad` 再确认一次。
