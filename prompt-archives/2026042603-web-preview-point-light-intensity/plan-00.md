# Web Preview Point Light Intensity Plan

## 背景

web 模型预览已经支持基础光照强度、额外点光源模式、手动点光源位置以及 per `.scad` 文件的外观配置持久化。用户现在要求新增 per model 的点光源强度设置，并持久化。

本计划中的 per model 按当前架构解释为 per `.scad` file：写入对应 `.scad.json` 的 `previewAppearance`，与背景色、网格色、基础光照强度、点光源模式和手动位置保持同一配置入口。

## Phase 1: 数据模型与序列化

### 输入

- `PreviewAppearance` 当前包含背景色、网格色、基础光照强度、点光源模式和点光源位置。
- `.scad.json` 通过 `parsePresetFile` / `stringifyPresetFile` 读写 `previewAppearance`。

### 操作步骤

1. 为 `PreviewAppearance` 增加 `pointLightIntensity`。
2. 默认值沿用当前运行时点光源强度系数 `1.6`，避免已有视觉效果变化。
3. 增加归一化规则，非法值回退默认值，并限制在适合预览交互的范围内。
4. 先写失败单元测试覆盖默认值、读取、写入和非法值归一化，再实现。

### 验收标准

- 旧 `.scad.json` 缺少 `pointLightIntensity` 时自动补默认值。
- 新 `.scad.json` 可读取和写入 `pointLightIntensity`。
- 非法 `pointLightIntensity` 不污染运行时配置。
- 实现 Phase 1 时必须保护：已有背景色、网格色、基础光照强度、点光源模式、点光源位置的读取写入行为不变。

## Phase 2: 运行时渲染与右侧栏控制

### 输入

- 右侧 Appearance 面板已有 `NumericControl`。
- 点光源运行时强度当前由 `lightingIntensity * 1.6` 计算。

### 操作步骤

1. 在 Appearance 面板新增点光源强度数值控件。
2. 将运行时点光源强度改为 `lightingIntensity * pointLightIntensity`。
3. 保留 `pointLightMode=off` 时点光源强度为 0 的行为。
4. 先写失败 Playwright 断言覆盖 UI 修改、canvas dataset 更新、`.scad.json` 持久化，再实现。

### 验收标准

- 修改点光源强度后，canvas 的点光源强度 dataset 实时变化。
- 修改点光源强度后，`.scad.json` 中的 `previewAppearance.pointLightIntensity` 更新。
- 修改点光源强度不会触发 OpenSCAD 重新渲染。
- 切换文件后，每个 `.scad` 文件恢复自己的点光源强度。
- 实现 Phase 2 时必须保护：shadow 强制点光源开启但不修改 `pointLightMode`，manual 位置和 reset 行为不变，mesh 不接收自阴影，shadow 开启时 build plate 仍作为运行时接收面。

## Phase 3: 回归验证与记录

### 输入

- Phase 1 和 Phase 2 的实现。

### 操作步骤

1. 运行相关单元测试。
2. 运行相关 Playwright 测试。
3. 运行 typecheck。
4. 邀请独立 subagent review 当前 diff。
5. 记录执行结果。

### 验收标准

- `bun --cwd packages/studio-web typecheck` 通过。
- `bun --cwd packages/studio-web test:unit -- tests/unit/preset-io.test.ts tests/unit/mesh-render-metrics.test.ts` 通过。
- `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts` 通过。
- review 没有阻塞问题。
- 实现 Phase 3 时必须保护：前两个 Phase 的所有验收标准继续成立。
