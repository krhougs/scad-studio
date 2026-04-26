# Web Preview FPS Statusbar Plan

## 背景

当前 3D 预览状态栏展示文档名、preview 状态、顶点数和索引数，但没有渲染 FPS。用户要求在状态栏最右边展示 FPS。

## Phase 1: 渲染端 FPS 指标

### 输入

- `createMeshViewer` 内部集中调用 `render()`。
- `MeshViewer` 已经向外暴露 mesh info、stats 和 camera change 回调。

### 操作步骤

1. 在 Three.js viewer 的 render 路径中计算 FPS。
2. 通过 canvas dataset 暴露可测试 FPS。
3. 通过同一预览容器内的 DOM 标记更新状态栏 FPS 文本，避免把 FPS 数据接入 React state 或 prop 链。

### 验收标准

- 每次 render 后 canvas 可暴露可测试的 FPS dataset。
- 状态栏 FPS 文本由 renderer 侧 DOM 更新完成，不触发 React 重绘。
- 实现 Phase 1 时必须保护：mesh stats、camera change、preview status、viewer dispose 行为不变。

## Phase 2: 状态栏展示

### 输入

- `CanvasZone` 已经掌握当前 active tab、meshInfo 和 statusbar UI。

### 操作步骤

1. `CanvasZone` 只提供状态栏 FPS 占位和 DOM scope 标记。
2. mesh 和 `.scad` viewer 复用同一 Three.js render 路径更新 FPS。
3. 在状态栏最右侧展示 FPS；3D viewer 初始化时展示 `— fps`。
4. 先写 Playwright 失败断言，再实现。

### 验收标准

- 打开 mesh 或 `.scad` 预览后，状态栏最右侧出现 FPS 单元。
- FPS 单元显示在状态栏其它指标右侧。
- FPS 展示不触发 OpenSCAD 重新渲染，也不写入任何 per file 配置。
- 实现 Phase 2 时必须保护：状态栏布局不与 canvas、toolbar、error card 重叠。

## Phase 3: 验证与记录

### 输入

- Phase 1 和 Phase 2 的实现。

### 操作步骤

1. 运行相关 Playwright 测试。
2. 运行 typecheck。
3. 记录执行结果。

### 验收标准

- `bun --cwd packages/studio-web typecheck` 通过。
- `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts` 通过。
