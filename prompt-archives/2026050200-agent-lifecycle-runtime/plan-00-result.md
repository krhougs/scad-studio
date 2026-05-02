# Agent 生命周期与 WebSocket 观察架构执行结果

## 当前状态

- Phase 1「Chat identity 与 chats.json」已完成实现、验证、独立 review 和修正。
- Phase 2「Provider type 与 base_url 产品配置」已完成实现、验证、独立 review 和修正。
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

## 尚未执行

- 尚未迁移 Agent 外部操作目标到 `agent_id`。
- 尚未实现 workspace 级 Agent runtime、多 WebSocket observer、event log replay、snapshot 恢复、idle 资源释放和 interrupted 重启恢复。
- 尚未实现 chat 模型绑定和后端模型强制。
- 未迁移根目录 `llm.toml`，且本计划不要求迁移。

## 后续执行入口

- Phase 3 开始前必须重新通读 `plan-prompt.md`、`plan-00.md`、本结果文档、`docs/2026050200-agent-lifecycle-runtime/architecture.md` 和根 `AGENTS.md`。
- Phase 3 执行时必须保护 Phase 1 已达成的边界：后端随机 `chat_id`、`chats.json` 权威状态、chat 等同于 agent 的身份关系、Web 首发草稿语义和 protocol version 9。
- Phase 3 执行时必须保护 Phase 2 已达成的边界：三类 provider type、`base_url` 解析语义、`agents.toml` 私有配置边界、根目录 `llm.toml` 不作为产品配置入口，以及 Chat bound model 不持久化 `base_url`。
