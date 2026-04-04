## 背景

- 用户在上一轮“斜面三角片感”修复后继续反馈：在斜着的带颜色平面上，仍然会闪烁默认白色的三角形或方形。
- 当前运行时使用根目录 `src/main.rs` 这一套渲染代码。
- 代码排查显示当前默认 `ViewerState` 为：
  - `RenderMode::Solid`
  - `ColorMode::Color`
  - `shadows_enabled = false`
- 因此若用户未主动开启阴影，问题更可能来自 `shader.wgsl` / `shader_xray.wgsl` 中对彩色表面仍叠加白色高光，而不是 shadow map。

## 本轮目标

- 降低彩色模式下白色高光导致的闪烁白块。
- 保持单色模式和现有基础光照结构不被破坏。
- 不回退上一轮的法线平滑修复。

## 注意事项

- 先写测试，再写实现。
- 只动着色路径相关代码：
  - `src/pipeline.rs`
  - `src/renderer.rs`
  - `src/shader.wgsl`
  - `src/shader_xray.wgsl`
  - 相关测试
- 不修改 3MF 解析、网格构建、OpenSCAD 导出链路。
