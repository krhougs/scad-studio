# Web Agent Chat UI 问题修复执行结果

## 当前状态

计划已完成。Phase 1 已完成 Chat Session 与 History 状态修复，并通过第二轮独立 review；Phase 2 已完成 Agent 输出时间线、Thinking 与自动滚动修复，并通过第三轮独立 review；Phase 3 已完成文件列表刷新按钮与回归验证，并通过独立 review；plan 级最终独立 review 未发现阻塞问题或高风险问题。

## Phase 结果记录

| Phase | 状态 | 结果 |
|---|---|---|
| Phase 1 — Chat Session 与 History 状态修复 | 已完成 | 已修复冷启动首个 Chat history 自动加载、新建 Chat 旧 history 残留、history response 乱序覆盖当前 session 的问题 |
| Phase 2 — Agent 输出时间线、Thinking 与自动滚动 | 已完成 | 已修复 Agent 实时输出顺序、tool call 后 thinking 动画丢失、done 后最终流式文本丢失、长 token 截断和消息自动滚动问题 |
| Phase 3 — 文件列表刷新按钮与回归验证 | 已完成 | 已增加 Files panel 刷新按钮，点击后复用既有 `workspace.list` 链路刷新 root 与已展开目录，并完成 Web 单元、类型检查和文件监听 Playwright 回归 |

## 执行记录

- 已创建 `plan-prompt.md`，记录用户反馈、当前定位和约束。
- 已创建 `plan-00.md`，按 Chat 状态、Agent 输出时间线、文件列表刷新与最终回归拆分 Phase。

### Phase 1 — Chat Session 与 History 状态修复

- 完成情况：
  - `ManagedClient` 收到 `ChatCreated` 后会切换到新 session、清空旧 `current_chat_history`，避免新建 Chat 继续显示旧记录。
  - Web Chat 在冷启动拿到 session 列表但 snapshot 尚无 `current_chat_session` 时，会自动请求首个 session 的 `chat.history`。
  - `ManagedClient` 记录最新 `chat.history` request id，只允许最新响应更新当前 session/history，避免自动首个 history 请求和用户快速选择 session 之间的乱序覆盖。
- Review：
  - 第一轮 review 发现自动首个 session history 请求存在乱序覆盖用户选择的高风险问题。
  - 已新增反序测试并修复；第二轮 review 未发现阻塞问题或高风险问题。
- 验证：
  - `cargo test -p studio-common --test managed_client_tests stale_chat_history_response_does_not_replace_newer_selection`：修复前失败，修复后通过。
  - `cargo test -p studio-common --test managed_client_tests`：22 个测试通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/chat-zone.test.tsx`：13 个测试通过。
- 遗留问题：
  - Phase 1 未处理 Agent 输出顺序、thinking 动画、最终流式文本和自动滚动；继续由 Phase 2 修复。

### Phase 2 — Agent 输出时间线、Thinking 与自动滚动

- 完成情况：
  - Web Chat 不再使用独立 `streamText` 累加器，而是直接基于当前 session 的 `agent_events` 构建实时输出 timeline。
  - 连续 `agent.token` 会合并为 assistant 文本段；`tool_start`、`tool_result`、`plan_saved`、`error`、`done` 等事件按到达顺序插入 timeline。
  - 当前 run 或最近 run 的全部 agent 事件会被保留，不再按固定数量截断，避免长回答丢失早期 token。
  - 运行中只要 `agent_run` 存在就显示 thinking 动画，tool call 出现后不会隐藏动画。
  - `agent.done` 后不会立即丢弃最终流式文本；只有同一 run 的 history 刷新确实增加了 assistant 消息，并且最新 assistant 内容覆盖完整 token 文本时，才隐藏 live token 段。
  - Chat body 增加滚动锚点，在消息、实时事件或 streaming 状态变化时自动滚动到最新内容。
  - 长 tool result 和长消息增加换行约束，避免内容挤破布局。
- Review：
  - 第一轮 review 发现固定截断当前 run 事件会导致长回答丢失早期 token。
  - 第二轮 review 发现仅按历史 assistant 文本匹配会在 `agent.done` 后、history 刷新前误隐藏当前 token。
  - 已分别补充失败测试并修复；第三轮 review 未发现阻塞问题或高风险问题。
- 验证：
  - `cd packages/studio-web && bun run test:unit -- tests/unit/chat-zone.test.tsx -t "keeps every live token|does not hide current live tokens"`：修复前失败，修复后通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/chat-zone.test.tsx -t "keeps done text visible|does not hide current live tokens|keeps streamed text visible"`：修复前失败，修复后通过。
  - `cd packages/studio-web && bun run test:unit -- tests/unit/chat-zone.test.tsx tests/unit/chat-messages.test.ts tests/unit/markdown-preview-security.test.ts`：35 个测试通过。
  - `cd packages/studio-web && bun run typecheck`：通过。
- 遗留问题：
  - Phase 2 未处理文件列表刷新按钮；继续由 Phase 3 修复。

### Phase 3 — 文件列表刷新按钮与回归验证

- 完成情况：
  - Files panel header 增加图标刷新按钮，可访问名称为 `refresh files`。
  - 刷新按钮通过 `LeftPanel` 接到 `WorkbenchLayout` 的 `handleRefreshFiles()`。
  - `handleRefreshFiles()` 仅复用既有 `refreshRootListing(client)` 与 `refreshExpandedDirectories(client)`，最终仍通过 app server protocol 的 `workspace.list` 请求刷新 root 与已展开目录。
  - `SidePanelHeader` 增加可选 actions 区域，默认不影响其它 panel。
  - `docs/known_issues.md` 已记录“Web 文件列表缺少手动刷新入口”为已处理历史问题。
- Review：
  - Phase 3 独立 review 未发现阻塞问题。
  - Review 提醒：当前新增单元测试验证 Files panel 按钮调用回调，未直接在 Workbench 层断言点击真实按钮会同时触发 root 与已展开目录 `workspace.list` 请求。代码审查确认当前接线满足要求；该项作为非阻塞测试覆盖风险记录。
- 验证：
  - `cd packages/studio-web && bun run test:unit -- tests/unit/workspace-tree.test.tsx -t "refresh button"`：修复前失败，修复后通过。
  - `cd packages/studio-web && bun run test:unit`：30 个测试文件、160 个测试通过。
  - `cd packages/studio-web && bun run typecheck`：通过。
  - `cd packages/studio-web && bun run test:e2e -- tests/playwright/browser-watch-smoke.spec.ts -g "watch push triggers Files tab"`：1 个 Playwright 用例通过。
  - `git diff --check`：通过。
- 遗留问题：
  - 未发现阻塞交付的问题。
  - 可选后续增强：为 Workbench 层补一个更直接的手动刷新接线测试，减少未来 `LeftPanel` / `WorkbenchLayout` prop 接线变化造成的回归风险。

### Plan 级最终独立 review

- Review 结论：
  - 未发现阻塞问题或高风险问题，当前状态可进入最终提交。
  - Phase 1、Phase 2、Phase 3 均满足原计划验收标准。
  - Phase 之间未发现行为冲突或重复实现；Phase 2/3 未破坏 Phase 1 的 Chat session/history 边界，Phase 3 未破坏 Phase 2 的 Agent timeline、thinking、done token 与自动滚动边界。
- 剩余风险：
  - 手动刷新链路缺少 Workbench 层直接测试；现有测试覆盖 Files panel 按钮与回调，代码审查确认 Workbench 当前接线正确。该风险为非阻塞后续增强项。
