# Agent 生命周期与 WebSocket 观察架构执行结果

## 当前状态

- Phase 1「Chat identity 与 chats.json」已完成实现、验证、独立 review 和修正。
- Phase 2「Provider type 与 base_url 产品配置」已完成实现、验证、独立 review 和修正。
- Phase 3「Agent 身份与 Chat 绑定协议设计」已完成实现、验证、独立 review 和修正。
- 本文件记录到 2026-05-02 的执行结果。

## Phase 1 完成情况

### 实现摘要

- `chats.json` 成为 chat 列表、显示顺序、workspace 当前 chat 和 metadata 的权威来源。
- 后端生成随机 `chat_id` 与稳定 `agent_id`，不再从 title、JSONL 文件名或路径派生长期身份。
- `ChatIndexEntry` 承载 `chat_id`、`agent_id`、首次创建 `client_request_id`、title、goal、summary、open questions、archived、created / updated 时间、related files、`messages_path`、`events_path`、`bound_model`。
- Chat history 通过 `chats.json.messages_path` 读取，JSONL 文件名改变不影响 `chat_id`。
- `chats.json.active_chat_id` 在 history/select 和 archive 路径更新，并通过 `ChatListChanged` push 同步给同 workspace dispatcher。
- 旧 JSONL 工作区读取时迁移为 `chats.json` 索引；旧文件名只作为初始 title 来源，不作为身份来源。只有 archived chat 的旧工作区不会设置 active chat。
- `chats.json` 写入使用临时文件加 rename；目标 `chats.json` 与 `chats.json.tmp` 均拒绝 symlink。
- 创建 chat 时创建 Chat JSONL 与 Agent event JSONL，并以 `chats.json` 作为提交标记；已覆盖 event log 创建失败后的清理。
- 首次创建要求 `client_request_id` 和非空 `initial_user_message`；空白 `client_request_id` 会被拒绝。
- 同一 `client_request_id` 的重复创建和并发创建返回同一 `chat_id` / `agent_id`，首条 user message 去重。
- `agent.invoke` 的 `client_request_id` 以 `session_id + request_id` 为 key 在 workspace 级 registry 去重，完成后清理。
- Protocol version 升级到 9，并同步 Rust protocol、TS protocol 和 generated WASM。
- `studio-common` 处理 `ChatListChanged` push，更新 snapshot 并发出 `SnapshotChanged`。
- Web `New Chat` 仅创建本地草稿；首次发送才创建后端 chat。草稿、无草稿直接发送、slash command、saved plan 都携带一次性 `client_request_id`，并通过 create 请求写入首条 user message。
- Web 首发过程中 `chat.list` 或 `agent.invoke` 失败后不会保留本地草稿造成重复状态；busy 状态覆盖 create 到 invoke/history 的完整流程。

### 验收说明

- Archive 已验证不改变 `chat_id` / `agent_id`。
- Rename / reorder 当前没有 protocol 命令或产品入口，因此本 Phase 不新增未规划能力；该验收项在当前代码状态下不适用。后续若新增 rename / reorder 命令，必须补充身份不变测试。
- 进程崩溃恢复窗口属于计划 Phase 7 的重启恢复矩阵；Phase 1 已覆盖正常错误清理、索引损坏、symlink 防护和并发迁移，不在本 Phase 扩展 interrupted 恢复实现。

### Review 记录

- 第一轮独立 review 发现 active chat 未持久化、首发缺 `client_request_id`、并发创建竞态、TS protocol 未同步、metadata 和损坏索引测试不足；已修复。
- 第二轮独立 review 发现首条 user message 并发去重、跨 dispatcher agent invoke 去重、`chats.json` symlink、active chat push、创建失败清理和 legacy event 文件风险；已修复。
- 第三轮独立 review 发现 `ChatListChanged` 未触发 Web snapshot 刷新、saved plan 草稿缺首条 user message、Web summary equality 漏 `agent_id` / `related_files`；已修复。
- 第四轮独立 review 发现 create/send/invoke 拆分会留下空 chat、summary update 不广播、agent request id 作用域和清理问题、临时文件句柄风险；已修复。
- 第五轮独立 review 发现无草稿直接发送缺 request id、saved plan 真实入口缺 request id、slash command 首条消息不一致、listener N×N 广播；已修复。
- 第六轮独立 review 发现 protocol version 未升级、invoke 失败后本地草稿保留、`chat.create` 可缺首条 user message；已修复。
- 第七轮独立 review 发现 `chat.list` 失败后本地草稿保留、busy 提前解除、draft 影响首个后端 title、崩溃恢复风险；前三项已修复，崩溃恢复风险保留到 Phase 7。
- 第八轮独立 review 发现空白 `client_request_id` 和 archived-only legacy active chat 风险；已修复。
- 最终独立 review 未发现阻塞项；仅要求更新本结果文档，并说明 rename / reorder 当前不适用。

### 验证结果

- `cargo test -p app-server-core --test chat_tests`：26 passed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：23 passed。
- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests`：16 passed。
- `cargo test -p app-server-protocol --test wire_payload_contract_tests`：2 passed。
- `cargo test -p studio-common --test managed_client_tests`：26 passed。
- `bun run --cwd packages/studio-web test:unit -- chat-zone.test.tsx`：40 passed；仍有两个既有 React `act(...)` 警告。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run protocol:build`：通过。
- `git diff --check`：通过。
- `bun run protocol:check-generated`：Phase 1 commit 后已重新运行，通过。

## Phase 2 完成情况

### 实现摘要

- Provider type 产品语义统一为 `openai_responses`、`openai_completions` 和 `anthropic`。
- `agents.toml` provider 支持 `base_url`，解析后的值进入 `ResolvedAgentProvider`，并在构造 `RigAgentConfig` 时传给 Agent turn 执行路径。
- `base_url` 解析规则已实现：未配置时使用 Rig 默认；以 `#` 结尾时去掉末尾 `#` 后原样使用；OpenAI family 无尾斜杠时补 `/v1`，有尾斜杠时保留原路径；Anthropic 不补 `/v1`。
- 模型发现路径按 provider type 分流：OpenAI Responses 使用 `Client`，OpenAI Chat Completions 使用 `CompletionsClient` 并复用同一 `base_url` 访问 `/models`，Anthropic 使用 Anthropic builder。
- Agent turn 执行路径按 provider type 分流：OpenAI Responses 使用 Responses client，OpenAI Chat Completions 使用 Completions client，Anthropic 使用 Anthropic client，三者均读取解析后的 `RigAgentConfig.base_url`。
- `openai_completions` 不注入 OpenAI Responses hosted web search、Responses reasoning 或 service tier 参数；该 provider type 当前不标记 provider-native web search 为已应用。
- `AgentModelRegistryProvider`、Web registry fixture、ChatStore 和 protocol payload 不暴露 `base_url`。
- `agents.example.toml`、`README.md`、`docs/getting-started.md` 和 `docs/cadquery-mvp/decisions.md` 已更新为当前 provider type 语义，并说明根目录 `llm.toml` 不作为产品配置入口。
- `docs/known_issues.md` 中旧 provider 描述已更新为当前三类 provider，避免历史记录误导后续开发。

### 验收说明

- 配置测试覆盖三类 provider type。
- 配置测试覆盖 OpenAI family 未配置 `base_url`、无尾斜杠、有尾斜杠、`#` 强制原样四类路径。
- 配置测试覆盖 Anthropic `base_url` 不追加 `/v1`，以及 `#` 强制原样。
- 模型发现和 Agent turn 执行均使用解析后的 provider 配置；源码与测试均未发现默认 endpoint 覆盖解析后 `base_url` 的路径。
- Chat bound model 当前仍不持久化 `base_url`；搜索确认 `base_url` 未进入 protocol registry、Web 状态、ChatStore 或 chat tests。
- 产品文档和示例配置不要求迁移或读取根目录 `llm.toml`。

### Review 记录

- 第一轮独立 review 发现 `openai_completions` 复用 OpenAI Responses additional params 的高风险问题；已修复为不注入 Responses-only 参数，并补充测试与示例说明。
- 第二轮独立 review 未发现阻塞项或高风险问题；剩余低风险为尚未使用 provider mock 做 HTTP URI 级断言。当前已通过 Rig 源码核对确认 builder 保留 `base_url`。

### 验证结果

- `cargo test -p app-server-core --test llm_tests`：45 passed。
- `cargo test -p app-server-core --test chat_tests`：26 passed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：23 passed。
- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests`：16 passed。
- `cargo test -p app-server-protocol --test wire_payload_contract_tests`：2 passed。
- `bun run --cwd packages/studio-web test:unit -- chat-zone.test.tsx`：40 passed；仍有两个既有 React `act(...)` 警告。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run protocol:check-generated`：通过。
- `git diff --check`：通过。
- `rg -n "anthropic_messages|AnthropicMessages" README.md docs agents.example.toml crates packages -g '!packages/studio-web/dist/**'`：无结果。
- `rg -n "base_url" crates/app-server-protocol packages/studio-web/src packages/studio-web/tests/unit crates/app-server-core/src/chat.rs crates/app-server-core/tests/chat_tests.rs -g '!packages/studio-web/dist/**'`：无结果。

## Phase 3 完成情况

### 实现摘要

- Protocol version 升级到 10，新增 `AgentId`、`AgentTurnId`、`AgentEventId`、`BoundAgentModel`、`AgentRuntimeStatus`、Agent snapshot / subscribe / start turn 相关结构。
- `ChatCreateRequest` 支持 `requested_model` 与 `initial_turn`，`ChatCreatedResponse` 返回稳定 `agent_id` 与可选 `initial_turn` 启动结果。
- `agent.cancel` 改为以 `agent_id` 为目标；新增 `agent.start_turn`、`agent.snapshot`、`agent.subscribe` protocol 命令，其中 snapshot / subscribe 当前返回 workspace runtime 尚未接入的明确错误。
- `ChatStore` 持久化每个 chat 的 `agent_id` 与 `bound_model`，并提供 session / agent 双向查询；`bound_model` 不包含 `base_url`。
- `chat.create.initial_turn` 要求同时携带非空 `client_request_id`、非空首条用户消息和 `requested_model`；同一 `client_request_id` 的重试返回同一 chat / agent，不重复启动 initial turn。
- `chat.create.initial_turn` 在创建 chat 前做 workspace 级 reservation，防止不同请求在 Agent busy 时先写入 orphan chat。
- 同一 `client_request_id` 的并发首发通过 `OwnedNotified` 等待 reservation 完成，或在同请求已进入 running 状态后重新读取 `chats.json` 返回既有 chat / agent，避免误返回 `AgentBusy`。
- 旧 `agent.invoke` 保留兼容入口，但会把 session 映射到 `agent_id`；若 chat 已绑定模型，后端忽略旧请求中的不同模型参数。
- 后续 turn 通过 `agent.start_turn` 以 `agent_id` 定位 chat，并只使用 chat 的 bound model。
- Agent worker 运行前检查 bound provider type 与当前 provider config kind 是否一致，避免同 provider id 配置类型切换后静默使用错误运行路径。
- `studio-common`、`studio-web-wasm`、Web wasm bridge 和 TS protocol 已同步新增命令与响应类型。
- Web 首个用户消息、slash command 和 Markdown preview 的 `Run Plan` 均通过 `chat.create.initial_turn` 启动首个 turn，不再先 create 后额外 `agent.invoke`。
- Web 已有 chat 的后续消息通过 `dispatchAgentStartTurn`；缺少 `agent_id` 时拒绝发送并显示状态消息。
- Markdown preview 所在的 WorkbenchLayout 会请求 agent model registry，只有存在活动模型时才启用 `Run Plan`。

### 验收说明

- Protocol roundtrip 覆盖新增 Agent identity、snapshot、subscribe、start turn、cancel by `agent_id`、chat metadata Agent 关联和 bound model。
- Wire contract 覆盖 protocol version 10、cancel 不暴露 `run_id`、bound model 不包含 `base_url`。
- ChatStore 测试覆盖 `chats.json.bound_model`、list summary、session / agent 查询和 `base_url` 不持久化。
- Host 测试覆盖 `chat.create.initial_turn` 写入 bound model、启动 initial turn、缺少 model 拒绝、busy 时不创建 orphan chat、完成后同 request retry 不重复启动、同 request 并发首发幂等。
- Host 单元测试覆盖旧 `agent.invoke` 对已绑定 chat 优先使用 bound model，以及 provider type mismatch 拒绝。
- Web 单元与 Playwright 测试覆盖首发协议帧使用 `chat.create.initial_turn`、后续 turn 使用 `agent.start_turn`、Run Plan 携带 `plan_ref`。
- `agent.snapshot` / `agent.subscribe` 的完整 workspace runtime 行为属于 Phase 4；Phase 3 已定义 protocol 结构并返回明确错误，未伪造 runtime 行为。

### Review 记录

- 第一轮独立 review 发现 `chat.create.initial_turn` 可缺模型、Web 首发仍拆成 create + invoke、Web 缺后续 `agent.start_turn` 路径；已修复。
- 第二轮独立 review 发现 busy 时可能留下已创建 chat、同 request retry 可能重复启动、Playwright 仍断言旧 `agent.invoke`、provider type 未检查；已修复。
- 第三轮独立 review 发现同 request 并发首发在 `chats.json` 写入前可能返回 `AgentBusy`；已改为按 request reservation 等待并补充并发测试。
- 第四轮独立 review 发现同 request running 窗口仍可能返回 `AgentBusy`，并且 `Notify` 可能丢通知；已改为 `DuplicateCommitted` 重新读取 index，并在 registry 锁内创建 `OwnedNotified`。
- 第五轮独立 review 未发现阻塞问题；剩余非阻塞风险为首发 reservation 对外部 task abort 尚未具备 RAII 清理，已记录到 `docs/known_issues.md`。

### 验证结果

- `cargo test -p app-server-protocol --test borsh_payload_roundtrip_tests`：17 passed。
- `cargo test -p app-server-protocol --test wire_payload_contract_tests`：4 passed。
- `cargo test -p app-server-core --test chat_tests`：27 passed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：28 passed。
- `cargo test -p studio-common --test managed_client_tests`：26 passed。
- `cargo test -p app-server-host bound_provider_type_mismatch_rejects_config`：passed。
- `cargo test -p app-server-host initial_turn_reservation`：2 passed。
- `cargo test -p app-server-protocol-wasm --tests`：0 failed。
- `cargo test -p studio-web-wasm --tests`：4 passed。
- `bun run --cwd packages/studio-web typecheck`：通过。
- `bun run --cwd packages/studio-web test:unit -- chat-actions.test.ts chat-zone.test.tsx protocol-store.test.ts protocol-package-import.test.ts wasm-client.test.ts`：92 passed；仍有两个既有 React `act(...)` 警告。
- `bun run --cwd packages/studio-web test:e2e -- tests/playwright/agent-chat-interaction.spec.ts tests/playwright/markdown-preview.spec.ts`：12 passed。
- `bun run protocol:build`：通过。
- `bun scripts/build_studio_web.ts`：通过。
- `git diff --check`：通过。

## Phase 4 完成情况

### 实现摘要

- Host 侧引入 workspace 级 `WorkspaceAgentRuntime`，按 canonical workspace path 复用同一 runtime；canonicalize 失败时回退原路径。
- Agent active turn registry 已从 `HostRequestDispatcher` 移入 runtime，workspace 单 active turn 约束仍由后端统一强制。
- `HostRequestDispatcher` 持有 runtime subscription；`disconnect()` 只移除 subscriber 和 watcher，不取消 active Agent。
- `agent.snapshot` 与 `agent.subscribe` 已接入 runtime，不再返回占位错误。
- Agent worker 的 push sink 改为 runtime sink：先记录 runtime event / legacy event，再广播给订阅该 `agent_id` 的 subscriber。
- Runtime event log 记录 `StateChanged`、token、reasoning、tool start/result、error、done，并维护 `current_text`、`current_reasoning`、terminal state、active turn。
- `AgentMeshReady`、`AgentPlanProposed`、`AgentPlanSaved` 这类旧 push 事件记录到 `legacy_events`，供 `agent.subscribe` 按 `since_event_id` 回放。
- `agent.subscribe` 在回放期间将新 live event 放入 subscriber pending 队列；回放发送完成后按顺序清空 pending，pending 清空后才恢复 live 直推，避免新事件先于旧回放到达。
- `agent.start_turn` 和旧 `agent.invoke` 仍通过 ChatStore 映射 `agent_id` / `session_id`，后续 turn 使用 chat bound model，不使用第二个 dispatcher 的本地模型选择覆盖绑定模型。
- WebSocket 层仍只创建 dispatcher 和 push sink；Agent 生命周期不再由单个 WebSocket connection 拥有。

### 验收说明

- 两个 dispatcher 可以引用同一个 `agent_id`，第二个 dispatcher 订阅后能收到第一个 dispatcher 启动的 Agent 事件。
- 第二个 dispatcher 可以读取 active Agent snapshot，snapshot 包含当前 `agent_id`、`chat_id`、active turn 和 runtime event。
- 第一个 dispatcher disconnect 后 active Agent 仍保持 running，第二个 dispatcher 可以继续 snapshot 并 cancel。
- 第二个 dispatcher 本地选择不同模型后，对已绑定 chat 发起后续 turn 时仍使用 `chats.json.bound_model`。
- 同 workspace 两个 dispatcher 启动第二个 active turn 时返回 `AgentBusy`。
- `agent.subscribe` 支持基于 snapshot event cursor 的回放；使用最新 event cursor 订阅不会重复回放旧事件。
- Runtime subscriber 回放期间的 live event 进入 pending 队列，回放完成后再按顺序发送，避免恢复事件乱序。
- Runtime 不使用 WebSocket connection id、push handle 或前端本地模型状态作为 Agent 身份来源；外部操作目标仍为 `agent_id`。

### Review 记录

- 第一轮独立 review 发现 legacy worker event 未写入 runtime 回放日志，以及测试缺少 bound model、workspace busy、snapshot events、`since_event_id` replay 覆盖；已修复。
- 第二轮独立 review 发现 `subscribe_agent` 回放与 live push 之间存在顺序竞态；已通过 subscriber `replaying_agents` 和 `pending_events` 修复，并补 runtime 单元测试。
- 第三轮独立 review 未发现 Phase 4 阻塞问题。剩余非阻塞风险：
  - 长时间高频 token 流可能让 `subscribe_agent` drain pending 的 request handler 响应变慢；后续可为 drain 增加批次上限或把 replay completion 明确建模为 runtime 内部状态。
  - 旧 live push 不携带 `AgentEventId`，客户端需要先通过 snapshot 获取 cursor；Phase 6 的结构化 event push / event log 设计需要处理该语义。
  - Workspace path canonicalize 失败时仍回退原始路径；若未来支持未创建 workspace 的早期 dispatcher，需要在 workspace 绑定完成后重新绑定 canonical runtime。

### 验证结果

- `cargo test -p app-server-host dispatcher::tests::runtime_subscribe_queues_live_events_until_replay_finishes`：passed。
- `cargo test -p app-server-host --test shared_dispatcher_roundtrip_tests`：34 passed。
- `rustfmt --edition 2024 --check crates/app-server-host/src/dispatcher.rs crates/app-server-host/tests/shared_dispatcher_roundtrip_tests.rs`：通过。
- `git diff --check`：通过。
- `bun run protocol:check-generated`：通过。

## 尚未执行

- 尚未实现持久化 event log、前端刷新后的 snapshot 恢复、idle 资源释放和 interrupted 重启恢复。
- 未迁移根目录 `llm.toml`，且本计划不要求迁移。

## 后续执行入口

- Phase 5 开始前必须重新通读 `plan-prompt.md`、`plan-00.md`、本结果文档、`docs/2026050200-agent-lifecycle-runtime/architecture.md` 和根 `AGENTS.md`。
- Phase 5 执行时必须保护 Phase 1 已达成的边界：后端随机 `chat_id`、`chats.json` 权威状态、chat 等同于 agent 的身份关系、Web 首发草稿语义和 protocol version 10。
- Phase 5 执行时必须保护 Phase 2 已达成的边界：三类 provider type、`base_url` 解析语义、`agents.toml` 私有配置边界、根目录 `llm.toml` 不作为产品配置入口，以及 Chat bound model 不持久化 `base_url`。
- Phase 5 执行时必须保护 Phase 3 已达成的边界：外部 Agent 操作目标使用 `agent_id`，首发 `chat.create.initial_turn` 保证同 `client_request_id` 幂等，后续 turn 使用 `agent.start_turn`，cancel 使用 `agent_id`，旧 `agent.invoke` 对已绑定 chat 不允许前端模型参数覆盖后端 bound model。
- Phase 5 执行时必须保护 Phase 4 已达成的边界：Agent runtime 按 workspace 共享，WebSocket disconnect 不取消 active Agent，多 dispatcher 可通过同一 `agent_id` snapshot / subscribe / cancel，runtime 强制 workspace 单 active turn。
