# Web Agent Chat UI 问题修复计划

## 背景

上一轮 Agent / Plan 工作区计划流已经完成后，Web Agent Chat 仍存在明显的交互回归：session 冷启动加载不完整、新建 Chat 后沿用旧 history、Agent 实时输出无法按事件顺序展示、done 事件会清空最终流式文本、消息区域不会自动滚动、文件树缺少手动刷新入口。

这些问题集中在 Web Chat UI、managed client snapshot 更新和文件树刷新入口，不需要改变 protocol、Agent / Plan 后端执行模型或 CadQuery 工具边界。

## 用户强制约束识别

- 冷启动时 Chat 列表和当前 Chat history 必须正确加载。
- 新建 Chat 后不能继续显示旧 Chat 的消息记录。
- Agent 工作时，文字输出、思考状态和 tool call 必须按收到顺序展示。
- tool call 或 thinking 出现后，运行中动画不能消失；最终文字不能被丢弃。
- Chat 消息区域必须自动滚动到最新内容。
- 文件列表需要可点击刷新按钮。

## Phase 1 — Chat Session 与 History 状态修复

### 输入

- `crates/studio-common/src/managed_client/inbound.rs`
- `crates/studio-common/tests/managed_client_tests.rs`
- `packages/studio-web/src/workbench/chat-zone.tsx`
- `packages/studio-web/tests/unit/chat-zone.test.tsx`

### 前序目标保护

- 保护现有 Chat session 列表、session 选择、发送消息和 Agent busy 禁用行为。
- 保护 `chat.list` / `chat.history` 均通过 app server protocol 触发。
- 保护现有 selection context 和 `agent.invoke` 参数。

### 操作步骤

1. 先新增失败测试：`ChatCreated` 更新 snapshot 时应切换到新 session 并清空旧 history。
2. 先新增失败测试：Web 冷启动拿到 session 列表但没有 current session 时，应自动请求首个 session 的 history。
3. 修复 managed client 的 `ChatCreated` snapshot 更新，避免新 Chat 继承旧 history。
4. 修复 Web Chat 初始化逻辑，在 session 列表可用但 current session 缺失时请求首个 session history。
5. 运行 Rust / Web 聚焦测试。
6. 启动独立 review，修复阻塞项后再进入下一 Phase。
7. 更新 `plan-00-result.md` 并提交 Phase 1。

### 验收标准

- 冷启动能加载 Chat 列表和首个 Chat history。
- 新建 Chat 后旧消息不会继续显示在新 Chat 上。
- 现有发送消息、选择 session、Agent busy 禁用测试仍通过。

## Phase 2 — Agent 输出时间线、Thinking 与自动滚动

### 输入

- `packages/studio-web/src/workbench/chat-messages.tsx`
- `packages/studio-web/src/workbench/chat-zone.tsx`
- `packages/studio-web/src/styles/workbench-zones.css`
- `packages/studio-web/tests/unit/chat-zone.test.tsx`
- `packages/studio-web/tests/unit/chat-messages.test.ts`

### 前序目标保护

- 保护 Markdown 输出仍使用 `rehypeSanitize`。
- 保护 Plan Package card 的 `Open Plan` / `Run Plan` 行为。
- 保护 Agent event 只展示当前 session 的事件。
- 保护 Agent / Plan 双模式和 `plan_ref` 请求不变。

### 操作步骤

1. 先新增失败测试：token、tool_start、tool_result、后续 token 应按事件顺序渲染。
2. 先新增失败测试：`agent.done` 到 history 刷新完成前不应丢弃流式文本。
3. 先新增失败测试：running 状态下即使已有 tool event，也应显示 thinking 动画。
4. 先新增失败测试：消息或实时事件更新后 `.chat-body` 会滚动到底部。
5. 将实时 Agent 输出改为按 `agent_events` 顺序构建 timeline：连续 token 合并为 assistant 文本段，tool / plan / error / done 按原位置插入。
6. 停止在 `agent.done` 事件立即清空流式文本；只有 session 切换、新 run 开始或历史消息已覆盖流式文本时才隐藏实时文本段。
7. 给 Chat body 增加滚动锚点或 ref，按消息 / 事件 / streaming 更新自动滚动。
8. 补充必要样式，保证长 tool result 不挤破布局。
9. 运行 Web 聚焦测试。
10. 启动独立 review，修复阻塞项后进入下一 Phase。
11. 更新 `plan-00-result.md` 并提交 Phase 2。

### 验收标准

- Agent 输出按事件到达顺序展示。
- 运行中 tool call 后仍有 thinking 动画。
- `agent.done` 后最终流式文字不会在 history 刷新前消失。
- Chat body 自动滚动到最新消息。
- Markdown sanitize、Plan Package card 和 Run Plan 行为不回退。

## Phase 3 — 文件列表刷新按钮与回归验证

### 输入

- `packages/studio-web/src/workbench/files-panel.tsx`
- `packages/studio-web/src/workbench/left-panel.tsx`
- `packages/studio-web/src/workbench/workbench-layout.tsx`
- `packages/studio-web/src/workbench/side-panel-header.tsx`
- `packages/studio-web/tests/unit/workspace-tree.test.tsx`
- Web Playwright Chat / watch / layout 相关测试

### 前序目标保护

- 保护文件树展开 / 折叠 / 打开文件行为。
- 保护 watch event 自动刷新行为。
- 手动刷新只能调用已有 app server protocol 的 workspace list 请求，不直接读本地文件系统。

### 操作步骤

1. 先新增失败测试：Files panel 显示刷新按钮，点击后调用刷新回调。
2. 扩展 header 或 Files panel 局部 UI，增加图标按钮和可访问名称。
3. 将 Workbench 现有 root listing 与 expanded directories 刷新函数接到 Files panel。
4. 运行 Web 单元测试、Chat Playwright、watch 或文件树相关 Playwright 回归。
5. 运行 `git diff --check` 和必要的格式 / 类型检查。
6. 启动独立 review，修复阻塞项。
7. 更新 `docs/known_issues.md`：如本轮确认并修复已知刷新缺口，更新对应状态；如发现新非阻塞缺口，新增记录。
8. 更新 `plan-00-result.md`，执行 Plan 级独立 review，通过后提交最终归档。

### 验收标准

- 文件列表有明确刷新按钮。
- 点击刷新会重新请求 root 和已展开目录。
- Chat 五个用户反馈问题均有测试覆盖或回归验证。
- 最终工作树不包含测试生成的临时文件。
