# Prompt Archive: web preview appearance controls

## 原始问题

用户要求优化 Studio Web 模型预览体验：

1. 预览区域背景颜色需要提亮，平面网格颜色也需要随着一起优化对比度。
2. 调整打光策略，让模型在各个角度都能清楚、明亮地被看清楚。
3. 光照强度应该可以被实时调整，并放在右边栏里。

用户随后确认：

- 视觉验证使用 Playwright。
- 光照强度需要按 `.scad` 文件持久化。
- 背景颜色和平面网格颜色也需要可调，并按 `.scad` 文件持久化。
- 额外维护一套默认颜色。
- 接受把 per file 配置写入现有 `.scad.json` 文件。

## 已核对上下文

- 当前 Web 预览由 `packages/studio-web/src/viewers/mesh-three.ts` 的 Three.js WebGLRenderer 驱动。
- 当前默认背景是 `#101114`，并通过 `data-preview-background` 暴露给 Playwright。
- 当前网格使用 `GridHelper(200, 40, 0x2c2c31, 0x1a1a1d)`，主线和细线都偏暗。
- 当前光照组合为 `AmbientLight` + 三个 `DirectionalLight`，右侧栏已有 `camera`、`parameters`、`presets` 等 Inspector 分区。
- 当前 `.scad` 的 per file 文件使用 `derivePresetPath` 派生为 `<stem>.scad.json`，并通过 `parsePresetFile` / `stringifyPresetFile` 读写 presets。
- Context7 已核对 Three.js API：`AmbientLight`、`HemisphereLight`、`DirectionalLight` 都支持 `intensity`；`Scene.background` 可使用 `Color`；`WebGLRenderer.setClearColor` 可设置清屏颜色。

## 强制约束

- 不改变 app server、protocol、transport 和后端 mesh payload。
- 不回退项目坐标系、相机 preset、ViewportGizmo、加载状态、远距离裁切和预览请求 dedup 相关行为。
- per `.scad` 文件的预览外观配置写入现有 `<stem>.scad.json` 文件。
- 旧 `.scad.json` 缺少预览外观字段时，必须使用代码内默认配置。
- 背景颜色、平面网格颜色和光照强度都需要能从右侧栏实时调整。
- 背景、网格和光照配置只影响当前文件的预览外观，不影响另一个 `.scad` 文件。
- 使用 Playwright 验证 UI 和浏览器渲染路径。
- 工具链命令优先使用 `bun`。
- 按项目 Plan Mode 要求，每个 Phase 完成后必须使用独立 subagent review，并把结果写入 `plan-00-result.md`。

## 预期产出

- 一套默认预览外观配置，包含背景颜色、网格颜色和光照强度。
- `.scad.json` 读写保留 presets，并新增向后兼容的预览外观字段。
- 右侧栏新增预览外观控制区域，能实时调整背景颜色、网格颜色和光照强度。
- Three.js 预览在各角度更明亮、清楚，网格与背景对比度更合理。
- 单元测试和 Playwright 测试覆盖默认值、持久化、实时更新和文件切换隔离。
