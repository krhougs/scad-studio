# scad preview request dedup result

## 当前状态

计划已创建，尚未执行。

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
- 遗留问题：该测试当前红灯，需 Phase 2 修复生产代码后转绿。

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

- 状态：未开始
- 完成情况：无
- 变更摘要：无
- 验证命令：未运行
- Review：未执行
- 遗留问题：无

## Phase 4: 全量回归与结果归档

- 状态：未开始
- 完成情况：无
- 变更摘要：无
- 验证命令：未运行
- Review：未执行
- 遗留问题：无
