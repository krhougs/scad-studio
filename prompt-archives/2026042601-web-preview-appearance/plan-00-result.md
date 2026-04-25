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

- 状态：已完成。
- 前序目标保护：
  - 保留 Phase 1 的失败测试语义，并让其转为通过。
  - 保留 presets save / load / delete round-trip，`.scad.json` 新增 `previewAppearance` 时不丢 presets。
  - 外观变化不触发新的远端 `preview.request`，只更新本地 Three.js viewer options。
  - 外部 `.scad.json` 修改仍刷新 presets 面板，但不刷新 mesh preview。
- 本轮变更摘要：
  - `viewer-options.ts` 新增 `PreviewAppearance`、`DEFAULT_PREVIEW_APPEARANCE` 与 `normalizePreviewAppearance`，默认背景为 `#181b20`，网格主线为 `#5a6573`，网格细线为 `#343b45`，光照强度为 `1.25`。
  - `preset-io.ts` 扩展 `.scad.json` 读写，支持 `previewAppearance` 与 `presets` 共存；旧文件缺字段时回退默认值，非法颜色和光照强度会归一化。
  - 新增 `PreviewAppearancePanel`，右侧 Inspector 的 `.scad` 文件上下文中显示背景颜色、网格主线颜色、网格细线颜色和光照强度控件。
  - `ScadWorkbenchState` 加载当前 `.scad` 对应 `<stem>.scad.json` 中的外观配置；调整后实时更新 viewer options，并按 debounce 写回同一个 `.scad.json`。
  - `.scad.json` 写入使用 path + 完整 `PresetFile` 快照，并通过写入队列、epoch、dirty/version 检查处理文件切换、外部刷新、preset 保存和 appearance 写回的并发风险。
  - `WorkbenchLayout` 将 `.scad` 源文件刷新和 `.scad.json` settings 刷新拆分；settings 刷新只重新加载 presets / appearance，不触发 mesh 重新预览。
  - 修复用户补充发现的问题：修改 appearance 不再触发 `.scad` 重新渲染请求。
- 本轮验证：
  - `bun run typecheck`
    - 结果：通过。
  - `bun x vitest run tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts`
    - 结果：2 个测试文件通过，20 个测试通过。
  - `bun x playwright test tests/playwright/parameters-presets.spec.ts --grep "save, load, delete round-trip"`
    - 结果：1 个浏览器用例通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "scad preview appearance controls persist per file"`
    - 结果：1 个浏览器用例通过。
  - `bun x playwright test tests/playwright/preview-request-dedup.spec.ts --grep "appearance changes do not emit preview request|external scad settings refresh does not emit preview request"`
    - 结果：2 个浏览器用例通过。
  - `bun x playwright test tests/playwright/browser-watch-smoke.spec.ts --grep "preset list refreshes"`
    - 结果：1 个浏览器用例通过。
- 独立 review：
  - 多轮 review 发现并推动修复了 `.scad.json` 异步加载污染、debounce 写回串文件、presets 与 appearance 全量写回互相覆盖、dirty 状态自触发写回、in-flight 写入覆盖、旧测试断言不兼容新字段、外部 settings 刷新与本地 pending 写入交错等问题。
  - 最后一轮 review 无 blocker；指出的外部 settings refresh 与 in-flight 旧写入风险已通过 settings refresh 递增 write epoch 修复，并完成针对性验证。
- 遗留问题：
  - 当前没有已确认的 Phase 2 blocker。失败写入路径已有错误日志与 presetError 回写；更复杂的事务型冲突恢复不在本轮范围内。

## Phase 3：Three.js 背景、网格与打光优化

- 状态：已完成。
- 前序目标保护：
  - 保留 Phase 1 和 Phase 2 的配置模型、`.scad.json` 持久化、右侧栏实时控制和 per `.scad` 文件隔离。
  - 保留用户补充要求：修改 appearance 或外部 `.scad.json` settings 不触发新的 `.scad` preview request。
  - 未改动 mesh payload、OpenSCAD 请求参数、项目坐标系、相机交互、ViewportGizmo、裁切或 vertex color 主逻辑。
- 本轮变更摘要：
  - `mesh-three.ts` 将背景色、网格主线颜色、网格细线颜色和光照强度接入当前 `MeshViewerOptions`，并在 options 变化时实时应用。
  - 打光改为环境光、Z-up 半球光和多方向补光组合，光照强度作为整体倍率应用，保持各光源比例稳定。
  - `GridHelper` 颜色变化时重建网格，并释放旧 geometry 与材质；dispose 路径改为覆盖数组材质。
  - 新增 canvas dataset：`data-grid-color-signature` 反映 `GridHelper.geometry` 的颜色属性变化，`data-light-rig-intensity` 反映当前灯光组合总强度。
  - `canvas-interaction` Playwright 用例补充背景截图产物、网格几何颜色签名变化、光照倍率变化和测试工作区 `model.stl` 复原，避免同一 spec 重跑时被前次写入污染。
  - 修复 viewer toolbar 回归中稳定复现的 mesh 文件刷新问题：非 `.scad` 当前文档遇到目录级 watch 事件时刷新自身；`.scad` 的 settings 变化仍只刷新 settings，不触发 mesh preview。
- 本轮验证：
  - `bun run --cwd packages/studio-web typecheck`
    - 结果：通过。
  - `bun x playwright test tests/playwright/canvas-interaction.spec.ts --grep "viewer toolbar drives render state|scad preview appearance controls persist per file"`
    - 结果：2 个浏览器用例通过。
  - `bun x playwright test tests/playwright/preview-request-dedup.spec.ts --grep "appearance changes do not emit preview request|external scad settings refresh does not emit preview request"`
    - 结果：2 个浏览器用例通过。
  - `bun x playwright test tests/playwright/browser-watch-smoke.spec.ts --grep "mesh viewer refreshes"`
    - 结果：1 个浏览器用例通过。
- 独立 review：
  - 第一轮 review 无 blocker；指出半球光应显式适配 Z-up、网格颜色需要接近 Three.js 实际状态验证、光照断言不应硬编码权重总和。
  - 已修复：`HemisphereLight` 设置为 Z-up，新增 `GridHelper.geometry` 颜色签名 dataset，光照测试改为验证倍率关系。
  - 第二轮 review 无 blocker / important；minor 提到网格签名可进一步精确化，签名索引应避免和 divisions 隐式绑定。已将 `GRID_DIVISIONS` 抽为常量；精确签名因 Three.js 几何颜色使用线性色彩空间，本轮保留“几何颜色发生变化”的稳定断言。
  - 最终 review 无 blocker / important；确认 watch 修复不会违反 appearance/settings 不触发 `.scad` preview request 的主线要求。
- 遗留问题：
  - 非 `.scad` 文档的目录级 watch fallback 可能在未来服务端输出更精确文件路径后造成额外刷新；当前 host watch payload 仍可能只给目录 handle，本轮采用最小修复以恢复当前活动 mesh/markdown/image 的刷新能力。
  - 网格签名当前验证 `GridHelper` 几何颜色发生变化，不精确断言 sRGB 输入值，避免和 Three.js 线性色彩空间实现细节绑定。

## Phase 4：完整回归与结果归档

- 状态：未开始。
