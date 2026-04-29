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

## Phase 1 — 替换核心 UI

**状态**：已完成

**变更摘要**：
- 新增 `chat-runtime.tsx`：运行时桥接层，将历史消息 + agent events + token 流转换为 `ThreadMessageLike`，通过 `useExternalStoreRuntime` 接入 assistant-ui
- 重写 `chat-zone.tsx`：使用 `AssistantRuntimeProvider` 包裹 `ChatBody` + `ChatComposer`，保留 `ChatHeader` 和 controller 逻辑
- 重写 `chat-messages.tsx`：使用 `ThreadPrimitive.Viewport` + `ThreadPrimitive.Messages` + `MessagePrimitive.Content`，通过 `data.by_name` 接入 agent event 卡片
- 重写 `chat-composer.tsx`：使用 `ComposerPrimitive.Root` + `ComposerPrimitive.Input`（`submitMode="ctrlEnter"`）+ `ComposerPrimitive.Send`
- 更新 `chat-actions.ts`：`sendChatMessage` 改为接收 `text` 参数，移除 `draft`/`setDraft`
- 更新 `setup.ts`：添加 `ResizeObserver` 和 `scrollTo` polyfill（jsdom 兼容）

**Review 修复**：
- Issue #1：`@assistant-ui/react` 写入 `packages/studio-web/package.json` 依赖
- Issue #2：`planActionDisabled` 通过 React Context 传递到 data 组件
- Issue #3：`OperationSelect` 恢复 `disabled` 属性
- Issue #4：清理 `draft`/`setDraft` 僵尸状态
- Issue #5：`RuntimeMessage` 改为非导出类型
- Issue #6：`ComposerPrimitive.Input` 添加 `aria-label="chat message"`

**验证结果**：
- `npx tsc --noEmit`：chat 文件零新增错误（仅有既有 DOM lib 问题）
- `bun run test:unit`：160/160 测试通过，0 错误

## Phase 2 — 清理与集成

**状态**：已完成

**变更摘要**：
- `AgentEventRow` 属性 `planActionDisabled` 重命名为 `actionDisabled`，消除与 `ChatBodyCtx` 的命名不一致
- 新增 `LlmSetupGuide` 分支测试（`llm_configured: false` → `llm-setup-guide` testid）

**Review 修复**：
- P2 #1：新增 `LlmSetupGuide` empty state 测试覆盖
- P2 #2：`AgentEventRow` 属性重命名为 `actionDisabled`，统一命名

**验证结果**：
- `npx tsc --noEmit`：chat 文件零新增错误
- `bun run test:unit`：161/161 测试通过（含新增 LlmSetupGuide 测试）

**独立 review**：通过，无 P1 问题。6 项验收标准全部 PASS：
1. Timeline dedup：`historyHasRun` 在 history 覆盖当前 run 时跳过 event 转换
2. Auto-scroll：`ThreadPrimitive.Viewport` 的 `autoScroll` prop
3. Empty state：`ThreadPrimitive.If empty` 分支渲染 WelcomeEmptyState / LlmSetupGuide
4. Disabled state：`isDisabled` 通过 runtime 传递到 ComposerPrimitive
5. Context pills：`ContextPillBar` 渲染 + remove 逻辑
6. Operation select：`disabled` prop 完整传递

## Plan 级 Codex Review

**状态**：通过

**审查结论**：全部 7 项审查标准 PASS，无阻塞项。

1. Phase 验收标准覆盖：PASS — 0a 的 run_id 协议/旧 JSONL 兼容/TS 类型同步、0b 的 8 项设计决策验证、1 的 17 项功能清单、2 的 6 项旧功能覆盖，均有代码证据
2. Phase 间行为冲突：PASS — run_id 协议被 Phase 1 正确消费，Phase 2 仅做命名统一未破坏 Phase 1
3. 前序目标保护：PASS — 每个 Phase 均保留了前序 Phase 的核心成果
4. 测试/编译验证：PASS — `cargo test` 全通过、`npx tsc --noEmit` 零错误、161/161 单元测试通过
5. 结果文档准确性：PASS — 变更摘要与实际 diff 一致，验证结果与命令输出一致
6. 用户强制约束遵守：PASS — 未安装 react-ui/react-markdown、合成消息方案、保留 select 切换器、bun 工具链
7. 风险项检查：PASS — Tailwind 风险已解除、useMemo 增量缓存、稳定 id 来自 snapshot、无 run_id 回退基线去重

## 全部 Phase 完成

Phase 0a → 0b → 1 → 2 全部完成，5 轮独立 review（4 轮 Phase review + 1 轮 Plan 级 codex review）均通过。
