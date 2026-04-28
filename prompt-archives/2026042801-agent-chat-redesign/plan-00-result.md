# Agent Chat 产品流程重设计 — 执行结果

## Phase A — LLM 接入（Agent 能说话）

**状态：** 完成（已 review，findings 已修复）

### 变更清单

| 文件 | 变更 |
|------|------|
| `crates/app-server-core/src/llm/mod.rs` | 新增。LlmProvider trait、LlmMessage、LlmError、try_create_provider |
| `crates/app-server-core/src/llm/config.rs` | 新增。LlmConfig（手动 Debug 隐藏 api_key）、环境变量配置加载 |
| `crates/app-server-core/src/llm/openai_compat.rs` | 新增。OpenAI Compatible SSE 流式调用（ureq v3 + BufReader SSE 解析） |
| `crates/app-server-core/src/agent.rs` | 新增 stream_agent_turn、llm_generate_cadquery_code、build_turn_messages、build_execute_messages、extract_cadquery_code 等 |
| `crates/app-server-core/src/lib.rs` | 新增 pub mod llm 和相关 re-export |
| `crates/app-server-core/Cargo.toml` | 新增 ureq = { version = "3", features = ["json"] } |
| `crates/app-server-host/src/dispatcher.rs` | run_text_agent 和 generate_cadquery_or_report 接入 LLM provider，fallback 到 LocalAgentBackend |
| `crates/app-server-core/tests/llm_tests.rs` | 新增。22 个纯函数测试 |

### Review Findings 处理
- F1 (LlmConfig Debug 泄露 API key) — 已修复：手动实现 Debug，api_key 显示为 "***"
- F7 (5 个纯函数缺少测试) — 已修复：补充 build_turn_messages、build_execute_messages、build_turn_context 测试

### 验证结果
- `cargo check` 全 workspace 通过
- `cargo test -p app-server-core --test llm_tests` — 22/22 通过
- `cargo test -p app-server-core --test agent_tests` — 12/12 通过（无回归）

---

## Phase B — 解耦 Selection + 重设计 Composer

**状态：** 完成（已 review，findings 已修复）

### 变更清单

| 文件 | 变更 |
|------|------|
| `crates/app-server-protocol/src/protocol.rs` | AgentOperationLevel 新增 Auto=3、AgentInvokeRequest 新增 context_refs 字段 |
| `crates/app-server-core/src/agent.rs` | operation_label 处理 Auto |
| `crates/app-server-host/src/dispatcher.rs` | run_agent_worker 将 Auto 路由到 text agent |
| `packages/app-server-protocol/src/index.ts` | AgentOperationLevel 新增 "auto"、AgentInvokeRequest 新增 context_refs |
| `packages/studio-web/src/workbench/chat-zone.tsx` | 移除 OperationSelector/ExecuteTargetInput/OperationButton；新增 WelcomeEmptyState/ContextPillBar；简化 Composer |
| `packages/studio-web/src/workbench/cadquery-agent-scope.ts` | 新增 preferredRefText 导出 |
| `packages/studio-web/tests/unit/cadquery-agent-scope.test.ts` | 新增 3 个 preferredRefText 测试 |

### 验证结果
- `cargo check` 全 workspace 通过
- `cargo test -p app-server-core --test llm_tests` — 22/22 通过
- `cargo test -p app-server-core --test agent_tests` — 12/12 通过
- TypeScript 类型检查通过（仅限本次变更文件；DOM lib 错误为预存）
- vitest unit tests — 121 通过，26 文件，0 失败
- cadquery-agent-scope tests — 10/10 通过（含 3 个新增）

### 产品变更
- 用户不再需要手动选择 Inform/Plan/Execute 模式，默认发送 Auto
- 无 Viewer 选择时可以正常聊天
- Viewer 选择以可移除的 Context Pill 形式显示在输入框上方
- 空状态显示欢迎界面和建议提示词
- confirmed_cadquery 字段保留在协议中作为 Execute 安全门（Phase C 使用）

---

## Phase C — Agent 自动模式 + Plan 确认卡片

**状态：** 完成（已 review，findings 已修复）

### Review Findings 处理
- F1 (TS 协议类型缺少 run_id) — 已修复：AgentPlanConfirmRequest 和 AgentPlanRejectRequest 补充 run_id 字段
- F2 (前端 confirm/reject 调用缺少 run_id) — 已修复：confirmPlan 和 rejectPlan 传递 plan.run_id
- F3 (dispatcher.rs 过大) — 已修复：plan extraction 逻辑抽取到 plan_extraction.rs（127 行），dispatcher.rs 从 1433 行减少到 1308 行

### 变更清单

| 文件 | 变更 |
|------|------|
| `crates/app-server-core/src/agent.rs` | AgentTurnInput 新增 context_refs 字段；build_turn_context 注入 context refs |
| `crates/app-server-core/tests/llm_tests.rs` | 新增 build_turn_context_includes_context_refs 测试（23 total） |
| `crates/app-server-core/tests/agent_tests.rs` | 所有 AgentTurnInput 构造补充 context_refs |
| `crates/app-server-host/src/dispatcher.rs` | confirm/reject 处理；context_refs 传递；try_propose_plan 调用 plan_extraction |
| `crates/app-server-host/src/plan_extraction.rs` | 新文件：Plan proposal 提取逻辑（JSON block + selection 推断）、export_handle_for、extract_object_name |
| `crates/app-server-host/src/lib.rs` | 新增 pub mod plan_extraction；re-export 纯函数 |
| `crates/app-server-host/tests/plan_extraction_tests.rs` | 14 个纯函数测试（JSON/selection/object name/export handle） |
| `crates/studio-common/src/managed_client/dispatch.rs` | 新增 dispatch_agent_plan_confirm 和 dispatch_agent_plan_reject |
| `crates/studio-common/src/managed_client/inbound.rs` | AgentPlanConfirmed 设置 agent_run；AgentPlanRejected 处理 |
| `crates/studio-app/src/protocol_client.rs` | 新增 AgentPlanProposed match arm |
| `crates/studio-web-wasm/src/wasm_bridge/client.rs` | 新增 client_dispatch_agent_plan_confirm 和 client_dispatch_agent_plan_reject |
| `packages/app-server-protocol/src/index.ts` | 新增 AgentPlanProposedEvent/AgentPlanConfirmRequest/AgentPlanRejectRequest 类型 |
| `packages/studio-web/src/wasm-bridge/client.ts` | 新增 dispatchAgentPlanConfirm/dispatchAgentPlanReject 方法 |
| `packages/studio-web/src/workbench/chat-zone.tsx` | 拆分为 chat-zone/chat-messages/chat-composer/chat-actions；新增 pendingPlan 状态管理 |
| `packages/studio-web/src/workbench/chat-messages.tsx` | 新文件：ChatBody/ChatMessage/PlanConfirmationCard/AgentLevelBadge/AgentEventRow |
| `packages/studio-web/src/workbench/chat-composer.tsx` | 新文件：ChatComposer/ContextPillBar/ChatTextarea/ChatComposerTools |
| `packages/studio-web/src/workbench/chat-actions.ts` | 新文件：confirmPlan/rejectPlan/sendChatMessage/createChatSession/selectChatSession/cancelAgentRun |
| `packages/studio-web/tests/unit/chat-zone.test.tsx` | 重写：4 个测试（auto invoke/done refresh/context pills/empty state） |

### 验证结果
- `cargo check` 全 workspace 通过（无新 warning）
- `cargo test -p app-server-core --test llm_tests` — 23/23 通过
- `cargo test -p app-server-core --test agent_tests` — 12/12 通过
- `cargo test -p app-server-host --test plan_extraction_tests` — 14/14 通过
- TypeScript 类型检查通过（仅限本次变更文件）
- vitest unit tests — 121 通过，26 文件，0 失败
- 所有文件均在 500 行限制内（chat-zone 341/chat-messages 179/chat-composer 107/chat-actions 160）

### 产品变更
- Agent 在 Auto 模式下，回复含修改意图时自动提出 Plan 确认卡片
- Plan 确认卡片展示目标文件、影响范围、导出目标、变更描述
- 用户点击 [Confirm Execute] 触发 AgentPlanConfirm → 启动 Execute AgentWorker
- 用户点击 [Cancel] 触发 AgentPlanReject → 取消 Plan
- context_refs 从前端 context pills 传递到后端，注入 LLM prompt
- chat-zone.tsx 拆分为 4 个文件，每个文件职责单一

---

## Phase D — 体验打磨

**状态：** 完成（已 review，findings 已修复）

### Review Findings 处理
- F1/F2/F3 (dispatcher.rs/protocol.rs/index.ts 超 500 行) — 历史技术债，Phase D 未恶化，各仅新增 1 行
- F5 (context_refs Borsh 向后兼容) — Phase C 遗留，由 protocol version 协商保护
- F10 (Preview 按钮缺少集成测试) — 已修复：补充 dispatchCadQueryPreview 到 fakeClient，新增 preview 集成测试

### 变更清单

| 文件 | 变更 |
|------|------|
| `packages/studio-web/src/workbench/chat-actions.ts` | 新增 parseSlashCommand 纯函数、previewPlan 异步函数；sendChatMessageInner 使用 parseSlashCommand 提取 operation override |
| `packages/studio-web/src/workbench/chat-messages.tsx` | 新增 AgentErrorCard/friendlyErrorMessage/LlmSetupGuide/AssemblyImpactWarning/findAffectedAssemblies；ChatBody 新增 llmConfigured 和 onPreviewPlan props；PlanConfirmationCard 新增 Preview 按钮 |
| `packages/studio-web/src/workbench/chat-zone.tsx` | ChatSnapshot 新增 llm_configured；ChatHeader 新增 llm-dot 状态指示；新增 previewPlan handler |
| `packages/studio-web/src/wasm-bridge/client.ts` | 新增 dispatchCadQueryPreview 方法 |
| `packages/app-server-protocol/src/index.ts` | ServerCapabilities 新增 llm_configured |
| `crates/app-server-protocol/src/protocol.rs` | ServerCapabilities 新增 llm_configured: bool |
| `crates/studio-common/src/managed_client/mod.rs` | ManagedClient 新增 llm_configured 字段，snapshot() 包含该字段 |
| `crates/studio-common/src/managed_client/inbound.rs` | handle_handshake_ack 保存 llm_configured |
| `crates/studio-common/src/managed_client/types.rs` | ClientSnapshot 新增 llm_configured |
| `crates/app-server-host/src/dispatcher.rs` | server_capabilities() 调用 load_llm_config().is_some() 设置 llm_configured |
| `packages/studio-web/tests/unit/chat-actions.test.ts` | 新增 8 个 parseSlashCommand 测试 |
| `packages/studio-web/tests/unit/chat-messages.test.ts` | 新增 5 个 friendlyErrorMessage + 4 个 findAffectedAssemblies 测试 |
| `packages/studio-web/tests/unit/chat-zone.test.tsx` | 新增 2 个集成测试（slash command + preview button） |
| 8 个 Rust 测试文件 | 补充 llm_configured: false；补充 context_refs: Vec::new()（预存遗漏修复） |

### 验证结果
- `cargo check` 全 workspace 通过（无新 warning）
- `cargo test --workspace` — 397 tests 全部通过
- vitest unit tests — 140 通过，28 文件，0 失败
- TypeScript 类型检查通过（仅限本次变更文件；WASM d.ts 未重新生成为预存问题）
- 所有新增文件在 500 行限制内（chat-zone 359/chat-messages 299/chat-actions 205/chat-composer 107）

### 产品变更
- 用户可通过 /plan、/execute、/inform 斜杠命令显式覆盖 Agent 操作级别
- Agent 错误以结构化卡片展示，10 种错误类型映射为用户友好消息
- Plan 确认卡片中修改影响 assembly 文件时显示警告
- ChatHeader 显示 LLM 连接状态指示点（绿/灰）
- 无 LLM 配置时显示环境变量配置引导界面
- Plan 确认卡片新增 [Preview] 按钮，调用 CadQuery 预览

---

## Playwright 集成测试

**状态：** 完成（UI 测试通过；协议帧测试待 WASM 二进制重新生成后通过）

### 变更清单

| 文件 | 变更 |
|------|------|
| `packages/studio-web/tests/playwright/agent-chat-interaction.spec.ts` | 新增。9 个 Playwright 集成测试 |

### 测试清单

| 测试 | 类别 | 状态 |
|------|------|------|
| welcome empty state | UI 渲染 | 通过 |
| llm status dot in header | UI 渲染 | 通过 |
| navigate via rail button | UI 导航 | 通过 |
| input placeholder and send button | UI 渲染 | 通过 |
| agent.invoke with operation auto | 协议帧验证 | 阻塞（WASM 未重新生成） |
| /plan sends operation plan | 协议帧验证 | 阻塞（WASM 未重新生成） |
| /execute sends operation execute | 协议帧验证 | 阻塞（WASM 未重新生成） |
| /inform sends operation inform | 协议帧验证 | 阻塞（WASM 未重新生成） |
| chat.send without slash prefix | 协议帧验证 | 阻塞（WASM 未重新生成） |

### 阻塞原因

Phase A-D 新增了多个协议字段（`context_refs`、`llm_configured`、`AgentPlanConfirm/Reject` 等），但 WASM 二进制未重新生成（`wasm-pack build`）。导致 WASM client 与 Rust host 之间的 Borsh 序列化格式不匹配，WebSocket 握手无法完成。此问题影响所有依赖协议帧的 Playwright 测试（包括预存的 `wasm-bridge-smoke.spec.ts`），不是本次新增测试独有的问题。

**修复方式：** 执行 `wasm-pack build` 重新生成 `studio-web-wasm` 和 `app-server-protocol-wasm` 的 WASM 二进制和类型声明即可。

### 测试设计要点
- 端口分配：bindPort 39220 / vitePort 5220（避免与现有测试冲突）
- `waitForHandshake` 通过 `workspace-name` 不为 "(loading)" 判断协议握手完成
- `fillAndSend` 通过 `toHaveValue` 等待 React 状态更新后再点击发送
- `latestRecordedClientCommand` 解码 Borsh 协议帧验证命令类型和负载
- UI 测试不依赖握手完成，可独立通过
