# Hosted Tool Request Display Prompt 存档

## 用户输入

用户最初要求围绕 Rig 0.36 与 provider-native `web_search` 分析实现方式，并希望前端显示类似 `searched` 的轻量状态。

后续讨论中，目标多次收窄并最终明确为：

- 不需要证明 provider 实际调用了 hosted tool。
- 不需要解析或展示 query、url、source。
- 不需要 response stream tap。
- 只需要确认本轮 LLM request 已经把 hosted web search tool 发给 provider。
- 当前前端只展示 web search 的 `searched`。
- 底层事件仍应按 hosted tools 设计，避免写成 search-only 临时逻辑。

用户明确要求：

- 研究和事实核查后写 plan。
- 直接覆盖 `plan-00.md`。
- 不读取 `agents.toml` 或任何本机密钥文件内容。

## 当前目标

只计划，不实现：

- 升级 `rig-core` 到 `0.36.0`。
- 建立 request-side hosted tool helper。
- 在构造 Rig request 时确认 hosted web search tool 已加入 additional params 后发送统一 `agent.hosted_tool_activity` 事件。
- 前端收到 `tool_type = "web_search"` 且 `status = "requested"` 后显示 `searched`。
- 不观察 provider response，不解析 SSE response，不解析 query / url / source。
- 不做 HTTP wrapper，不检查 outbound HTTP body 或 header。
- 不把 provider-native hosted tool 变成本地 function tool。
- 不写入 Chat tool history。

## 事实核查摘要

当前仓库：

- `Cargo.toml` 仍使用 `rig-core = "0.35.0"`。
- `Cargo.lock` 仍锁定 `rig-core 0.35.0`。
- OpenAI Responses client 构建点：`crates/app-server-core/src/agent.rs`。
- Anthropic client 构建点：`crates/app-server-core/src/agent.rs`。
- 当前 provider-native web search request 配置：
  - OpenAI Responses：`tools = [{"type":"web_search"}]`。
  - Anthropic：`tools = [{"type":"web_search_20250305","name":"web_search"}]`。
  - OpenAI-compatible Completions：不发送 hosted web search。
- 当前 `AgentToolObserver` 只有 `tool_start` / `tool_result`，host observer 会写入 Chat history，因此不能复用它表达 hosted tool request。
- `ServerPushEvent` 当前没有 hosted tool request/activity 事件。
- `studio-common` 会把 Agent push event 放进 `agent_events`。
- Web 当前 event summary 在 `packages/studio-web/src/workbench/chat-messages.tsx` 中处理。

Rig 0.36 与 PR #1430：

- PR #1430 增加的是 request-side hosted tools 支持，包括 `ProviderToolDefinition`、`CompletionRequest::with_provider_tool(s)` 和 provider-specific request serialization。
- PR #1430 不能证明 provider 实际调用 hosted tool；但用户当前目标只要求确认 hosted tool 已随 request 发出。
- 本计划要求升级到 `rig-core 0.36.0`。
- 当前仓库使用 `AgentBuilder::additional_params` 链路，`rig-core 0.36.0` 的 `ProviderToolDefinition` 不是当前实现路径的直接入口。
- 因此本轮计划升级 Rig，但仍不需要 response stream tap，也不需要 HTTP request body/header 检查；只以同源 hosted tool request 配置作为判断依据。

## 约束

- 本计划不实现代码。
- 后续执行时禁止读取 `agents.toml` 内容；需要实际 LLM 验证时使用本机已配置环境，但不得打印、复制、归档配置内容。
- Agent 产品功能验证禁止 mock LLM；判断是否达成用户意图应由第三方 LLM 分析对话与事件记录，不使用固定文本断言。
- request construction、protocol roundtrip、UI 展示这类非 LLM 行为可以用确定性单元测试覆盖。
