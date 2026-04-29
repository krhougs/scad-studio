# Web Agent Operation 下拉框实施计划

## 背景

Web Agent 目前支持 `auto`、`inform`、`plan`、`execute` 四种 operation，但 UI 只暴露普通输入框。`/plan`、`/execute`、`/inform` 作为隐藏 slash command 存在，普通输入默认走 `auto`。用户在 Web UI 中看不到模式入口，导致无法直接理解当前请求会以什么操作级别发送。

## 用户强制约束识别

- operation 选择必须在输入框区域呈现为下拉框。
- 下拉框必须支持 `auto`。
- 不删除现有 slash command 兼容行为。
- 不改变 Plan 卡片确认路径；确认执行仍由 `agent.plan.confirm` 保护。

## Phase 1 — Web 输入区 Operation 下拉框

### 输入

- 当前 Web Agent 输入组件：`packages/studio-web/src/workbench/chat-composer.tsx`
- 当前发送动作：`packages/studio-web/src/workbench/chat-zone.tsx`、`packages/studio-web/src/workbench/chat-actions.ts`
- 当前测试：`packages/studio-web/tests/unit/chat-zone.test.tsx`、`packages/studio-web/tests/unit/chat-actions.test.ts`
- 当前样式：`packages/studio-web/src/styles/workbench-zones.css`

### 前序目标保护

- 保护既有普通输入默认 `auto` 的行为。
- 保护既有 slash command 行为，`/plan`、`/execute`、`/inform` 仍可覆盖本次发送 operation。
- 保护 Plan 卡片确认流，不能把下拉框选择直接替代 `agent.plan.confirm` 的结构化确认。
- 保护上下文 Ref pills 和禁用态逻辑，不能影响 selection context 传递、busy 状态和 running agent 的禁用规则。

### 操作步骤

1. 在 `chat-zone` 单元测试中先新增失败用例：
   - 渲染后能看到 operation 下拉框。
   - 默认值为 `auto`。
   - 选择 `execute` 后发送普通消息，`dispatchAgentInvoke` 收到 `operation: "execute"`。
   - 选择 `plan` 后输入 `/inform explain`，slash command 仍发送 `operation: "inform"`。
2. 修改 `ChatZone` 控制器状态，保存当前下拉框选择的 operation，初始值为 `auto`。
3. 修改 `ChatComposer`，在输入区工具栏增加 operation 下拉框，选项为 `auto`、`inform`、`plan`、`execute`。
4. 修改 `sendChatMessage()` 参数和发送逻辑，让普通输入使用下拉框 operation；slash command 仍优先覆盖本次发送 operation。
5. 必要时补充样式，使下拉框与现有输入工具栏视觉一致，避免挤压发送按钮或上下文 pills。
6. 运行相关 Web 单元测试。
7. 对本 Phase 变更调用独立 subagent review；发现 block 或 important 问题后修复并重新回归。
8. Phase 完成后更新 `plan-00-result.md`，提交本 Phase 变更。

### 验收标准

- Web Agent 输入区有可见 operation 下拉框。
- 下拉框包含 `auto`、`inform`、`plan`、`execute`。
- 默认发送 operation 为 `auto`。
- 用户选择具体 operation 后，普通输入按该 operation 发送。
- Slash command 继续优先于下拉框。
- 相关单元测试通过。
- 独立 review 无 block 问题。

## 执行记录

执行结果实时记录在 `plan-00-result.md`。
