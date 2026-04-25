# scad preview request dedup result

## 当前状态

计划已执行完成，正在做最终 review 反馈修正后的收敛验证。

## Phase 1: 固化复现与回归测试

- 状态：已完成
- 完成情况：新增 Playwright 回归测试，能够在修复前稳定暴露 `.scad` 初次打开时重复发送等价 `preview.request`。
- 变更摘要：
  - 新增 `packages/studio-web/tests/playwright/preview-request-dedup.spec.ts`。
  - 测试启动独立 harness，注入双向 WebSocket recorder，解码 outgoing client frame 和 incoming server frame。
  - 测试按 decoded `source + defines + configured_openscad_path` 构造重复请求 key，并在首次 preview response 后等待稳定窗口再统计。
- 验证命令：
  - `bun run --cwd packages/studio-web test:e2e preview-request-dedup.spec.ts`
  - 结果：按预期失败。失败信息显示重复 key 为 `examples/cube.scad + [] + null`，request id 为 `[10, 12]`，时间差约 `255ms`。
- Review：通过。独立 reviewer 确认测试基于 decoded WS frame 与 request id，不依赖 UI 文案；等待逻辑覆盖 250ms debounce 后的第二次请求；recorder 隔离在本 spec 内；失败信息满足证据要求。
- 遗留问题：无。该测试在 Phase 2 和 Phase 3 修复后已转绿。

## Phase 2: 收敛 `.scad` appliedDefines 等价更新

- 状态：已完成
- 完成情况：`.scad` 初次打开时不再因等价 `appliedDefines` 数组替换触发第二个等价 `preview.request`；真实参数变化仍触发新的 `preview.request`。
- 变更摘要：
  - `packages/studio-web/src/workbench/scad-workbench.tsx` 新增 `applyDefines` guarded setter，按字符串数组内容比较，内容相同时保留旧数组引用。
  - 源码解析、参数 debounce、恢复默认、预设加载、路径切换清空等 `appliedDefines` 写入点统一接入 guarded setter。
  - `packages/studio-web/tests/playwright/preview-request-dedup.spec.ts` 增加参数变化正向验证，确认修改 `params-cube.scad` 参数后产生新的 request id 且 decoded defines 变化。
- 验证命令：
  - `bun run --cwd packages/studio-web test:e2e preview-request-dedup.spec.ts`
  - 结果：2 passed。初次打开去重和参数变化正向验证均通过。
- Review：通过。独立 reviewer 确认 `applyDefines` 只按顺序比较字符串数组内容，真实参数变化不会被吞掉；`setAppliedDefines` 只剩 guarded setter 内部调用；hook 依赖合理；测试覆盖初次打开去重和参数变化正向路径。
- 遗留问题：参数 debounce 在等价更新被跳过后仍会记录一条 `parameters preview update` 日志，这是既有语义延续，不影响本轮重复请求修复。

## Phase 3: 评估并补齐预览请求层幂等保护

- 状态：已完成
- 完成情况：未新增通用 `MeshViewer` 请求层幂等保护；改为修正 `.scad` refresh 双入口。Phase 3 review 指出同一个 `refreshSignal` 会同时驱动源码重读和 `MeshViewer` 直接请求，已补充测试并修复。
- 变更摘要：
  - `packages/studio-web/tests/playwright/preview-request-dedup.spec.ts` 改为使用临时 workspace，并新增 `.scad` 文件刷新回归测试。
  - 修复前 refresh 测试按预期失败，重复 key 为 `examples/cube.scad + [] + null`，request id 为 `[14, 18]`，时间差约 `64ms`。
  - `packages/studio-web/src/workbench/scad-workbench.tsx` 不再把 `refreshSignal` 直接传给 `ScadPreviewViewer`；`.scad` refresh 只通过 `useScadWorkbenchState` 重读源码，然后由 `sourceReady` / defines 状态驱动预览。
  - 保留真实参数变化与文件 refresh 触发新预览的能力，避免用全局节流或请求层防御掩盖状态双入口。
- 验证命令：
  - `bun run --cwd packages/studio-web test:e2e preview-request-dedup.spec.ts`
  - 结果：3 passed，覆盖初次打开、参数变化、`.scad` 文件 refresh。
  - `bun run --cwd packages/studio-web typecheck`
  - 结果：通过。
- Review：通过。首次 review 指出 `.scad` refresh 双入口风险；已补充 refresh 红灯测试并修复。复审确认 `refreshSignal` 不再直达 `ScadPreviewViewer`，`.scad` refresh 只剩源码重读入口，不需要新增通用 `MeshViewer` 请求层幂等保护。最终 review 要求 refresh 测试显式断言刷新后恰好一次 `cube.scad` 请求，已补充该断言。
- 遗留问题：无

## Phase 4: 全量回归与结果归档

- 状态：已完成验证，正在复核最终 review 反馈
- 完成情况：完成最终回归、diff 范围检查与结果归档。
- 变更摘要：
  - 代码变更范围仅包含 `packages/studio-web/src/workbench/scad-workbench.tsx`、`packages/studio-web/tests/playwright/preview-request-dedup.spec.ts`、本 result 文档。
  - 后端 protocol、transport、app-server-host 未改动。
  - `.scad` 初次打开、参数变化、文件 refresh 三条路径均通过 decoded WebSocket frame 验证。
- 验证命令：
  - `bun run --cwd packages/studio-web test:e2e preview-request-dedup.spec.ts`
    - 结果：3 passed。
  - `bun run --cwd packages/studio-web typecheck`
    - 结果：通过。
  - `bun run --cwd packages/studio-web test:e2e parameters-presets.spec.ts`
    - 结果：7 passed。
  - `bun run --cwd packages/studio-web test:e2e browser-watch-smoke.spec.ts`
    - 结果：6 passed。
  - `git diff --check 38cc7b4..HEAD`
    - 结果：通过。
  - `git diff --stat 38cc7b4..HEAD && git diff --name-only 38cc7b4..HEAD`
    - 结果：3 个文件变更，范围符合计划。
- Review：最终 review 发现两项需小改：refresh 测试缺少“恰好一次 `cube.scad` 请求”断言，result 文档状态不一致。两项均已修正，修正后重新运行验证。
- 遗留问题：无
