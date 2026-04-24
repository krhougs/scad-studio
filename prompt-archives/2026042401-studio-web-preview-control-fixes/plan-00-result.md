# Plan-00 Result：Studio Web 预览控制与坐标修正

## Phase 1：失败测试与依据核对

- 状态：已完成。
- 依据核对：
  - Base UI NumberField 文档确认受控用法为 `value` 与 `onValueChange`，并由 `Group`、`Input`、`Increment`、`Decrement` 组合输入控件。
  - `react-knob-headless` 官方文档确认 knob 使用 `valueRaw`、`valueMin`、`valueMax`、`valueRawRoundFn`、`valueRawDisplayFn` 与 `onValueRawChange`，rounding 应放在 `valueRawRoundFn`。
  - `scad-scene::mesh::openscad_to_viewer` 将 OpenSCAD `[x, y, z]` 映射为 viewer `[x, z, -y]`，所以 UI 坐标轴必须展示 OpenSCAD 语义轴。
  - `scad-scene::OrbitalCamera` 中左键为 orbit，中键和右键为 pan，wheel 正向缩小距离，平移按当前相机距离缩放。
  - `window.__studioWebPreviewDelayMs` 只允许作为浏览器测试或开发观测钩子，用于制造可控 preview pending；生产逻辑不得依赖该字段。
- 已添加失败测试：
  - 参数范围不再随 current value 扩大。
  - Web 相机缩放方向与中键平移语义对齐 desktop OrbitControls。
  - OpenSCAD 语义轴到 viewer 方向的固定映射。
  - 参数与相机数值控件必须暴露 knob 与 NumberField。
  - 参数与相机数值输入过程保持行级布局稳定。
  - ViewportGizmo 点击切换视角。
  - 初次 mesh 预览必须在可控 preview 延迟下展示显眼 loading。
  - mesh 渲染必须等 mesh info、真实 viewport 宽高和 device pixel ratio 可用后再 frame。
  - plate、grid、axis 等辅助元素尺寸必须随真实 mesh dimensions 变化。
  - 远距离相机的 near/far 必须覆盖真实 mesh bounds。
- 渲染异步时机检查矩阵：
  - 远端 preview 异步结果：`MeshViewer` 初次 pending 当前没有显眼 overlay；已由浏览器测试覆盖，纳入 Phase 4 修复。
  - 真实 mesh bounds：`mesh-three.setMesh` 在 payload 到达后才有 `computeMeshInfo`，但 frame 与 scene helper 需要统一通过真实 `MeshInfo`；已由 `mesh-render-metrics` 失败测试覆盖，纳入 Phase 4。
  - 真实图片尺寸：`ImageViewer` 在 `img.onload` 后读取 natural size，等待期间已有 overlay；本轮未确认存在永久占位尺寸问题，Phase 4 只做回归保护。
  - 真实 viewport 与 device pixel ratio：`mesh-three` 初始 `viewportWidth/viewportHeight` 为 `1`，`frameToInfo` 可能先于 `ResizeObserver` 使用占位 aspect；已由 frame readiness 失败测试覆盖，纳入 Phase 4。
  - projection 状态：orthographic 宽高当前由相机距离和占位 viewport 推导，未绑定真实 bounds 的准备状态；已纳入 Phase 4。
  - plate、grid、axis、fog 与 clip plane：当前 plate/grid/fog/clip 在 `setMesh` 后使用 `MeshInfo`，但缺少统一 readiness；axis 仍是 Three.js 内部轴且没有 OpenSCAD 语义 gizmo，纳入 Phase 3 与 Phase 4。
  - 远距离相机裁切：`mesh-three.dolly` 允许距离到 `20000`，但 camera near/far 没有按当前相机距离与 bounds 重新计算；已由远距离 clipping 失败测试覆盖，纳入 Phase 5。
- 失败验证：
  - `bun x vitest run tests/unit/parameter-model.test.ts tests/unit/camera-controls.test.ts tests/unit/openscad-axis.test.ts tests/unit/mesh-render-metrics.test.ts` 已失败，失败原因分别为目标模块缺失、参数范围仍随 current value 扩大、wheel 方向与中键 pan 语义未对齐。
  - `bun x playwright test tests/playwright/parameters-presets.spec.ts --grep "typed controls drive current defines"` 已失败，原因是参数 NumberField/knob 结构尚不存在。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "initial mesh preview exposes prominent loading"` 已失败，原因是在可控 preview 延迟下初次 loading overlay 尚不存在。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "ViewportGizmo|preview info and camera controls"` 已失败，原因是相机 knob/NumberField 和 ViewportGizmo 尚不存在。
- Review：
  - 独立 subagent 三轮只读 review 已完成；最终结论为无 blocker、无 important，Phase 1 可以完成。
