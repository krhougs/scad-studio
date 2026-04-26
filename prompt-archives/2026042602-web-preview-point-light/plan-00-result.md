# Plan-00 Result：Studio Web 预览额外点光源控制

## 状态总览

- 状态：执行中。
- 当前任务：为 Studio Web 预览增加额外点光源模式、手动位置、reset 和 shadow 强制启用运行时语义。
- 计划存档：`prompt-archives/2026042602-web-preview-point-light/plan-00.md`。

## Phase 1：配置模型与失败测试

- 状态：已完成。
- 变更摘要：
  - 扩展 `.scad.json` 读写单元测试，覆盖 `pointLightMode`、`pointLightPosition` 的默认值、合法读取、非法值归一化、写回和 auto/off 模式保留手动位置。
  - 扩展自动位置单元测试，使用非零中心 bounds 验证 `center + normalize([1,-1,1]) * frontDistance`。
  - 扩展 Playwright appearance 用例，覆盖 off/auto/manual 按钮选中态、直接 `off -> manual`、manual X/Y/Z、reset、per file 隔离、shadow forced、不污染持久化配置、auto/manual shadow 非 forced 分支。
  - 扩展 preview request dedup 用例，覆盖 direct manual、auto/manual/off、X/Y/Z 和 reset 均不触发 `.scad` preview request。
  - 扩展 parameters presets 用例，固定新增默认字段和 presets 共存。
- 失败验证：
  - `bun x vitest run tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts`：失败符合预期，原因是生产代码尚未提供点光源字段和 `pointLightAutoPositionForBounds`。
  - `bun run --cwd packages/studio-web typecheck`：失败符合预期，原因是 `mesh-render-metrics` 尚未导出 `pointLightAutoPositionForBounds`。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "scad preview appearance controls persist per file" --timeout=20000`：失败符合预期，原因是 canvas 尚无 `data-point-light-mode`。
  - `bun x playwright test tests/playwright/preview-request-dedup.spec.ts --grep "appearance changes do not emit preview request" --timeout=15000`：失败符合预期，原因是右侧栏尚无 `preview-point-light-mode-manual`。
- 独立 review：
  - 多轮只读 subagent review 已完成；最终结论为无 blocker、无 important，可以提交 Phase 1 并进入 Phase 2。
- 遗留问题：
  - 无。Phase 1 按计划只新增失败测试，未修改生产代码。

## Phase 2：点光源配置读写与右侧栏状态流

- 状态：已完成。
- 变更摘要：
  - 扩展 `PreviewAppearance`，支持 `pointLightMode` 与 `pointLightPosition` 的默认值、归一化和写回。
  - 增加自动点光源位置纯函数，复用 `front` camera framing distance。
  - 右侧栏增加 off / auto / manual 按钮、manual X/Y/Z 输入和 reset。
  - manual 默认位置和 reset 使用当前真实自动位置；无可靠位置时禁用会写配置的 manual/reset 路径，避免写入 fallback。
  - CanvasZone 将当前 `.scad` 预览的自动位置传给 ScadWorkbench；appearance 操作继续只写 `.scad.json`，不触发 `.scad` preview request。
  - 临时将点光源状态流同步到 canvas dataset，真实 Three.js `PointLight` 对象留给 Phase 3。
- 验证结果：
  - `bun run --cwd packages/studio-web typecheck`：通过。
  - `bun x vitest run tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts`：通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "scad preview appearance controls persist per file" --timeout=60000`：通过。
  - `bun x playwright test tests/playwright/preview-request-dedup.spec.ts --grep "appearance changes do not emit preview request" --timeout=30000`：通过。
  - `bun x playwright test tests/playwright/parameters-presets.spec.ts --grep "save, load, delete round-trip" --timeout=60000`：通过。
- 独立 review：
  - 初轮 review 发现无可靠自动位置时可能写入 fallback；已修复。
  - 复审结论为无 blocker、无 important，可以提交 Phase 2 并进入 Phase 3。
- 遗留问题：
  - 真实 `PointLight` 状态与 dataset 来源将在 Phase 3 处理。
  - Phase 4 回归可补充 manual 缺失 position 且 auto 未就绪的定向测试。

## Phase 3：Three.js 点光源运行时与自动位置

- 状态：未开始。

## Phase 4：完整回归与结果归档

- 状态：未开始。
