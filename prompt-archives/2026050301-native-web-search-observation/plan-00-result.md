# Hosted Tool Request Display 执行结果

## 当前状态

- 已根据用户最新目标覆盖 `plan-00.md`。
- 旧 `plan-01.md` 已删除，避免后续执行者走 response stream tap 或 observation 路径。
- Phase 1 到 Phase 5 已完成实现、验证和阶段独立 review；当前等待计划级独立 review 通过后交付。
- 第二轮计划级独立 review 已通过；无阻塞问题。

## 最新事实核查摘要

- 当前目标不是证明 provider 实际调用 hosted tool，而是确认 hosted web search tool 已随 LLM request 发出。
- 用户明确要求升级 Rig，本计划保留 `rig-core 0.36.0` 升级为 Phase 1 验收。
- 当前实现仍使用 `AgentBuilder::additional_params` 配置 hosted web search。
- 当前仓库已经能为 OpenAI Responses 和 Anthropic 构造 hosted web search request。
- OpenAI-compatible Completions 当前不发送 hosted web search，不应显示 `searched`。
- 不应复用 `AgentToolObserver`，因为 host dispatcher 会把它写入 Chat tool history。
- 本计划不需要 response stream tap，也不解析 query、url、source 或 provider response；也不做 HTTP request body/header 检查，只基于同源 hosted tool request 配置。
- 独立 review 指出：如需历史恢复，必须同时扩展 `AgentEventPayload` 和 dispatcher 的 `agent_payload_from_push`。

## Phase 记录

### Phase 1 — Rig 0.36 Upgrade And Hosted Tool Helper

- 状态：已完成。
- 完成时间：2026-05-03 16:23:15 CST。
- 变更摘要：
  - 已将 workspace `rig-core` 依赖和锁文件升级到 `0.36.0`。
  - 新增 `HostedToolRequest` 与 `hosted_tool_requests_for_config`，覆盖 OpenAI Responses、Anthropic、OpenAI-compatible Completions 三类 provider。
  - `rig_agent_additional_params` 已复用 hosted tool helper 生成 provider-native `tools` 字段，保证 request 配置与后续 hosted tool event 使用同一来源。
  - 补充 helper 输出测试，并直接断言 Anthropic additional params 包含 `name = "web_search"`。
- 验证命令：
  - `cargo test -p app-server-core --test llm_tests`：53 passed，0 failed。
  - `cargo test -p app-server-core --test agent_tool_tests`：116 passed，0 failed。
- 独立 review：
  - 结论：未发现进入 Phase 2 前必须修复的阻塞项。
  - 非阻塞意见：`HostedToolRequest` 和 `hosted_tool_requests_for_config` 当前作为 `app-server-core` public API 暴露，主要为集成测试与后续 host callback 复用；后续若确认不需要跨 crate 使用，可再收窄。
- 遗留问题：无阻塞遗留问题。

### Phase 2 — Protocol Event

- 状态：已完成。
- 完成时间：2026-05-03 16:38:35 CST。
- 变更摘要：
  - 新增 `AgentHostedToolActivityStatus::Requested` 与 `AgentHostedToolActivityEvent`，字段包含 `session_id`、`run_id`、`provider_id`、`provider_kind`、`tool_type`、`status`。
  - `ServerPushEvent` 新增 `agent.hosted_tool_activity`，`AgentEventPayload` 新增 `HostedToolActivity` 持久化 payload。
  - Rust re-export、TypeScript mirror、Borsh roundtrip、wire payload contract 测试已更新。
  - `CURRENT_PROTOCOL_VERSION` 已从 13 升到 14。
  - 生成的 protocol WASM 产物已更新。
  - 为消除 Phase 2 引入的下游编译阻塞，`app-server-host` 的 runtime log 对 `HostedToolActivity` 按无状态事件处理，不改变当前文本、reasoning、tool_start 或 tool_result 语义。
- 验证命令：
  - `cargo test -p app-server-protocol`：48 passed，0 failed。
  - `bun run protocol:build`：通过。
  - `bun run protocol:check-generated`：通过。
  - `cargo check -p app-server-host`：通过。
- 独立 review：
  - 第一轮发现 `app-server-host` 对新增 `AgentEventPayload` variant 的穷尽匹配存在编译阻塞。
  - 修复后第二轮 review 结论：未发现阻塞项，`agent_payload_from_push` 映射按计划留到 Phase 3。
- 遗留问题：无阻塞遗留问题。

### Phase 3 — Host Dispatch

- 状态：已完成。
- 完成时间：2026-05-03 17:00:57 CST。
- 变更摘要：
  - `RigAgentCallbacks` 新增 `on_hosted_tool_requested` callback。
  - Agent request 构造在设置 `additional_params` 后、调用 `stream_chat` 前，基于 Phase 1 的 `hosted_tool_requests_for_config` 触发 hosted tool request callback。
  - Host dispatcher 将 callback 映射为 `agent.hosted_tool_activity` push event，并记录 provider、model、tool_type、status 级别的 `info` 日志；未记录 API key、请求头或 request body。
  - `agent_payload_from_push` 已将 hosted tool activity 映射为 `AgentEventPayload::HostedToolActivity`，保证 runtime event log 可恢复。
  - `HostedToolActivity` 在 runtime log 状态机中按无状态事件处理，不改变 token、reasoning、tool_start、tool_result、done、error 行为。
  - 新增 dispatcher roundtrip 测试，验证 OpenAI Responses hosted web search request 会产生实时 push、持久化 Agent event，且不写入 Chat tool history。
- 验证命令：
  - `cargo test -p app-server-core --test llm_tests`：53 passed，0 failed。
  - `cargo test -p app-server-core --test agent_tool_tests`：116 passed，0 failed。
  - `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：59 passed，0 failed。
  - `cargo test -p app-server-core hosted_tool_request_callback`：目标测试通过。
  - `cargo test -p app-server-host hosted_tool_activity_push_maps_to_persisted_payload`：目标测试通过。
- 独立 review：
  - 结论：未发现阻塞项。
  - 非阻塞风险：Anthropic 当前没有 dispatcher 端到端专门测试；现有证据来自 core helper/additional params/callback 测试与 host 通用 callback 映射路径。由于 host 侧不按 provider 分支处理 hosted callback，当前不阻塞 Phase 4。
- 遗留问题：无阻塞遗留问题。

### Phase 4 — Web Display

- 状态：已完成。
- 完成时间：2026-05-03 17:11:57 CST。
- 变更摘要：
  - `studio-common` inbound 现有通配逻辑已能将 `AgentHostedToolActivity` push 放入 `agent_events`，本阶段补充实时 push 测试。
  - `studio-common` 补充 snapshot 中 `AgentEventPayload::HostedToolActivity` 保留到 `agent_event_records` 的测试。
  - Web `protocol-store` 已支持从持久化 `hosted_tool_activity` record 恢复为 `agent.hosted_tool_activity`。
  - Web `chat-runtime` 现有非 token / reasoning event 通路已能将 hosted tool activity 放入消息流，本阶段补充测试。
  - Web `chat-messages` 仅在 `agent.hosted_tool_activity` 且 `tool_type = "web_search"` 且 `status = "requested"` 时显示 `searched`；其他 hosted tool type 不新增 `searched` 文案。
- 验证命令：
  - `cargo test -p studio-common --test managed_client_tests`：33 passed，0 failed。
  - `bun run --cwd packages/studio-web test:unit -- protocol-store.test.ts chat-messages.test.tsx chat-runtime.test.ts`：91 passed，0 failed。
  - `bun run --cwd packages/studio-web test:unit`：306 passed，0 failed。
- 独立 review：
  - 结论：未发现阻塞项。
  - 非阻塞注意：其他 hosted tool type 仍走通用事件详情；当前 web search requested 分支提前返回，只显示 `searched`，不展示 query、url、source 或 provider 原始 JSON。若未来 hosted payload 扩展敏感字段，需要同步收敛通用 fallback。
  - Web 完整单测存在既有 React `act(...)` warning，但测试全部通过。
- 遗留问题：无阻塞遗留问题。

### Phase 5 — Final Verification

- 状态：已完成。
- 完成时间：2026-05-03 17:31:40 CST。
- 变更摘要：
  - 已完成计划要求的 Rust、protocol、Web 单元测试与生成物一致性验证。
  - 已恢复执行过程中由格式化和构建命令触碰的无关文件改动，保留本计划必要的 protocol WASM 生成物更新。
  - 未读取、打印、复制或归档 `agents.toml` 内容。
  - 未将本次验收 prompt 或一次性业务文案写入通用产品代码。
- 验证命令：
  - `cargo test -p app-server-core --test llm_tests`：53 passed，0 failed。
  - `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：59 passed，0 failed。
  - `cargo test -p app-server-protocol`：48 passed，0 failed。
  - `cargo test -p studio-common --test managed_client_tests`：33 passed，0 failed。
  - `bun run protocol:build`：通过。
  - `bun run protocol:check-generated`：通过。
  - `bun run --cwd packages/studio-web test:unit`：306 passed，0 failed。
  - `git diff --check`：通过。
- 遗留问题：
  - `packages/studio-web` 完整单测仍输出既有 React `act(...)` warning；本次没有新增失败，且该 warning 不影响测试通过。
  - 计划级独立 review 指出 Anthropic 缺少 dispatcher 层端到端 roundtrip 专门测试；当前 core helper、additional params、Anthropic callback 路径和 host 通用 callback 映射已覆盖计划语义，因此不阻塞交付。
  - `AGENTS.md` 在本轮开始前已有工作树改动，本计划未修改。

## 计划级独立 Review

- 第一轮结论：不通过。原因是本文件顶部仍保留“尚未执行任何实现 Phase”的过期状态。
- 修复动作：已将当前状态更新为 Phase 1 到 Phase 5 已完成，并重新运行 `git diff --check` 通过。
- 第二轮结论：通过。事件协议、host push 与持久化、`studio-common` 历史恢复、Web `searched` 展示、OpenAI Responses / Anthropic / OpenAI-compatible Completions 行为与结果文档证据均满足计划验收。
