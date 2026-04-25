# Plan-00：Studio Web 预览外观控制

## 背景

当前 Studio Web 模型预览已经具备 Three.js 渲染、项目坐标系、相机控制、ViewportGizmo、加载状态和 `.scad` 参数预览能力。用户反馈当前预览背景偏暗、平面网格对比度不足，模型在某些角度不够明亮，同时要求光照强度、背景颜色和平面网格颜色都能在右侧栏实时调整，并按 `.scad` 文件持久化。

本计划只处理 Web 端模型预览外观体验，不改变后端协议、OpenSCAD 输出、mesh payload、坐标系或相机交互语义。

## 目标

- 提亮预览背景，并提供可维护的默认预览外观配置。
- 优化网格颜色与背景的对比度。
- 调整 Three.js 打光策略，使模型从不同角度都更清楚、明亮。
- 在右侧 Inspector 中提供背景颜色、平面网格颜色和光照强度的实时控制。
- 将上述外观配置按 `.scad` 文件持久化到现有 `<stem>.scad.json` 文件。
- 保持旧 `.scad.json` 文件兼容，缺字段时使用默认外观配置。

## 非目标

- 不修改 app server、protocol、transport 或 `studio-web-wasm` mesh 解析契约。
- 不新增全局用户偏好设置界面。
- 不修改 STL / 3MF / `.scad` 预览请求 payload。
- 不重构整个 workbench、Inspector 或 presets 架构。
- 不处理 Vite chunk size warning。

## 强制约束识别

- 用户指定 Playwright 作为视觉验证方式。
- 用户指定光照强度、背景颜色和平面网格颜色都是 per `.scad` file 配置。
- 用户确认配置写入现有 `<stem>.scad.json` 文件。
- 用户要求额外维护默认颜色；实现必须有集中默认值，并且旧配置缺字段时回退到默认值。
- 每个 Phase 必须保护前面 Phase 已达成的目标和边界。
- 每个 Phase 完成编码后必须调用独立 subagent review，review 输入包含本 Phase 目标、完整计划和 diff 或文件清单。

## Phase 执行规则

每个 Phase 都必须按以下循环执行：

1. 干活：只处理当前 Phase 的目标，不顺手重构无关代码。
2. Review：调用独立 subagent 做只读 review，review 输入必须包含当前 Phase 目标与验收标准、完整 `plan-00.md`、本次变更 diff 或涉及文件清单。
3. 回归：按当前 Phase 验收方式运行针对性验证；review 发现 blocker 或 important 时先修复，再重新 review 和回归。
4. 记录：更新 `plan-00-result.md`，记录完成情况、变更摘要、验证结果和遗留问题。
5. 提交：当前 Phase 消除 blocker 且通过对应验证后提交；随后自动进入下一个 Phase，不等待用户确认。

## Phase 1：配置模型与失败测试

### 输入

- `packages/studio-web/src/workbench/preset-io.ts` 现有 `.scad.json` 读写能力。
- `packages/studio-web/src/viewers/viewer-options.ts` 现有 viewer options。
- `packages/studio-web/src/viewers/mesh-three.ts` 当前背景、网格和光照默认值。
- `packages/studio-web/tests/unit/preset-io.test.ts` 与 `packages/studio-web/tests/playwright/canvas-interaction.spec.ts`。

### 前序目标保护

这是第一个 Phase。执行时必须保护现有 presets round-trip、legacy preset 读取、参数预览、相机控制、ViewportGizmo、加载状态和预览请求 dedup 行为。

### 操作步骤

1. 为预览外观定义集中默认配置，至少包含背景颜色、网格主线颜色、网格细线颜色和光照强度。
2. 扩展 `.scad.json` 解析和序列化测试，覆盖：
   - 旧 shared presets shape 缺少外观字段时回退默认外观。
   - 新外观字段与 presets 同时存在时能读出。
   - 写回时保留 presets，并输出外观字段。
   - 非法颜色或非法光照强度不会污染运行时配置。
3. 扩展 viewer options 测试或新增纯函数测试，覆盖默认外观值和范围限制。
4. 扩展 Playwright 测试，先验证右侧栏外观控件和 canvas dataset；修复前测试应失败。

### 验收标准

- 新增或扩展的单元测试在实现前失败，失败原因对应缺失的外观配置能力。
- 新增 Playwright 断言在实现前失败，失败原因对应缺失的右侧栏外观控件或 canvas dataset。
- 现有 presets 解析测试不被删除或弱化。
- 独立 subagent review 无 blocker。

## Phase 2：外观配置读写与右侧栏接线

### 输入

- Phase 1 的失败测试。
- 现有 `ScadWorkbenchState`、`scadInspectorPanelsForState`、`Inspector` slot 机制。
- 现有 `NumericControl` 和 Inspector 分区样式。

### 前序目标保护

- 保护 Phase 1 测试语义，不通过删除断言或降低断言让测试通过。
- 保护 presets round-trip，新增预览外观字段时不能丢失已有 presets。
- 保护 `.scad` 参数更新会触发预览的行为。
- 保护非 `.scad` mesh 直接预览不要求 per file 持久化。

### 操作步骤

1. 在 `.scad.json` 文件模型中加入预览外观配置，并保证 legacy presets 文件仍只作为 presets 兼容来源。
2. 在 `useScadWorkbenchState` 中加载当前 `.scad` 的外观配置；缺文件或缺字段时使用默认值。
3. 在右侧 Inspector 中新增预览外观控制区域，提供背景颜色、网格主线颜色、网格细线颜色和光照强度的实时控制。
4. 调整外观配置时立即更新当前 `MeshViewerOptions`，并以 debounce 写回当前 `.scad.json` 文件。
5. 切换 `.scad` 文件时加载对应文件自己的外观配置，不沿用前一个文件的外观配置。

### 验收标准

- Phase 1 中 `.scad.json` 单元测试通过。
- 右侧栏能看到预览外观控制区域。
- 调整背景颜色、网格颜色和光照强度后，当前 canvas dataset 立即变化。
- 写回 `.scad.json` 后仍保留 presets。
- 切换 `.scad` 文件时外观配置按文件隔离。
- 独立 subagent review 无 blocker。

## Phase 3：Three.js 背景、网格与打光优化

### 输入

- Phase 2 中进入 `MeshViewerOptions` 的外观配置。
- Context7 核对过的 Three.js `AmbientLight`、`HemisphereLight`、`DirectionalLight`、`Scene.background`、`WebGLRenderer.setClearColor` API。
- 当前 `mesh-three.ts` 光照、网格、背景、材质和 dataset 逻辑。

### 前序目标保护

- 保护 Phase 1 和 Phase 2 建立的配置模型、持久化测试和右侧栏交互。
- 保护项目坐标系、相机控制、ViewportGizmo、加载状态、裁切和 orthographic 范围逻辑。
- 保护 vertex color 模式，不因为提亮默认材质而丢失 3MF 颜色。

### 操作步骤

1. 将 Three.js 背景颜色从硬编码改为消费当前外观配置。
2. 将 `GridHelper` 的主线和细线颜色改为消费当前外观配置，并在配置变化时实时更新。
3. 调整默认打光策略为更均匀的组合，使用环境光、半球光和多方向补光减少背面过暗。
4. 将光照强度作为整体倍率应用到光源组合，保持各光源之间比例稳定。
5. 通过 dataset 暴露背景颜色、网格颜色和光照强度，供 Playwright 验证。

### 验收标准

- 单元测试覆盖外观配置 clamp / normalize 行为。
- Playwright 能验证背景颜色、网格颜色和光照强度调整后 dataset 变化。
- 现有 `@canvas-interaction viewer toolbar drives render state`、加载状态、ViewportGizmo 和相机相关测试通过。
- Playwright 截图可用于人工确认模型预览更明亮、网格对比度更合理。
- 独立 subagent review 无 blocker。

## Phase 4：完整回归与结果归档

### 输入

- Phase 1-3 的全部变更和 review 结果。

### 前序目标保护

- 保护 Phase 1 的配置模型和测试覆盖。
- 保护 Phase 2 的 per `.scad` 持久化与文件隔离。
- 保护 Phase 3 的 Three.js 实时渲染效果和 dataset 验证。
- 保护既有参数、presets、相机、ViewportGizmo、加载状态和预览请求 dedup 行为。

### 操作步骤

1. 运行前端 typecheck、相关 unit 测试和相关 Playwright 测试。
2. 生成或检查 Playwright screenshot，确认视觉效果没有明显重叠、空白、过暗或过曝。
3. 检查 `git diff`，确认没有无关文件、生成产物或后端协议变更。
4. 更新 `plan-00-result.md`，记录每个 Phase 的完成情况、验证命令、review 结论和遗留风险。
5. 提交变更。

### 验收标准

- `bun run typecheck` 通过。
- 相关 unit 测试通过。
- 相关 Playwright 测试通过。
- `plan-00-result.md` 完整记录每个 Phase。
- 工作树只包含本计划范围内的变更。
- 独立 subagent 完整 review 无 blocker。

## 执行完成判定

整个计划只有在以下条件全部满足时才算完成：

- 背景颜色、网格主线颜色、网格细线颜色和光照强度都有集中默认值。
- `.scad.json` 能同时保存 presets 和预览外观配置。
- 右侧栏能实时调整上述外观配置。
- 配置按 `.scad` 文件隔离，并能持久化。
- Three.js 预览在默认配置下更明亮、清楚，网格对比度更合理。
- 自动化测试和 Playwright 验证覆盖关键行为。
- 每个 Phase 均完成独立 subagent review，并已写入 `plan-00-result.md`。
