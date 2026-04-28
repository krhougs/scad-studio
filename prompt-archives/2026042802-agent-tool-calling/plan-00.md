# Agent Tool Calling — 实施计划

## Phase 1 — LLM 类型与 Provider 扩展

扩展 `llm/mod.rs` 类型系统：
- `LlmToolDefinition`：工具定义（name/description/parameters schema）
- `LlmToolCall`：LLM 返回的工具调用（id/function_name/arguments）
- `LlmResponse`：stream_chat 返回类型，含 content 和 tool_calls
- `LlmMessage` 增加 tool_calls 和 tool_call_id 字段，添加构造方法
- `LlmProvider::stream_chat` 签名加 `tools: &[LlmToolDefinition]`，返回 `LlmResponse`

扩展 `llm/openai_compat.rs`：
- `build_request_body` 接受 tools 参数，有 tools 时序列化到请求体
- `serialize_message` 处理 tool 角色和 assistant tool_calls
- `read_sse_stream` 解析 tool_calls delta（按 index 累积 id/name/arguments），返回 `LlmResponse`
- `stream_chat` 适配新签名

验收：
- `cargo check` 通过
- 现有 `llm_tests.rs` 全部通过（需更新调用签名）
- 新增测试：tool call SSE 解析、带 tools 的 request body 序列化

## Phase 2 — 工具定义与执行器

新建 `crates/app-server-core/src/agent/tools.rs`：
- `agent_tool_definitions()` 返回 read_file 和 list_directory 的 LlmToolDefinition
- `ToolExecutor` trait：`fn execute(&self, call: &LlmToolCall) -> String`
- `WorkspaceToolExecutor`：workspace 沙箱执行器，path traversal 防护
- `run_tool_loop()`：agentic 多轮循环——LLM 调用 → 执行工具 → 结果注入 → 继续，直到 LLM 回复纯文本

保护前序 Phase 目标：
- 不修改 Execute 路径
- 不修改 Plan 确认卡片逻辑

验收：
- `cargo check` 通过
- 新增测试：tool definitions 结构验证、WorkspaceToolExecutor 沙箱测试、run_tool_loop 模拟测试

## Phase 3 — Agent 接入与 Dispatcher 接线

修改 `agent.rs`：
- `mod tools;` 引入工具模块
- `stream_agent_turn_with_tools()` 公开函数，构建消息后调用 `run_tool_loop`
- 更新 `stream_agent_turn()` 和 `llm_generate_cadquery_code()` 适配新 provider 签名（传 `&[]` 无工具）

修改 `lib.rs`：
- Re-export 新类型

修改 `dispatcher.rs`：
- `run_text_agent_llm` 使用 `stream_agent_turn_with_tools`，传入 `WorkspaceToolExecutor`
- `generate_agent_cadquery_llm` 保持不变（无工具）

验收：
- `cargo check` 全 workspace 通过
- `cargo test --workspace` 全部通过
- `bun run dev` 启动后，Agent 能读取 workspace 文件并给出有意义回答
