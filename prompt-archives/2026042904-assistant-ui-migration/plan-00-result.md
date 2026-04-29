# 执行结果

计划已通过 codex review（5 轮收敛），执行中。

## Plan 状态

- `plan-00.md`：最终版本，通过全部 7 条审查标准
- `plan-prompt.md`：已同步用户约束修正

## Phase 0a — run_id 协议迁移（前置依赖）

**状态**：已完成

**变更摘要**：
- `ChatMessageRecord`（protocol.rs）增加 `run_id: Option<String>`，含 `#[serde(default)]`
- `JsonlMessage`（chat.rs）增加 `#[serde(default)] run_id: Option<String>`，新增 `with_run_id` builder
- `ChatStore` 新增 `append_message_with_run_id`、`append_tool_call_with_run_id`、`append_tool_result_with_run_id` 方法，原有方法委托到新方法保持向后兼容
- `dispatcher.rs` 中 `append_agent_message` 改用 `append_message_with_run_id` 传入 run_id（仅最终 assistant 回答写入）
- TS 类型同步更新（app-server-protocol/index.ts、chat-zone.tsx）
- 新增 2 个测试：旧 JSONL 兼容性、run_id 往返；补充 Borsh None roundtrip 测试

**验证结果**：
- `cargo test --workspace --exclude studio-app`：全通过（studio-app 有既有编译错误，非本次改动引入）
- `wasm-pack build`：通过
- `npx tsc --noEmit`：通过

**独立 review**：通过，无 P1 问题。P2/P3 已修复（Borsh None 测试、serde default）。

## Phase 0b — API 验证与决策确认

**状态**：已完成

**变更摘要**：
- 安装 `@assistant-ui/react@0.12.27`
- 逐项验证全部 8 项已锁定设计决策，均标注「通过」
- 关键发现：@assistant-ui/react 不依赖 Tailwind（Radix UI + zustand，无样式原语）
- 验证结果文档：`phase-0b-verification.md`

**验证结论**：
- External store 运行时模式确认可用（`useExternalStoreRuntime`）
- 消息不合并行为确认（`ThreadPrimitive.Messages` 逐条独立渲染）
- 自定义渲染插槽确认可用（`MessagePrimitive.Parts` 支持 `part.type` 分支）
- Composer 支持 `submitMode="ctrlEnter"` 确认 Cmd/Ctrl+Enter 发送
- Tailwind 风险已解除

## 待执行 Phase

3. Phase 1 — 替换核心 UI
4. Phase 2 — 清理与集成
