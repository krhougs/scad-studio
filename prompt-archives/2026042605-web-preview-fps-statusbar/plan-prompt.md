# Prompt

用户要求：3D 渲染状态栏最右边展示 fps 帧率。

后续补充：

- 注意性能问题，避免频繁触发 React 重绘。
- 更新 FPS 不需要节流。
- FPS 显示相关代码尽量和 React 无关。

## 上下文

- web 端 3D 预览由 `packages/studio-web/src/viewers/mesh-three.ts` 中的 Three.js renderer 驱动。
- 状态栏位于 `packages/studio-web/src/workbench/canvas-zone.tsx` 的 `.canvas-statusbar`。
- `.scad` 预览与 mesh 文件预览都通过 `MeshViewer` 进入同一 Three.js viewer。

## 本次目标

- 在 3D 渲染状态栏最右侧展示 FPS。
- FPS 为运行时渲染指标，不持久化，不触发 preview request。
- 对 `.scad` 和 mesh tab 都生效；非 3D 文档不展示 FPS。
- FPS 数值更新不进入 React state 或 prop 链。
