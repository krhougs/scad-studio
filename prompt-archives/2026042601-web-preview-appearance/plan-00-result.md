# Plan-00 Result：Studio Web 预览外观控制

## 状态总览

- 状态：进行中。
- 当前任务：按 `.scad` 文件持久化背景颜色、平面网格颜色和光照强度，并优化默认预览外观。
- 计划存档：`prompt-archives/2026042601-web-preview-appearance/plan-00.md`。

## Phase 1：配置模型与失败测试

- 状态：已完成。
- 前序目标保护：
  - 未修改生产代码、后端协议、mesh payload、相机控制、ViewportGizmo、加载状态和预览请求 dedup 行为。
  - 现有 presets 解析测试保留，新增断言只覆盖 per `.scad` 预览外观配置。
- 本轮变更摘要：
  - 扩展 `packages/studio-web/tests/unit/preset-io.test.ts`，覆盖 `.scad.json` 缺少外观字段时的默认值、外观字段读取、非法值归一化、presets 与外观字段一起写回。
  - 扩展 `packages/studio-web/tests/unit/mesh-render-metrics.test.ts`，固定默认背景颜色、网格主线颜色、网格细线颜色和光照强度。
  - 扩展 `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`，覆盖右侧栏外观控件、canvas dataset 实时更新、`.scad.json` 写回和切换 `.scad` 文件后的 per file 隔离。
  - Playwright 测试在 `beforeEach` 清理 `examples/params-cube.scad.json`，避免共享测试 workspace 被本用例写入污染。
- 失败验证：
  - `bun x vitest run tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts`
    - 结果：2 个测试文件执行成功，5 个预期失败。
    - 失败原因：`parsePresetFile` 尚未返回 `previewAppearance`，`stringifyPresetFile` 尚未写出 `previewAppearance`，`DEFAULT_MESH_VIEWER_OPTIONS` 尚未包含外观字段。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "scad preview appearance controls persist per file"`
    - 结果：1 个浏览器用例预期失败。
    - 失败原因：右侧 Inspector 中尚不存在 `preview-appearance-panel`。
- 独立 review：
  - 第一轮 review 无 blocker。
  - review 指出需要补齐切换文件后网格和光照回默认值断言、非法细网格颜色覆盖、共享 workspace 污染防护；已全部修复并重新运行红灯验证。
  - review minor 提到光照强度上限 `3` 是测试固定的交互边界；本轮采用 `0.25 - 3` 作为光照强度范围，后续实现按该范围处理。
- 遗留问题：
  - Phase 1 只提交失败测试；生产实现留到 Phase 2 和 Phase 3。

## Phase 2：外观配置读写与右侧栏接线

- 状态：未开始。

## Phase 3：Three.js 背景、网格与打光优化

- 状态：未开始。

## Phase 4：完整回归与结果归档

- 状态：未开始。
