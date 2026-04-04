## 执行结果

### Phase 1

- 已完成。
- 在 `tests/pipeline_tests.rs` 新增 `color_mode_uses_weaker_specular_strength_than_mono_mode`。
- 首次运行 `cargo test color_mode_uses_weaker_specular_strength_than_mono_mode --test pipeline_tests` 时，因当前实现缺少 `pipeline_specular_strength` API 而编译失败；这直接表明现状尚未为 `ColorMode` 提供独立高光策略。
- 在新增 API 并接入实现后重新运行，测试转绿。

### Phase 2

- 已完成。
- 在 `src/pipeline.rs` 增加 `pipeline_specular_strength(ColorMode)`，使彩色模式使用更弱的高光系数。
- 在 `src/renderer.rs` 将高光强度写入 `SceneUniform.render_params.w`。
- 在 `src/shader.wgsl` 与 `src/shader_xray.wgsl` 中：
  - 用新的 uniform 控制高光强度
  - 在彩色模式下将高光颜色向底色收敛，避免纯白高光块
- 未修改上一轮的 `src/mesh.rs` 法线修复逻辑。

### Phase 3

- 已完成。
- 运行 `cargo fmt --all`。
- 运行 `git diff --check`，未发现补丁格式问题。
- 格式化后重新运行：
  - `cargo test --test pipeline_tests`，12/12 通过
  - `cargo test --test mesh_tests`，5/5 通过
  - `cargo test --test three_mf_tests`，9/9 通过

## 遗留风险

- 本轮是基于当前默认 `shadows_enabled = false` 的运行路径定位出的高光问题；若用户在开启阴影后仍能稳定复现“方块”伪影，需要继续单独排查 shadow map bias / PCF 稳定性。
- 彩色模式高光强度当前采用固定系数 `0.3`，这是偏保守的经验值；后续若需要更强材质质感，可以再根据实际模型样本细调。
