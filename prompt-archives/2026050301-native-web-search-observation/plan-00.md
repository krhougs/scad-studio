# Hosted Tool Request Display 实施计划

> 执行者要求：执行本计划前必须通读 `plan-prompt.md` 与本文件。当前用户要求先写计划、不要开工；因此本文件只定义后续执行步骤，不代表已经开始实现。

## 目标

当本轮 Agent 已把 provider-hosted web search 配进 Rig request additional params 时，前端显示 `searched`。

本计划不要求证明 provider 实际调用了搜索，不解析 query、url、source 或 provider response，不做 stream tap，不做 HTTP wrapper，不检查 HTTP body/header。

## 事实核查结论

- 当前仓库已经通过 `AgentBuilder::additional_params` 注入 hosted web search：`crates/app-server-core/src/agent.rs`。
- OpenAI Responses 当前配置为 `tools = [{"type":"web_search"}]`。
- Anthropic 当前配置为 `tools = [{"type":"web_search_20250305","name":"web_search"}]`。
- OpenAI-compatible Completions 当前不发送 hosted web search。
- `AgentToolObserver` 会写入 Chat tool history，不能复用为 hosted tool request 事件。
- 如果要支持刷新/历史恢复，必须同时扩展 `ServerPushEvent`、`AgentEventPayload`、dispatcher 的 `agent_payload_from_push` 和 Web 的 `AgentEventRecord` 映射。
- 本计划必须升级到 `rig-core 0.36.0`。升级是版本约束；当前实现仍使用 `AgentBuilder::additional_params`，因为 `ProviderToolDefinition` 不在当前 `AgentBuilder` 链路上。

## Phase 1 — Rig 0.36 Upgrade And Hosted Tool Helper

### 输入

- `Cargo.toml`
- `Cargo.lock`
- `crates/app-server-core/src/agent.rs`
- `crates/app-server-core/tests/agent_tool_tests.rs`
- `crates/app-server-core/tests/llm_tests.rs`

### 前序目标保护

- 保护现有 token、reasoning、本地 tool call / result、timeout、cancel 行为。
- 保护 provider 配置读取规则，不读取 `agents.toml` 内容。

### 操作步骤

1. 将 workspace 的 `rig-core` 依赖升级到 `0.36.0`。
2. 更新锁文件。
3. 只处理升级引起的编译问题，不改变 Agent 本地 tool、token、reasoning、timeout、cancel 行为。
4. 新增内部结构 `HostedToolRequest`，字段包含 `tool_type`、`provider_tool_type`、`provider_tool_name`。
5. 新增内部函数 `hosted_tool_requests_for_config(config: &RigAgentConfig) -> Vec<HostedToolRequest>`。
6. `openai_responses` 且 `native_web_search = true` 时返回 `tool_type = "web_search"`、`provider_tool_type = "web_search"`。
7. `anthropic` 且 `native_web_search = true` 时返回 `tool_type = "web_search"`、`provider_tool_type = "web_search_20250305"`、`provider_tool_name = "web_search"`。
8. `openai_completions` 返回空列表。
9. 让 `rig_agent_additional_params` 使用 `hosted_tool_requests_for_config` 生成 `tools` 字段，保证 request 配置和后续事件同源。
10. 添加单元测试覆盖三类 provider 的 helper 输出和 additional params。

### 验收标准

- `Cargo.toml` 使用 `rig-core = "0.36.0"`。
- `Cargo.lock` 锁定 `rig-core 0.36.0`。
- OpenAI Responses additional params 仍包含 `{"type":"web_search"}`。
- Anthropic additional params 仍包含 `{"type":"web_search_20250305","name":"web_search"}`。
- OpenAI-compatible Completions 不生成 hosted web search request。
- `cargo test -p app-server-core agent_tool_tests` 通过。
- `cargo test -p app-server-core llm_tests` 通过。

## Phase 2 — Protocol Event

### 输入

- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-protocol/src/lib.rs`
- `crates/app-server-protocol/tests/borsh_payload_roundtrip_tests.rs`
- `crates/app-server-protocol/tests/wire_payload_contract_tests.rs`
- `packages/app-server-protocol/src/index.ts`

### 前序目标保护

- 不改变 `agent.tool_start` / `agent.tool_result` 语义。
- 不把 hosted tool request 表达为 `ChatToolCallRecord` 或 `ChatToolResultRecord`。

### 操作步骤

1. 新增 `AgentHostedToolActivityEvent`，字段包含 `session_id`、`run_id`、`provider_id`、`provider_kind`、`tool_type`、`status`。
2. 当前 `status` 只使用 `requested`。
3. 在 `ServerPushEvent` 增加 `agent.hosted_tool_activity`。
4. 在持久化事件枚举 `AgentEventPayload` 增加 hosted tool activity payload。
5. 更新 Rust re-export、TypeScript mirror、Borsh roundtrip 和 wire payload contract 测试。

### 验收标准

- 实时 push 和持久化 payload 都支持 hosted tool activity。
- 新事件不包含 tool call id、args_json、result_json。
- `cargo test -p app-server-protocol` 通过。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。

## Phase 3 — Host Dispatch

### 输入

- `crates/app-server-core/src/agent.rs`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/tests/shared_dispatcher_roundtrip_tests.rs`

### 前序目标保护

- 保护 Phase 1 的 helper 和 Phase 2 的协议语义。
- 不写入 Chat tool history。

### 操作步骤

1. 在 `RigAgentCallbacks` 中新增 `on_hosted_tool_requested` callback。
2. 在 `run_streaming_rig_agent_turn` 中计算 hosted tool request 列表。
3. 设置 additional params 后、调用 `agent.stream_chat(...)` 前，对每个 hosted tool 调用 callback。
4. host dispatcher 将 callback 映射为 `agent.hosted_tool_activity` push event。
5. 在 `agent_payload_from_push` 中把 hosted tool activity 映射为 `AgentEventPayload`，保证历史恢复可用。
6. 终端 `info` 日志记录 provider、model、tool_type、status；不记录 API key、请求头或 request body。

### 验收标准

- OpenAI Responses 且 hosted web search 被配进 additional params 时推送 `agent.hosted_tool_activity`。
- Anthropic 且 hosted web search 被配进 additional params 时推送 `agent.hosted_tool_activity`。
- OpenAI-compatible Completions 不推送 hosted tool activity。
- `ChatToolCallRecord` / `ChatToolResultRecord` 写入行为不变。
- `cargo test -p app-server-core llm_tests` 通过。
- `cargo test -p app-server-core agent_tool_tests` 通过。
- `cargo test -p app-server-host shared_dispatcher_roundtrip_tests` 通过。

## Phase 4 — Web Display

### 输入

- `crates/studio-common/src/managed_client/inbound.rs`
- `crates/studio-common/tests/managed_client_tests.rs`
- `packages/studio-web/src/state/protocol-store.ts`
- `packages/studio-web/src/workbench/chat-runtime.tsx`
- `packages/studio-web/src/workbench/chat-messages.tsx`
- `packages/studio-web/tests/unit/chat-runtime.test.ts`
- `packages/studio-web/tests/unit/chat-messages.test.tsx`
- `packages/studio-web/tests/unit/protocol-store.test.ts`

### 前序目标保护

- 保护现有 token、reasoning、tool_start、tool_result、mesh_ready、error、done 渲染。
- 不展示 query、url、source 或 provider 原始 JSON。

### 操作步骤

1. 确认 `studio-common` inbound 将新 push event 放入 `agent_events`；如现有通配逻辑已覆盖，只补测试。
2. 更新 Web `AgentEventRecord` 映射，支持从持久化 hosted tool activity 恢复事件。
3. 更新 `chat-runtime.tsx`，让新事件进入消息流。
4. 更新 `chat-messages.tsx`：仅当 `event = "agent.hosted_tool_activity"`、`tool_type = "web_search"`、`status = "requested"` 时显示 `searched`。
5. 添加实时 push 与历史恢复测试。

### 验收标准

- 实时收到 hosted web search requested 事件时显示 `searched`。
- 从历史事件恢复后仍显示 `searched`。
- 其他 hosted tool type 不新增产品文案。
- 没有 hosted tool activity 时 UI 行为不变。
- `cargo test -p studio-common managed_client_tests` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。

## Phase 5 — Final Verification

### 输入

- `prompt-archives/2026050301-native-web-search-observation/plan-00-result.md`
- 相关 Rust / Web / protocol 测试命令。

### 前序目标保护

- 不读取、打印、复制或归档 `agents.toml` 内容。
- 不把本次验收 prompt 或一次性业务文案写入通用产品代码。

### 操作步骤

1. 运行相关回归。
2. 运行 `git diff --check`。
3. 更新 `plan-00-result.md`，记录每个 Phase 的执行结果、验证命令和遗留风险。

### 验收标准

- `cargo test -p app-server-core llm_tests` 通过。
- `cargo test -p app-server-host shared_dispatcher_roundtrip_tests` 通过。
- `cargo test -p app-server-protocol` 通过。
- `cargo test -p studio-common managed_client_tests` 通过。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。
- `git diff --check` 通过。
