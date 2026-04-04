## 执行结果

### Phase 1

- 已完成。
- 在 `tests/camera_tests.rs` 新增 `matrices_for_bounds_tighten_depth_range_for_scene_bounds`。
- 运行 `cargo test matrices_for_bounds_tighten_depth_range_for_scene_bounds --test camera_tests` 后按预期失败，失败原因是 `OrbitalCamera` 还不存在基于 bounds 的裁剪面 API。

### Phase 2

- 已完成。
- 在 `src/camera.rs` 中新增：
  - `clipping_planes(bounds)`
  - `matrices_for_bounds(bounds)`
- 透视和正交投影现在都使用基于场景包围盒收紧后的 near/far，而不是固定 `0.01 .. 10000.0`。
- 在 `src/renderer.rs` 中，渲染 uniform 改为使用 `camera.matrices_for_bounds(Some(scene_bounds()))`。
- 在 `src/main.rs` 中，UI 相机矩阵和截面交互射线也改为使用 `current_bounds`，保证与渲染一致。
- 实现过程中出现一次 `f32` 类型推断错误，已修正。

### Phase 3

- 已完成。
- 运行 `cargo fmt --all`。
- 运行 `git diff --check`，未发现补丁格式问题。
- 运行：
  - `cargo test --test camera_tests`，8/8 通过
  - `cargo test --test mesh_tests`，8/8 通过
  - `cargo test --test pipeline_tests`，15/15 通过
  - `cargo test --test three_mf_tests`，9/9 通过

## 说明

- 这轮修复直接针对“远距离屏幕区域闪烁”的深度精度根因。
- 当前仓库里 `OrbitalCamera::matrices()` 仅保留为向后兼容包装，实际渲染和交互路径已切到 `matrices_for_bounds`。
