# Web Agent Chat UI 问题修复执行结果

## 当前状态

正在执行计划。Phase 1 已完成 Chat Session 与 History 状态修复，并通过第二轮独立 review；Phase 2 待开始。

## Phase 结果记录

| Phase | 状态 | 结果 |
|---|---|---|
| Phase 1 — Chat Session 与 History 状态修复 | 已完成 | 已修复冷启动首个 Chat history 自动加载、新建 Chat 旧 history 残留、history response 乱序覆盖当前 session 的问题 |
| Phase 2 — Agent 输出时间线、Thinking 与自动滚动 | 待开始 | 待执行 |
| Phase 3 — 文件列表刷新按钮与回归验证 | 待开始 | 待执行 |

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
