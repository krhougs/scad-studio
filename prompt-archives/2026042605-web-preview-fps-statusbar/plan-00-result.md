# Web Preview FPS Statusbar Result

## 进度

- 已完成。

## Phase 1: 渲染端 FPS 指标

- 已在 Three.js render 路径中计算 FPS。
- FPS 采用相邻两次 render 的时间差计算；用户确认无需节流，因此每次 render 后都会更新。
- FPS 同步写入 canvas `data-render-fps`，便于测试和调试。
- FPS 状态栏文本由 renderer 通过 DOM 标记直接更新，不接入 React state、ref 回调链或 prop 链。

## Phase 2: 状态栏展示

- 已在 3D 状态栏最右侧增加 FPS 单元。
- `CanvasZone` 只提供 `data-canvas-fps-scope` 和 `data-canvas-fps-value` 标记，具体文本由 Three.js viewer 更新。
- FPS 作为正常 flex 项靠右展示，左侧文件名和状态文本具备收缩与省略能力，避免长文本压到 FPS 区域。
- Playwright 状态栏测试已增加真实数字 FPS、canvas dataset、右侧位置和状态文本不重叠断言。

## Phase 3: 验证

- `bun --cwd packages/studio-web typecheck` 通过。
- `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts -g "status bar and chrome do not overlap"` 通过，3 个用例通过。
- `bun --cwd packages/studio-web test:e2e tests/playwright/canvas-interaction.spec.ts` 通过，15 个用例通过。

## Review

- 独立 review 指出状态栏绝对定位存在长文本重叠风险，已改为 flex 靠右排列并补充文本收缩规则。
- 独立 review 指出测试只匹配 `fps` 文本过弱，已改为验证 `\\d+ fps` 与 canvas `data-render-fps`。

## 遗留问题

- 暂无新增遗留问题。
