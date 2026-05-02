# Agent 生命周期与 WebSocket 观察架构执行结果

## 当前状态

- Phase 1「Chat identity 与 chats.json」已完成实现、验证、独立 review 和修正。
- Phase 2 尚未开始。
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
- `bun run protocol:check-generated`：提交前按预期失败，因为 generated WASM 已更新但尚未写入 git index；Phase 1 commit 后需要重新运行。

## 尚未执行

- Phase 2 Provider type 与 `base_url` 产品配置尚未执行。
- 尚未迁移 Agent 外部操作目标到 `agent_id`。
- 尚未实现 workspace 级 Agent runtime、多 WebSocket observer、event log replay、snapshot 恢复、idle 资源释放和 interrupted 重启恢复。
- 尚未实现 chat 模型绑定和后端模型强制。
- 未迁移根目录 `llm.toml`，且本计划不要求迁移。

## 后续执行入口

- Phase 2 开始前必须重新通读 `plan-prompt.md`、`plan-00.md`、本结果文档、`docs/2026050200-agent-lifecycle-runtime/architecture.md` 和根 `AGENTS.md`。
- Phase 2 执行时必须保护 Phase 1 已达成的边界：后端随机 `chat_id`、`chats.json` 权威状态、chat 等同于 agent 的身份关系、Web 首发草稿语义和 protocol version 9。
