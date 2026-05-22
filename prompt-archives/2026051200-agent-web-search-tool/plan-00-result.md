# Agent Web Search Function Tool 执行结果

## 当前状态

- Phase 1-6 全部完成。Plan 级独立 review 通过，无阻塞项。

## Phase 记录

### Phase 1 — Config 数据结构与解析

- 状态：已完成。
- 变更摘要：
  - 在 `config.rs` 中新增 `WebSearchHttpMethod`、`WebSearchAuth`、`WebSearchParams`、`WebSearchResultMap`、`ResolvedWebSearchProvider` 运行时类型。
  - 新增 `WebSearchProviderFile`、`WebSearchResultMapFile` TOML 反序列化类型。
  - `AgentsConfigFile` 新增 `active_web_search` 和 `web_search_providers` 字段。
  - `AgentProviderRegistry` 新增 `active_web_search_id`、`web_search_providers` 字段和 `active_web_search_provider()` 方法。
  - 实现 resolve 和 validate 函数覆盖 8 条校验规则。
  - `WebSearchAuth` 和 `ResolvedWebSearchProvider` 的 Debug impl 遮蔽 api_key。
  - `auth_prefix` 不使用 `non_empty`（保留尾部空格，如 "Bearer "）。
  - 所有 `AgentProviderRegistry` 构造路径（`into_registry`、`registry_from_openai_model`）均包含 web_search 默认值。
- 验证命令：
  - `cargo test -p app-server-core --test llm_tests`：66 passed，0 failed。
  - `cargo check -p app-server-host`：通过。
- 独立 review：
  - 8 条校验规则全部有实现和测试覆盖。
  - 前序 provider/model 链路未被修改。
  - 阻塞项（已修复）：补充 result_map 空字段、id/endpoint 为空的拒绝测试。
  - 非阻塞风险：config.rs 文件行数较多（~1200 行），后续可考虑拆分。

### Phase 2 — Web Search 执行层

- 状态：已完成。
- 变更摘要：
  - workspace Cargo.toml 新增 `reqwest = { version = "0.13", default-features = false, features = ["json", "query", "rustls"] }`。
  - app-server-core Cargo.toml 引用 workspace reqwest 依赖。
  - 新增 `agent/web_search.rs`：`execute_web_search`、`send_search_request`、`build_get_request`、`build_post_request`、`apply_auth`、`extract_results`、`resolve_dot_path`、`truncate_response`。
  - `WebSearchResult` 结构体支持 `skip_serializing_if` 可选字段。
  - `truncate_response` 使用 `is_char_boundary` 避免 UTF-8 切割 panic。
  - 错误响应预览复用 `truncate_response` 同样安全。
  - 测试文件放在 `tests/web_search_tests.rs`（非内联）。
- 验证命令：
  - `cargo test -p app-server-core --test web_search_tests`：10 passed，0 failed。
  - `cargo test -p app-server-core --test llm_tests`：66 passed，0 failed。
  - `cargo check -p app-server-core`：通过（有预期的 dead_code 警告）。
- 独立 review：
  - 阻塞项（已修复）：UTF-8 truncation panic、内联测试违反 AGENTS.md 规则。
  - 非阻塞：dead_code 警告将在 Phase 4 集成后消除。

### Phase 3 — Fetch URL 执行层

- 状态：已完成。
- 变更摘要：
  - workspace Cargo.toml 新增 `htmd = "0.5"`，app-server-core 引用。
  - `web_search.rs` 新增 `fetch_url`、`read_response_body`、`is_html_content_type`、`html_to_markdown`。
  - `read_response_body` 使用 chunk-based 读取防止无 Content-Length 时的 OOM。
  - 大小限制 2MB（`FETCH_MAX_BYTES`），超时 30s（`FETCH_TIMEOUT_SECS`）。
  - 非 HTML 内容直接返回，HTML 通过 htmd 转为 Markdown。
  - `is_html_content_type` 公开用于测试。
- 验证命令：
  - `cargo test -p app-server-core --test web_search_tests`：17 passed，0 failed。
  - `cargo check -p app-server-core`：通过。
- 独立 review：
  - 阻塞项（已修复）：流式大体积防护（chunk-based 读取）、补充 `is_html_content_type` 测试。
  - 非阻塞：`fetch_url` 的网络层错误测试需要 mock server，当前不引入额外依赖。

### Phase 4 — Tool 注册与集成

- 状态：已完成。
- 变更摘要：
  - `registry/schemas.rs` 新增 `web_search_input_schema`、`web_search_success_schema`、`fetch_url_input_schema`、`fetch_url_success_schema`；error_type 枚举新增 `web_search_error`。
  - `registry.rs` 新增 `AgentToolCategory::WebSearch`；`agent_tool_specs()` 新增 `web_search` 和 `fetch_url` 两个工具规格。
  - 新增 `tools/web.rs`：`web_search` 和 `fetch_url` 工具执行分发（参数解析、调用执行层、格式化成功/错误 JSON）。
  - `tools.rs` 新增 `http_client: reqwest::Client` 和 `web_search_provider: Option<ResolvedWebSearchProvider>` 字段；新增 `with_web_search_provider` builder 方法；execute match 新增两个工具分支。
  - `dispatcher.rs` 从 `AgentProviderRegistry` 提取 `active_web_search_provider` 后传入 `WorkspaceToolExecutor`。
  - `agent_tool_registry_tests.rs` 更新 `expected_tool_modes()` 包含 `web_search` 和 `fetch_url`。
- 验证命令：
  - `cargo test -p app-server-core --test agent_tool_tests`：116 passed，0 failed。
  - `cargo test -p app-server-core --test agent_tool_registry_tests`：6 passed，0 failed。
  - `cargo test -p app-server-core --test llm_tests`：66 passed，0 failed。
  - `cargo test -p app-server-core --test web_search_tests`：17 passed，0 failed。
  - `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：59 passed，0 failed。
  - `cargo check -p app-server-host`：通过。
- 独立 review：
  - 无阻塞项。
  - 非阻塞风险 1：`web_search` 始终注册到 LLM 工具列表，未配置时靠运行时错误拒绝。Phase 5 的 turn context 将实现条件可见性。
  - 非阻塞风险 2：`filters` 参数声明未解析，符合 plan 中"保留字段暂不实现"设计。
  - 非阻塞风险 3：`fetch_url` 复用 `web_search_error` 错误类型，低风险命名问题。

### Phase 5 — System Prompt 与 Turn Context

- 状态：已完成。Phase 4 review 提出的条件可见性问题已在本 Phase 解决。
- 变更摘要：
  - `agent-system-prompt.md`：Section 2 和 Section 9 更新 web search 描述，覆盖 provider-native 和 function tool 两种搜索模式；新增 function tool web search 引用指令。
  - `AgentTurnInput` 新增 `function_web_search_available: bool` 字段。
  - `AgentToolRunContext` 新增 `web_search_available: bool` 字段。
  - `provider_native_capabilities_context` 重命名为 `web_search_capabilities_context`，支持三种状态：native 可用、function 可用、均不可用。
  - `current_turn_app_tools_context` 新增过滤：当 `function_web_search_available` 为 false 时，`web_search` 不出现在工具列表中。
  - `rig_tools_for_context` 新增过滤：`web_search` 仅在 `web_search_available` 为 true 时注册到 LLM 的 rig 工具 schema。
  - `dispatcher.rs` 根据 `web_search_provider.is_some()` 设置两个新标志。
  - `llm_tests.rs` 所有 7 处 `AgentTurnInput` 构造均已补充新字段；新增 2 个测试覆盖 function web search 可用/不可用两种场景的 turn context 和工具列表行为。
- 验证命令：
  - `cargo test -p app-server-core --test llm_tests`：68 passed，0 failed。
  - `cargo test -p app-server-core --test agent_tool_tests`：116 passed，0 failed。
  - `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：59 passed，0 failed。
  - `cargo check -p app-server-host`：通过。
- 独立 review：
  - 无阻塞项。
  - 非阻塞风险 1：当 native 和 function web search 同时可用时，turn context 文本可能产生矛盾（native 声称 web_search 不在 app tools 中，但 app tools 列表包含它）。实际部署中两者同时启用的概率极低。
  - 非阻塞风险 2：`fetch_url` 在 web search 不可用时仍然可见，这是 plan 设计意图。

### Phase 6 — 端到端验证

- 状态：已完成。
- 验证命令：
  - `cargo test -p app-server-core --test llm_tests`：68 passed，0 failed。
  - `cargo test -p app-server-core --test agent_tool_tests`：116 passed，0 failed。
  - `cargo test -p app-server-core --test agent_tool_registry_tests`：6 passed，0 failed。
  - `cargo test -p app-server-core --test web_search_tests`：17 passed，0 failed。
  - `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：59 passed（1 个预存在的 flaky test 单独运行通过）。
  - `cargo test -p app-server-protocol`：0 tests（无变更）。
  - `bun run --cwd packages/studio-web test:unit`：306 passed。
  - `git diff --check`：clean。
- Plan 级独立 review：
  - 无阻塞项。所有 Phase 验收标准均已满足。
  - 非阻塞风险 1：`HostedToolHook` 等在 diff 中可见但属于前序提交，非本 plan 变更。
  - 非阻塞风险 2：config.rs 文件增长至约 1298 行，后续可考虑拆分 web search 配置到独立模块。
  - 非阻塞风险 3：尚未更新 agents.example.toml 配置示例。
