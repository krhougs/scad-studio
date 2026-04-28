# Agent Tool Calling — 执行结果

## Phase 1-3 — 一次性完成

**状态：** 完成（review findings 已修复）

### Review Findings 处理
- F1 [critical] 路径穿越 symlink 绕过 — 已修复：`resolve_safe` 对已存在路径使用 `canonicalize` 解析 symlink，与 canonical workspace root 比对
- F2 [medium] `list_directory` 无条目数限制 — 已修复：引入 `MAX_DIR_ENTRIES=500` 截断
- F3 [medium] `run_tool_loop` 超限丢失内容 — 已修复：返回最后一轮累积的 content 而非 Err

### 变更清单

| 文件 | 变更 |
|------|------|
| `crates/app-server-core/src/llm/mod.rs` | 新增 LlmToolDefinition/LlmToolCall/LlmResponse 类型；LlmMessage 增加 tool_calls/tool_call_id 字段和构造方法；LlmProvider trait 签名改为 `stream_chat(messages, tools, on_token) -> LlmResponse` |
| `crates/app-server-core/src/llm/openai_compat.rs` | `build_request_body` 接受 tools 参数；`serialize_message` 处理 tool/assistant-with-tool_calls 消息格式；`read_sse_stream` 通过 ToolCallAccumulator 解析 tool_calls delta，返回 LlmResponse；`stream_chat` 适配新签名 |
| `crates/app-server-core/src/agent/tools.rs` | **新文件**。agent_tool_definitions()（read_file/list_directory）；ToolExecutor trait；WorkspaceToolExecutor（canonicalize symlink 防御 + 条目/字节截断）；run_tool_loop() agentic 多轮循环 |
| `crates/app-server-core/src/agent.rs` | 新增 `mod tools`；stream_agent_turn_with_tools()；stream_agent_turn()/llm_generate_cadquery_code() 适配新 API（传 `&[]` 无工具） |
| `crates/app-server-core/src/lib.rs` | Re-export stream_agent_turn_with_tools、ToolExecutor、WorkspaceToolExecutor、agent_tool_definitions、run_tool_loop |
| `crates/app-server-host/src/dispatcher.rs` | run_text_agent_llm 创建 WorkspaceToolExecutor 并调用 stream_agent_turn_with_tools |
| `crates/app-server-core/Cargo.toml` | dev-dependencies 新增 tempfile = "3" |
| `crates/app-server-core/tests/llm_tests.rs` | 更新现有测试适配新 API；新增 6 个测试（tool call SSE 解析、tools request body、tool message 序列化） |
| `crates/app-server-core/tests/agent_tool_tests.rs` | **新文件**。12 个测试（tool definitions 验证、WorkspaceToolExecutor 沙箱、run_tool_loop mock 多轮） |

### 验证结果
- `cargo check` 全 workspace 通过（无新 warning）
- `cargo test --workspace` 全部通过，0 失败
- 新增测试：34（llm_tests）+ 12（agent_tool_tests）= 46 个
- 所有新文件在 500 行限制内
- 所有新函数在 50 行限制内

### 产品变更
- Agent 在 Inform/Plan/Auto 模式下具备 `read_file` 和 `list_directory` 工具
- LLM 可自主探索 workspace 文件结构和内容后再回答
- Execute 路径（CadQuery）不受影响，保持原有行为
- 工具执行沙箱在 workspace 根目录内，symlink/path traversal 已防御
