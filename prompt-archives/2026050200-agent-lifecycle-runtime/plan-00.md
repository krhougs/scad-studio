# Agent 生命周期与 WebSocket 观察架构实施计划

## 背景

当前 Agent run 从 WebSocket connection 对应的 `HostRequestDispatcher` 中启动，worker 持有该 connection 的 `push_sink`。这种结构会让 Agent 实时事件与单个 WebSocket 生命周期绑定，无法完整支持页面刷新、多个 WebSocket 同时观察同一个 Agent、以及断线后恢复 active Agent 状态。

本计划将 Agent 生命周期迁移为 workspace 级后端运行时，由稳定 `agent_id` 作为外部操作目标。WebSocket 只作为命令和观察通道；Agent runtime 管理状态、事件日志、订阅者、chat 模型绑定和 active turn。

## 锁定决策

- 外部消费者只能通过 `agent_id` 查询、订阅、取消和发送 Agent 命令。
- `run_id` 或 `turn_id` 只作为内部 turn 追踪字段和事件排序字段，不作为外部操作目标。
- 一个 chat 对应一个稳定 Agent。
- Chat 首次发送消息时创建模型绑定；之后该 chat 的后续 Agent turn 必须使用已绑定模型，后端忽略前端传入的不同模型参数。
- 多个 WebSocket connection 可以同时订阅同一个 Agent。
- WebSocket 断开只移除 subscriber，不取消 active Agent。
- Active Agent 在无 WebSocket 连接时继续运行，并写入后端状态和事件日志。
- Agent idle 且无 subscriber 时，释放 LLM client、provider stream、tool executor、push handle 等运行对象，仅保留可恢复状态。
- 本计划保持 workspace 内同一时间只有一个 active Agent turn 的产品约束；该约束必须由后端 runtime 强制。
- Provider type 产品语义固定支持 `anthropic`、`openai_responses`、`openai_completions`。
- Provider 产品配置使用 `base_url`，不使用 endpoint 作为新的产品配置字段。
- OpenAI family provider 的 `base_url` 补全由 budn' 配置层负责；Rig 只负责斜杠拼接，不负责补全 `/v1`。
- 所有 provider type 的 `base_url` 以 `#` 结尾时，去掉末尾 `#` 后强制使用输入地址，不做路径补全。
- OpenAI family provider 的 `base_url` 以 `/` 结尾时，不追加 `/v1`。
- 根目录 `llm.toml` 属于本地开发环境相关配置，不迁移到产品配置整改范围。
- 实现必须按 TDD 推进，不允许只补前端状态或临时兼容。

## 保护边界

- 保护 app server 是唯一能力层的边界。
- 保护 WebSocket 只消费 app server protocol，不直接读取后端状态文件。
- 保护 Chat history、Agent event、CadQuery staging、workspace tool policy、preview、watch 和 Web 工作台既有行为。
- 保护 provider/model registry、provider type、`base_url` 解析和模型参数快照语义。
- 保护 `agents.toml` 私有配置边界和 API key 不进入仓库。
- 保护根目录 `llm.toml` 不被迁移进产品配置或示例文档。
- 保护现有 async 主链路，不新增手写系统线程或阻塞式请求路径。

## 主要输入

- `docs/2026050200-agent-lifecycle-runtime/architecture.md`
- `agents.example.toml`
- `README.md`
- `docs/getting-started.md`
- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/src/websocket.rs`
- `crates/app-server-core/src/chat.rs`
- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/src/agent.rs`
- `crates/studio-common/src/managed_client/*`
- `crates/studio-web-wasm/src/wasm_bridge/*`
- `packages/studio-web/src/state/protocol-store.ts`
- `packages/studio-web/src/workbench/chat-actions.ts`
- `packages/studio-web/src/workbench/chat-zone.tsx`
- `packages/studio-web/tests/unit/*`
- `crates/app-server-host/tests/*`
- `crates/studio-common/tests/*`

## Phase 1 — Provider type 与 base_url 产品配置

### 输入

- `docs/2026050200-agent-lifecycle-runtime/architecture.md`
- `agents.example.toml`
- `README.md`
- `docs/getting-started.md`
- `crates/app-server-core/src/llm/config.rs`
- `crates/app-server-core/src/agent.rs`

### 前序目标保护

- 本 Phase 为首个 Phase，没有前序 Phase。
- 保护 `agents.toml` 私有配置边界和 API key 不进入仓库。
- 保护根目录 `llm.toml` 不进入产品配置迁移范围。

### 操作步骤

1. 定义 provider type 的产品语义：`anthropic`、`openai_responses`、`openai_completions`。
2. 定义 `base_url` 字段在 provider 配置、模型发现、Agent turn 执行中的一致语义。
3. 定义 `base_url` 补全规则：`#` 结尾强制原样；OpenAI family 非 `/` 结尾补全 `/v1`；OpenAI family `/` 结尾不补全 `/v1`；Anthropic 不补全 `/v1`。
4. 定义 OpenAI Responses 与 OpenAI Chat Completions 在运行路径和模型发现路径中的 provider type 分流语义。
5. 更新产品示例配置和用户文档，明确 `llm.toml` 不作为产品配置来源。

### 验收标准

- 配置测试覆盖 `anthropic`、`openai_responses`、`openai_completions` 三类 provider type。
- 配置测试覆盖 OpenAI family 未配置 `base_url`、无尾斜杠、有尾斜杠、`#` 强制原样四类路径。
- 配置测试覆盖 Anthropic `base_url` 不追加 `/v1`，以及 `#` 强制原样。
- 模型发现和 Agent turn 执行使用同一份解析后的 provider 配置。
- 产品文档和示例配置不要求迁移或读取根目录 `llm.toml`。

## Phase 2 — Agent 身份与 Chat 绑定协议设计

### 输入

- `docs/2026050200-agent-lifecycle-runtime/architecture.md`
- `crates/app-server-protocol/src/protocol.rs`
- `crates/app-server-protocol/tests/borsh_payload_roundtrip_tests.rs`
- `crates/app-server-protocol/tests/wire_payload_contract_tests.rs`
- `crates/app-server-core/src/chat.rs`

### 前序目标保护

- 保护 Phase 1 的 provider type 与 `base_url` 产品配置语义。
- 保护现有 Chat session id 和 Chat history JSONL 读取能力。
- 保护现有 `agent.invoke` 兼容性规划，迁移过程中不得让旧 Web 客户端静默使用错误模型。

### 操作步骤

1. 定义稳定 `AgentId`、`AgentTurnId`、chat-agent binding、Agent snapshot、Agent event log 的 protocol 结构。
2. 定义从 chat session 获取或创建 Agent 的命令语义。
3. 定义 `agent_id` 作为 cancel / snapshot / subscribe / start turn 的唯一外部目标。
4. 定义 chat 模型绑定持久化语义：首次 turn 前创建，后续 turn 只读。
5. 定义旧 `run_id` 字段迁移策略：事件中保留内部 turn id，外部命令不再用 run id 定位 Agent。

### 验收标准

- protocol roundtrip 覆盖 `AgentId`、`AgentTurnId`、Agent snapshot、chat-agent binding 和 subscribe/cancel/start turn 命令。
- wire contract 覆盖新增字段和版本升级。
- 计划明确说明：外部命令不接受 `run_id` 作为 Agent 操作目标。
- 计划明确说明：已绑定 chat 忽略前端后续模型变化。

## Phase 3 — WorkspaceAgentRuntime 后端边界

### 输入

- Phase 2 protocol 结构。
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-host/src/websocket.rs`
- `crates/app-server-core/src/chat.rs`

### 前序目标保护

- 保护 Phase 1 的 provider type 与 `base_url` 产品配置语义。
- 保护 Phase 2 的 `agent_id` 外部目标约束。
- 保护 Chat 模型绑定持久化语义。
- 保护 WebSocket 不直接拥有 Agent 生命周期。

### 操作步骤

1. 引入 workspace 级 `WorkspaceAgentRuntime`，由 host 进程按 workspace 管理。
2. 将 Agent active turn registry 从 `HostRequestDispatcher` 移入 runtime。
3. 将 Agent worker 的事件输出改为写入 runtime event log，再由 runtime 广播给 subscriber。
4. 将 WebSocket connection 注册为 runtime subscriber；断开时只移除 subscriber。
5. 在 runtime 中强制 workspace 单 active Agent turn。

### 验收标准

- 后端测试证明两个 dispatcher / WebSocket observer 可以引用同一个 `agent_id`。
- 后端测试证明 WebSocket disconnect 不 cancel active Agent。
- 后端测试证明第二个 observer 可以读取 active Agent snapshot。
- 后端测试证明同一 workspace 同时启动第二个 active turn 会返回后端错误。

## Phase 4 — Chat 模型绑定与后端模型强制

### 输入

- Phase 3 runtime。
- `crates/app-server-host/src/dispatcher.rs`
- `crates/app-server-core/src/chat.rs`
- `crates/app-server-core/src/llm/config.rs`

### 前序目标保护

- 保护 Phase 1 的 provider type 与 `base_url` 产品配置语义。
- 保护 Phase 2/3 的稳定 `agent_id` 和 runtime 生命周期边界。
- 保护 provider/model registry、provider type、`base_url` 和 `Option<String>` 参数快照语义。

### 操作步骤

1. 首次 Agent turn 前根据当前请求模型快照创建 `ChatAgentBinding`。
2. 将 binding 写入持久状态，保证刷新页面或 host 重启后可恢复。
3. 后续同 chat 的 Agent turn 从 binding 读取模型，不使用前端传入的不同模型。
4. Agent snapshot 返回绑定模型和模型控件只读原因。

### 验收标准

- 测试覆盖空 chat 首次 turn 使用请求模型并创建 binding。
- 测试覆盖已绑定 chat 后续 turn 忽略不同请求模型。
- 测试覆盖刷新或新 dispatcher 读取同一 chat 时恢复绑定模型。
- 测试覆盖前端收到 binding 状态后把模型控件设为只读。

## Phase 5 — Event log、Snapshot 与重连恢复

### 输入

- Phase 3 runtime。
- `crates/studio-common/src/managed_client/*`
- `crates/studio-web-wasm/src/wasm_bridge/*`
- `packages/studio-web/src/state/protocol-store.ts`
- `packages/studio-web/src/workbench/chat-zone.tsx`

### 前序目标保护

- 保护 Phase 1 的 provider type 与 `base_url` 产品配置语义。
- 保护 Phase 3 的多 subscriber 观察语义。
- 保护 Phase 4 的 chat 模型绑定语义。

### 操作步骤

1. 设计并实现 runtime event log，事件按 `agent_id` 和 `turn_id` 记录。
2. Agent snapshot 返回当前状态、当前文本、reasoning、active tool call、错误、done 状态和事件游标。
3. 新 WebSocket 连接后通过 snapshot 恢复 UI，再订阅后续事件。
4. studio-common managed client 以 `agent_id` 聚合 Agent 状态。
5. Web store 和 Chat UI 以 `agent_id` 过滤和展示事件。

### 验收标准

- 测试覆盖断线期间产生的 token / tool / done 事件可以在新连接中通过 snapshot 或 replay 看到。
- 测试覆盖两个 WebSocket observer 同时收到同一个 Agent 的事件。
- 测试覆盖前端刷新后仍显示 active Agent 正在工作。
- 测试覆盖 done 后刷新页面能看到最终 Chat history 和 Agent 状态摘要。

## Phase 6 — Idle 资源释放

### 输入

- Phase 3 runtime。
- Phase 5 event log 与 snapshot。

### 前序目标保护

- 保护 Phase 1 的 provider type 与 `base_url` 产品配置语义。
- 保护 active Agent 断线继续运行。
- 保护多 subscriber 观察同一个 Agent。

### 操作步骤

1. 定义 active / idle / done / failed / cancelled 的状态转移。
2. active turn 完成后释放 LLM client、provider stream、tool executor、cancel token 和 task handle。
3. subscriber 数为 0 且 Agent idle 时，仅保留持久状态和 event log。
4. 新 subscriber 连接时不重建 LLM stream，只读取可恢复状态。

### 验收标准

- 测试覆盖 active turn 完成后 runtime 不再持有 active turn handle。
- 测试覆盖 idle 且无 subscriber 时删除 subscriber 相关资源。
- 测试覆盖 active 且无 subscriber 时 turn 继续运行。
- 测试覆盖新 subscriber 读取 idle Agent snapshot 不创建 LLM client。

## Phase 7 — Async / 阻塞路径复核与最终验证

### 输入

- 全部前序 Phase。
- `docs/known_issues.md`
- `crates/app-server-host/src/*`
- `crates/app-server-core/src/*`
- `crates/scad-scene/src/system_fonts.rs`

### 前序目标保护

- 保护 Phase 1 的 provider type 与 `base_url` 产品配置语义。
- 保护 Phase 2-6 的 Agent 生命周期与 WebSocket 生命周期分离。
- 保护当前外部工具只能通过 app server 管理的边界。

### 操作步骤

1. 检索生产代码中新增的 `std::thread`、`thread::spawn`、`spawn_blocking`、`block_in_place`、同步 `std::process::Command`。
2. 确认 Agent / WebSocket 主链路没有新增手写线程或同步阻塞外部命令。
3. 对已知的 `scad-scene` 字体探测同步命令保持已知问题记录，不在本计划中顺手修改。
4. 运行 Rust、protocol、Web typecheck、Web 单元测试和 smoke。

### 验收标准

- `cargo test --workspace` 通过。
- `bun run protocol:build` 通过。
- `bun run protocol:check-generated` 通过。
- `bun run --cwd packages/studio-web typecheck` 通过。
- `bun run --cwd packages/studio-web test:unit` 通过。
- `bun run web:smoke` 通过。
- `git diff --check` 与 `git diff --cached --check` 通过。
- 搜索确认 Agent / WebSocket 主链路未新增手写线程或同步阻塞外部命令。
- 搜索确认产品文档和示例配置不要求迁移或读取根目录 `llm.toml`。

## 执行前检查

执行本计划前必须通读：

- `prompt-archives/2026050200-agent-lifecycle-runtime/plan-prompt.md`
- `prompt-archives/2026050200-agent-lifecycle-runtime/plan-00.md`
- `docs/2026050200-agent-lifecycle-runtime/architecture.md`
- 根 `AGENTS.md`

本计划不存在需补全占位符、需用户继续选择的方案或缺失验收标准。执行阶段不得停下来询问是否改用 `run_id` 或是否让 WebSocket 持有 Agent 生命周期；这些边界已经锁定。
