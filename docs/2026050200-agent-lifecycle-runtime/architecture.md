# Agent 生命周期与 WebSocket 观察架构设计

## 背景

当前 Agent run 由 `HostRequestDispatcher` 启动，`HostRequestDispatcher` 又由每个 WebSocket connection 创建。Agent worker 持有当前 connection 的 `push_sink`，实时 token、reasoning、tool event 会直接推给该连接。这个结构能让一次连接内的 Agent run 工作，但无法满足刷新页面、多个 WebSocket 同时观察同一个 Agent、以及前端断开后重新接回实时状态的产品语义。

本设计将 Agent 生命周期从 WebSocket 生命周期中分离。WebSocket 只作为观察者和命令通道，Agent 生命周期由 workspace 级运行时管理。所有外部消费者通过 `agent_id` 查询和操作 Agent；`run_id` 或 `turn_id` 仅作为内部 turn 追踪字段，不作为外部操作目标。

## 当前行为核对

- 新建 chat 尚无消息时，前端可以通过后端返回的 `agent_model_registry` 选择模型。
- 当前模型选择是 WebSocket dispatcher 级运行时状态，不是 chat 级绑定状态。
- 发送消息时，前端会把当前模型快照随 `agent.invoke` 发送给后端；后端直接使用请求中的 provider/model/reasoning/service 参数。
- Chat history 当前没有持久化 chat 与模型的绑定关系。
- Reasoning 参数当前应保持 `Option<String>` 语义：`None` 表示不发送 reasoning 字段，`Some(String)` 表示把字符串原样发送给 LLM；不得引入嵌套 Option，也不得在后端生成默认 reasoning 字符串。
- WebSocket 断开不会主动 cancel 已启动的 worker，但旧 worker 仍持有旧连接的 `push_sink`，新连接无法接管旧 worker 的实时事件。
- 新连接会创建新的 `HostRequestDispatcher` 和新的 `AgentRunRegistry`，因此无法知道旧连接中仍在运行的 Agent run。

## 目标

- Agent 生命周期完全独立于 WebSocket 连接生命周期。
- WebSocket、用户、未来其他消费者只通过消息与 Agent runtime 共享状态。
- 多个 WebSocket connection 可以同时观察并操作同一个 Agent。
- 外部操作目标始终是稳定的 `agent_id`，不是 `run_id`。
- 一个 chat 首次发消息时绑定模型；绑定后同 chat 的后续消息必须使用同一模型，前端传入的模型变化不影响该 chat。
- 前端刷新或切换 chat 不影响 active Agent 的工作；重新连接后可以恢复当前 Agent 状态、历史事件和后续实时事件。
- LLM idle 且无 WebSocket observer 时，drop Agent 运行对象，释放 LLM client、stream、tool executor、subscriber 等运行资源，只保留可恢复的持久状态。
- Provider 产品配置支持 `anthropic`、`openai_responses`、`openai_completions` 三类 provider type，并支持各类型的 `base_url` 规则。
- 不引入临时兼容路径、前端假状态或只覆盖演示用例的实现。

## 非目标

- 本计划不迁移根目录 `llm.toml`；该文件属于本地开发环境相关配置，不进入产品配置整改范围。
- 本计划不引入多 Agent 同时写 workspace 的并发写入语义。
- 本计划不更改 CadQuery staging、path policy、workspace tool contract、preview 或 file watch 的产品语义。
- 本计划不把 WebSocket 会话 token 作为 Agent 身份来源。

## 核心模型

```rust
pub struct AgentId(pub String);

pub struct AgentRuntimeInstance {
    pub agent_id: AgentId,
    pub chat_session_id: ChatSessionId,
    pub model: BoundAgentModel,
    pub state: StoredAgentState,
    pub active_turn: Option<ActiveAgentTurn>,
}

pub struct ActiveAgentTurn {
    pub turn_id: AgentTurnId,
    pub cancel: CancellationToken,
    pub task: JoinHandle<()>,
}

pub struct BoundAgentModel {
    pub provider_id: String,
    pub provider_type: AgentProviderType,
    pub model_id: String,
    pub base_url: Option<String>,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
}
```

`AgentRuntimeInstance` 是后端 workspace 级运行对象，只在 Agent active 或仍有 subscriber 需要观察运行态时存在。`ActiveAgentTurn` 只在 LLM 正在输出、tool call 正在执行、或取消正在传播时存在。Agent idle 且没有 subscriber 后，runtime 必须 drop `AgentRuntimeInstance`，只通过 chat binding、event log 和 chat history 保留可恢复状态。

## Reasoning 参数语义

Reasoning 配置保持一层 `Option<String>`：

- `None` 表示不向 LLM provider request 写入 reasoning 字段。
- `Some(value)` 表示把 `value` 原样发送给 LLM。
- 后端不得为缺省 reasoning 自动生成默认字符串。
- 数据结构不得出现 `Option<Option<String>>` 或等价的嵌套缺省状态。

该语义已经覆盖当前业务场景：不传与原样发送是唯一需要表达的两种状态。

## Provider 与 base_url

产品配置以 `agents.toml` 和示例配置为准，不迁移根目录 `llm.toml`。

Provider type 固定支持三类产品语义：

- `anthropic`：Anthropic Messages API。
- `openai_responses`：OpenAI Responses API。
- `openai_completions`：OpenAI Chat Completions API 兼容形态。

`base_url` 解析规则由产品配置层统一处理，解析后的地址必须同时用于模型列表发现和 Agent turn 执行：

- 所有 provider type 的 `base_url` 若以 `#` 结尾，去掉末尾 `#` 后按输入地址原样使用，不做路径补全。
- Anthropic 不自动追加 `/v1`，因为 Rig Anthropic 调用路径已经包含 `/v1/messages` 和 `/v1/models`。
- OpenAI family provider 若未配置 `base_url`，使用 Rig 默认地址。
- OpenAI family provider 若配置的 `base_url` 不以 `/` 结尾且不以 `#` 强制原样结尾，则配置层补全为 `<base_url>/v1`。
- OpenAI family provider 若配置的 `base_url` 以 `/` 结尾，则保留该地址的路径语义，不追加 `/v1`。

已核对 Rig 行为：Rig 的 `Provider::build_uri` 只处理斜杠拼接，不会为 OpenAI-compatible provider 自动追加 `/v1`。因此 `/v1` 补全必须在 budn' 的配置解析层完成。

## 运行时边界

```rust
pub struct WorkspaceAgentRuntime {
    active_agents: HashMap<AgentId, AgentRuntimeInstance>,
    chat_bindings: ChatAgentBindingStore,
    subscribers: AgentSubscriberRegistry,
    event_log: AgentEventLog,
}
```

`WorkspaceAgentRuntime` 是唯一管理 Agent 生命周期的模块。WebSocket dispatcher 不再持有 `AgentRunRegistry`，也不直接创建 worker；它只向 runtime 发送命令，并订阅 runtime 输出的事件。

## 消息接口

```rust
pub enum AgentCommand {
    EnsureForChat {
        chat_session_id: ChatSessionId,
        requested_model: Option<BoundAgentModel>,
    },
    StartTurn {
        agent_id: AgentId,
        prompt: String,
        mode: AgentMode,
        context_refs: Vec<String>,
        plan_ref: Option<PathHandle>,
    },
    Cancel {
        agent_id: AgentId,
    },
    Snapshot {
        agent_id: AgentId,
        since_event_id: Option<AgentEventId>,
    },
    Subscribe {
        agent_id: AgentId,
        subscriber_id: AgentSubscriberId,
    },
}
```

所有外部命令都以 `agent_id` 为目标。`turn_id` 只存在于事件中，用于前端排序、去重和调试。

## Chat 与模型绑定

- Chat 首次发送消息前没有绑定模型，前端可以修改当前候选模型。
- 首次 `StartTurn` 前，后端用候选模型创建 `ChatAgentBinding`。
- `ChatAgentBinding` 必须持久化，保证刷新页面或重启 host 后仍可恢复。
- 已绑定 chat 的后续请求忽略前端传入的 provider/model/reasoning/service 参数，统一使用绑定模型。
- 前端切换到已绑定 chat 后，模型控件显示绑定模型并进入只读状态。

## 多 WebSocket 观察

每个 WebSocket connection 注册为 runtime subscriber。Agent 产生事件后先写入 runtime event log，再广播给当前所有 subscriber。多个 WebSocket connection 可以对同一个 `agent_id` 发起 snapshot、cancel 和 start turn 等命令；runtime 必须以 `agent_id` 和 chat binding 为准处理命令，不能让不同 connection 的本地模型状态覆盖 Agent 状态。

```rust
pub enum AgentEvent {
    StateChanged {
        agent_id: AgentId,
        turn_id: Option<AgentTurnId>,
        state: AgentState,
    },
    Token {
        agent_id: AgentId,
        turn_id: AgentTurnId,
        text: String,
    },
    Reasoning {
        agent_id: AgentId,
        turn_id: AgentTurnId,
        text: String,
    },
    ToolStarted {
        agent_id: AgentId,
        turn_id: AgentTurnId,
        tool_call_id: String,
        tool_name: String,
        args_json: String,
    },
    ToolFinished {
        agent_id: AgentId,
        turn_id: AgentTurnId,
        tool_call_id: String,
        result_json: String,
    },
    Done {
        agent_id: AgentId,
        turn_id: AgentTurnId,
    },
    Error {
        agent_id: AgentId,
        turn_id: Option<AgentTurnId>,
        message: String,
    },
}
```

断开的 WebSocket 只移除 subscriber，不改变 Agent 状态。新 WebSocket 连接后通过 snapshot 读取当前 Agent 状态和可重放事件，然后订阅后续事件。

## 资源释放

- Active turn 期间持有 LLM client、provider stream、tool executor、cancel token、task handle。
- WebSocket 断开不取消 active turn。
- Agent idle 且 subscriber 数为 0 时，drop Agent 运行对象，释放所有运行资源，仅保留 chat binding、event log、chat history 等持久状态。
- Agent done / failed / cancelled 后立即释放 active turn。
- 后续连接从持久状态恢复 UI，不重新创建 LLM stream。

## 并发策略

本计划保持当前产品语义的保守版本：workspace 内同一时间只允许一个 active Agent turn。这个约束由 `WorkspaceAgentRuntime` 强制，不能只依赖前端禁用按钮。

原因：

- 当前 workspace tool、CadQuery staging、文件写入与 chat history 都会修改共享 workspace。
- 在没有 workspace 写入调度和冲突处理模型前，允许多个 Agent turn 同时写 workspace 会引入真实数据冲突。
- 多个 WebSocket 同时观察和操作同一个 Agent 是本计划目标；多个 Agent 同时执行不是本计划目标。

## Async / 线程现状

当前 Agent / WebSocket 主链路未发现手写系统线程、`spawn_blocking` 或 `block_in_place`：

- WebSocket accept、connection、Agent worker 使用 `tokio::spawn`。
- CadQuery runner、OpenSCAD、export、slicer、CadQuery env verify 使用 `tokio::process::Command`。
- 测试代码中存在 `Runtime::new().block_on(...)` 和 `thread::spawn`，不属于生产路径。

已确认一个非 Agent / WebSocket 主链路问题：`crates/scad-scene/src/system_fonts.rs` 使用同步 `std::process::Command` 调用 `fc-match`。该问题已记录到 `docs/known_issues.md`。

## 验收口径

- 两个 WebSocket connection 订阅同一个 `agent_id` 时，都能收到同一组 active Agent 事件。
- 一个 WebSocket connection 启动 Agent turn 后，另一个 WebSocket connection 可以用同一个 `agent_id` snapshot / cancel / start turn，并得到同一个后端状态结果。
- 一个 WebSocket 断开后，active Agent 继续运行；新 WebSocket 连接后可通过 snapshot 恢复当前状态并继续接收后续事件。
- 同一个 chat 首次发送后绑定模型；后续请求即使携带不同模型，后端仍使用绑定模型。
- Reasoning 参数保持一层 `Option<String>`：`None` 不发送字段，`Some(String)` 原样发送，不生成默认 reasoning 字符串。
- 外部 cancel / snapshot / subscribe 命令只接受 `agent_id`，不接受 `run_id` 作为目标。
- Agent idle 且无 subscriber 时，不再持有 Agent 运行对象、active LLM client、provider stream、tool executor 或 WebSocket push handle。
- Protocol 类型保持 transport-neutral；`studio-common` 不依赖 transport、浏览器 API 或平台事件循环；Web 只负责展示和连接接线。
- 现有 Chat history、Agent event、CadQuery staging、workspace tool policy、preview、watch 和 Web 工作台行为保持可用。
