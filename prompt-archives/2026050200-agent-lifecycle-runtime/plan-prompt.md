# Agent 生命周期与 WebSocket 观察计划 Prompt 存档

## 原始背景

当前仓库：`/Users/krhougs/LocalCodes/scad-studio`

已提交的规则变更：

- `a2526e9 Document complete feature implementation rule`

该提交在根 `AGENTS.md` 中补充约束：budn' 尚未发布，功能实现必须按最精简的最佳实践一次性完成完整产品语义，禁止用临时兼容、前端假状态、只覆盖当前演示路径、后续再补的方式留下技术债。

## 用户确认的需求

1. Agent 生命周期要完全分离于外部 WebSocket 生命周期。
2. Agent 生命周期不直接依赖 WebSocket。
3. WebSocket 和 Agent 只通过消息共享状态。
4. 需要考虑多个 WebSocket connection 观察和交互同一个 Agent 的情况。
5. Agent 生命周期管理模块外的所有观察者和消费者，包括 WebSocket、用户和未来其他消费者，查询和操作目标永远是 Agent 本身的 `agent_id`，不是 `agent_run_id`。
6. 目标是多个消费者同时操作同一个 Agent 时不会互相覆盖状态或产生冲突。
7. LLM idle 且用户 WebSocket 未连接时，应释放 Agent 运行对象，节省资源。
8. Active Agent 在用户 WebSocket 断开、切换 chat 或刷新页面后仍应继续工作；用户重新连接后应能看到当前状态和后续实时输出。
9. 需要检查当前实现是否完全 async 化，以及是否存在多余的线程创建。
10. Reasoning 参数使用一层 `Option<String>` 即可完整表达业务语义：`None` 表示不发送，`Some(String)` 表示字符串原样发送给 LLM；不得引入嵌套 Option，也不得生成默认 reasoning 字符串。
11. Chat id 必须由后端随机生成，不得从 title 或文件名派生。
12. Workspace 根目录新增 `chats.json`，由它规定 chat 的显示顺序、当前 chat 和 metadata。
13. 产品语义中 chat 等同于 agent：每个 chat 拥有同一个稳定 `agent_id`。
14. Provider type 同时支持 `anthropic`、`openai_responses`、`openai_completions`。
15. Provider 产品配置使用 `base_url`，并按已确认规则补全或强制使用输入地址。
16. 根目录 `llm.toml` 属于本地开发环境相关配置，不迁移到产品文档或产品配置整改范围。
17. 当前任务只写设计文档和 plan，不做实现。

## 已核对的现状

- 当前 `HostRequestDispatcher` 每个 WebSocket connection 创建一次。
- 当前 `HostRequestDispatcher` 直接持有 `agent_runs: Arc<Mutex<AgentRunRegistry>>`。
- 当前 `AgentWorker` 直接持有当前 connection 的 `push_sink`。
- 当前 WebSocket 断开不会主动取消 worker，但 worker 的实时事件会发送到旧 connection 的 channel，新 connection 无法接管。
- 当前前端模型选择是 dispatcher 级运行时状态，不是 chat 级持久绑定。
- 当前 Chat history 未持久化 chat 与模型绑定。
- 当前 Chat session id 从 title 派生，并通过 JSONL 文件名反推出 id 和 title；该模型需要改为 `chats.json` 权威索引。
- 当前外部 Agent 操作和事件主要使用 `run_id`。
- 当前 Agent / WebSocket 主链路未发现手写系统线程、`spawn_blocking` 或 `block_in_place`。
- 已确认 `crates/scad-scene/src/system_fonts.rs` 中存在非 Agent / WebSocket 主链路的同步 `std::process::Command` 调用；该问题已记录到 `docs/known_issues.md`。
- 已核对 Rig 的 `Provider::build_uri` 行为：只负责斜杠拼接，不会替 OpenAI-compatible provider 自动追加 `/v1`。
- Rig OpenAI 默认 `base_url` 已包含 `/v1`；Anthropic 默认地址不由本计划改变。

## 本计划范围

- 设计 workspace 级 `WorkspaceAgentRuntime`。
- 设计 `chats.json` 驱动的 chat identity、显示顺序、当前 chat 和 metadata。
- 设计 provider type 和 `base_url` 产品配置规则。
- 设计稳定 `agent_id` 外部操作模型。
- 设计 chat 与 Agent / 模型绑定。
- 设计多 WebSocket 观察和事件重放。
- 设计 idle 资源释放策略。
- 设计迁移路径和验收测试。

## 非范围

- 不实现代码。
- 不迁移根目录 `llm.toml`。
- 不引入多 Agent 同时写 workspace 的并发写入语义。
- 不改变 CadQuery staging、path policy、workspace tool、preview、watch 的产品语义。
