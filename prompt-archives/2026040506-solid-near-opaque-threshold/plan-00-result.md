## 执行结果

### Phase 1

- 已完成。
- 在 `tests/mesh_tests.rs` 新增 `mesh_data_treats_near_opaque_triangles_as_opaque_for_solid_partition`。
- 运行 `cargo test mesh_data_treats_near_opaque_triangles_as_opaque_for_solid_partition --test mesh_tests` 后按预期失败，现状会把 `alpha=0.95` 的三角面错误地归入透明分区。

### Phase 2

- 已完成。
- 在 `src/mesh.rs` 中引入 `SOLID_TRANSPARENCY_ALPHA_THRESHOLD = 0.9`。
- `triangle_index_partitions()` 现在只把 `0 < alpha < 0.9` 的三角面归为透明；`alpha=0.95` 这类近乎不透明的面会进入不透明 pass。
- 透明排序能力保持不变，真正半透明面仍会进入透明 pass 并排序。

### Phase 3

- 已完成。
- 运行 `cargo fmt --all`。
- 运行 `git diff --check`，未发现补丁格式问题。
- 运行：
  - `cargo test --test mesh_tests`，8/8 通过
  - `cargo test --test pipeline_tests`，15/15 通过
  - `cargo test --test three_mf_tests`，9/9 通过

## 说明

- 本轮是面向 `Solid` 模式稳定性的保守策略：把近乎不透明的玻璃按不透明表面处理，优先消除真实模型中的混合闪烁。
